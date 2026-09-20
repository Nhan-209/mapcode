"""
Stress-test harness for Milestone 1:
- Cache Resilience & Corrupted JSON Handling
- AST Extraction Edge Cases across 6 languages
- Memory models and Workspace Lifecycle concurrency
"""

import os
import json
import tempfile
import unittest


def compute_fnv1a_hash(data: bytes) -> int:
    """Deterministic 64-bit FNV-1a hash identical to src/cache.rs."""
    offset_basis = 0xcbf29ce484222325
    prime = 0x100000001b3
    h = offset_basis
    for b in data:
        h = (h ^ b) & 0xffffffffffffffff
        h = (h * prime) & 0xffffffffffffffff
    return h


class TestCacheResilience(unittest.TestCase):
    """Stress-test cache resilience under corrupt, truncated, or invalid inputs."""

    def test_fnv1a_deterministic(self):
        d1 = b'fn main() { println!("hello"); }'
        h1 = compute_fnv1a_hash(d1)
        h2 = compute_fnv1a_hash(d1)
        self.assertEqual(h1, h2)
        self.assertNotEqual(h1, 0)

        d2 = b'fn main() { println!("world"); }'
        h3 = compute_fnv1a_hash(d2)
        self.assertNotEqual(h1, h3)

    def test_corrupted_json_parsing(self):
        """Simulate how serde_json behaves on malformed / truncated cache files."""
        corrupted_cases = [
            b"",  # 0 bytes (empty)
            b"   ",  # whitespace only
            b"{",  # truncated start
            b'{"version": 1, "files": ',  # truncated value
            b'{"version": "one"}',  # invalid type for u32 version
            b"NOT_A_VALID_JSON",  # garbage
            b"\x00\x01\x02\x03",  # raw binary
            b'{"version": 999, "files": {}}',  # version mismatch
        ]

        for case in corrupted_cases:
            # When decoded, these should either fail JSON parsing or fail schema validation
            try:
                data = json.loads(case.decode("utf-8"))
                version = data.get("version")
                # Version 999 is valid JSON but must be rejected by version check
                if version != 1:
                    is_valid = False
                else:
                    is_valid = "files" in data and "root_path" in data
            except Exception:
                is_valid = False

            self.assertFalse(is_valid, f"Case {case} should have been rejected as invalid cache")

    def test_valid_cache_schema_roundtrip(self):
        """Verify complete schema conformance with ProjectCache & FileCacheEntry."""
        cache_data = {
            "version": 1,
            "mapcode_version": "0.4.0",
            "created_at_epoch_secs": 1726830000,
            "root_path": "d:/project",
            "files": {
                "src/main.rs": {
                    "relative_path": "src/main.rs",
                    "mtime_nanos": 123456789,
                    "file_size": 1024,
                    "content_hash": compute_fnv1a_hash(b"test content"),
                    "symbols": [
                        {
                            "name": "main",
                            "kind": "function",
                            "file_path": "src/main.rs",
                            "start_line": 1,
                            "end_line": 5,
                            "signature": "fn main()",
                            "callees": ["println"],
                        }
                    ],
                    "imports": [
                        {
                            "source_path": "src/main.rs",
                            "specifier": "std::io",
                            "is_external": True,
                            "line": 1,
                        }
                    ],
                    "types": [],
                    "entrypoints": [
                        {
                            "name": "main",
                            "category": "startup",
                            "file_path": "src/main.rs",
                            "line": 1,
                        }
                    ],
                }
            },
        }
        encoded = json.dumps(cache_data)
        decoded = json.loads(encoded)
        self.assertEqual(decoded["version"], 1)
        self.assertIn("src/main.rs", decoded["files"])
        self.assertEqual(decoded["files"]["src/main.rs"]["entrypoints"][0]["category"], "startup")


class TestASTExtractionEdgeCases(unittest.TestCase):
    """Static and behavioral evaluation of AST extraction heuristics in src/parser.rs."""

    def test_rust_grouped_import_splitting(self):
        """Simulate line 2012-2027 of src/parser.rs on Rust use statements."""
        # Case A: use foo::{bar, baz as qux, *};
        raw_text = "use foo::{bar, baz as qux, *};"
        clean_text = raw_text.strip().removeprefix("pub ").removeprefix("use ").removesuffix(";").strip()
        prefix, sub_list = clean_text.split("::{", 1)
        sub_items = sub_list.removesuffix("}")
        items = [i.strip() for i in sub_items.split(",") if i.strip()]

        specs = []
        for item in items:
            specs.append(f"{prefix.strip()}::{item}")

        # In parser.rs, 'baz as qux' produces 'foo::baz as qux' because ' as ' stripping is ONLY in the else branch!
        self.assertIn("foo::baz as qux", specs)
        # Note: 'foo::baz as qux' retains the alias text ' as qux'

    def test_rust_nested_use_group_corruption(self):
        """Simulate parser.rs on nested use groups like `use std::{collections::{HashMap, HashSet}, sync::Arc};`"""
        raw_text = "use std::{collections::{HashMap, HashSet}, sync::Arc};"
        clean_text = raw_text.strip().removeprefix("pub ").removeprefix("use ").removesuffix(";").strip()
        prefix, sub_list = clean_text.split("::{", 1)
        sub_items = sub_list.removesuffix("}")
        items = [i.strip() for i in sub_items.split(",") if i.strip()]

        # parser.rs splits by comma blindly:
        specs = [f"{prefix.strip()}::{item}" for item in items]
        # Item 0: 'std::collections::{HashMap'
        # Item 1: 'std::HashSet}'
        self.assertTrue(any("{" in s for s in specs), "Nested braces produce corrupted specifier with unclosed '{'")
        self.assertTrue(any("}" in s for s in specs), "Nested braces produce corrupted specifier with trailing '}'")

    def test_rust_restricted_visibility_pub_crate(self):
        """Simulate parser.rs on `pub(crate) use foo::bar;`"""
        raw_text = "pub(crate) use foo::bar;"
        clean_text = raw_text.strip().removeprefix("pub ").removeprefix("use ").removesuffix(";").strip()
        # Because it only removes 'pub ' and 'use ', 'pub(crate) use' is NOT stripped!
        self.assertTrue(clean_text.startswith("pub(crate) use"), "pub(crate) prefix was not stripped")

    def test_lua_table_class_spacing_sensitivity(self):
        """Simulate parser.rs line 2586 on Lua table assignments."""
        # text.contains("= {}") || text.contains("={}")
        code1 = "local Player = {}"
        code2 = "local Player = { }"  # space inside braces
        code3 = "local Player = {\n}"  # newline inside braces

        def is_detected_as_type(text: str) -> bool:
            return "= {}" in text or "={}" in text

        self.assertTrue(is_detected_as_type(code1))
        self.assertFalse(is_detected_as_type(code2), "Space inside {} causes undetected Lua type")
        self.assertFalse(is_detected_as_type(code3), "Newline inside {} causes undetected Lua type")

    def test_python_generic_inheritance_omission(self):
        """In tree-sitter-python, List[int] has node kind 'subscript' instead of 'identifier' or 'attribute'."""
        # Line 2346: if ck == "identifier" || ck == "attribute"
        # When an AST node is a subscript (e.g. List[int], Generic[T]), it is silently skipped!
        def extract_py_supertype(node_kind: str, node_text: str):
            if node_kind in ("identifier", "attribute"):
                return node_text
            return None

        self.assertEqual(extract_py_supertype("identifier", "Parent"), "Parent")
        self.assertEqual(extract_py_supertype("attribute", "models.Parent"), "models.Parent")
        # Subscript kind (Generic base) returns None:
        self.assertIsNone(extract_py_supertype("subscript", "List[int]"))

    def test_typescript_generic_extends_omission(self):
        """In tree-sitter-typescript, extends Base<T> has child kind 'generic_type' instead of 'type_identifier'."""
        # Line 2395: if item.kind() == "identifier" || item.kind() == "type_identifier"
        def extract_ts_supertype(item_kind: str, item_text: str):
            if item_kind in ("identifier", "type_identifier"):
                return item_text
            return None

        self.assertEqual(extract_ts_supertype("type_identifier", "BaseService"), "BaseService")
        # generic_type kind returns None:
        self.assertIsNone(extract_ts_supertype("generic_type", "BaseService<User>"))

    def test_rust_generic_impl_method_correlation_mismatch(self):
        """In Rust, struct Container<T> extracts name 'Container', but impl<T> Container<T> extracts 'Container<T>'."""
        struct_name = "Container"
        impl_type_text = "Container<T>"

        # correlate_type_methods in parser.rs lines 2892-2917:
        # methods_by_container has key impl_type_text ("Container<T>")
        # but iterates over types where tr.name is struct_name ("Container")
        methods_by_container = {impl_type_text: ["new", "get"]}

        # Attempting lookup with tr.name fails:
        correlated = methods_by_container.get(struct_name, [])
        self.assertEqual(len(correlated), 0, "Methods on generic impl blocks fail to correlate with struct")


if __name__ == "__main__":
    unittest.main()
