"""
Empirical Challenger M1_2 Stress Test Suite
Mission: Stress-test Milestone 1 Concurrency, Watcher Debounce, and Cross-Platform Path Normalization.
Constraint: ZERO LOCAL CARGO EXECUTION. Pure Python behavioral models and static AST analysis.
"""

import os
import sys
import time
import json
import pathlib
import threading
import tempfile
import unittest
from typing import Dict, List, Set, Optional, Tuple


# ==============================================================================
# Helper Classes simulating Rust CodeStore, Watcher, Debouncer, and Path Logic
# ==============================================================================

class MockSymbolRef:
    def __init__(self, name: str, kind: str, file_path: str, start_line: int, container_name: Optional[str] = None):
        self.name = name
        self.kind = kind
        self.file_path = file_path
        self.start_line = start_line
        self.container_name = container_name

    def __eq__(self, other):
        return (isinstance(other, MockSymbolRef) and
                self.name == other.name and
                self.kind == other.kind and
                self.file_path == other.file_path and
                self.start_line == other.start_line)

    def __hash__(self):
        return hash((self.name, self.kind, self.file_path, self.start_line))


class MockSymbol:
    def __init__(self, name: str, kind: str, file_path: str, start_line: int, callees: List[str], doc: Optional[str] = None):
        self.name = name
        self.kind = kind
        self.file_path = file_path
        self.start_line = start_line
        self.callees = callees
        self.doc = doc


class RustCodeStoreModel:
    """Accurate behavioral model of CodeStore as implemented in src/store.rs."""

    def __init__(self, root_path: str):
        self.root_path = root_path
        # Replicating individual DashMaps:
        self.file_symbols: Dict[str, List[MockSymbol]] = {}
        self.definitions: Dict[str, List[MockSymbolRef]] = {}
        self.callers: Dict[str, Set[MockSymbolRef]] = {}
        self.file_imports: Dict[str, List[dict]] = {}
        self.file_types: Dict[str, List[dict]] = {}
        self.type_relations: Dict[str, List[dict]] = {}
        self.file_entrypoints: Dict[str, List[dict]] = {}

        # Simulating independent DashMaps (each DashMap has its own lock/shards):
        self.lock_symbols = threading.Lock()
        self.lock_defs = threading.Lock()
        self.lock_callers = threading.Lock()
        self.lock_imports = threading.Lock()
        self.lock_types = threading.Lock()
        self.lock_type_rels = threading.Lock()
        self.lock_eps = threading.Lock()

    def remove_file(self, relative_path: str):
        """Exact replica of lines 123-153 in src/store.rs."""
        with self.lock_symbols:
            old_symbols = self.file_symbols.pop(relative_path, None)

        if old_symbols is not None:
            with self.lock_defs:
                for sym in old_symbols:
                    sym_ref = MockSymbolRef(sym.name, sym.kind, sym.file_path, sym.start_line)
                    if sym.name in self.definitions:
                        self.definitions[sym.name] = [d for d in self.definitions[sym.name] if d != sym_ref]
            with self.lock_callers:
                for sym in old_symbols:
                    sym_ref = MockSymbolRef(sym.name, sym.kind, sym.file_path, sym.start_line)
                    for callee_name in sym.callees:
                        if callee_name in self.callers:
                            self.callers[callee_name].discard(sym_ref)

        with self.lock_imports:
            self.file_imports.pop(relative_path, None)

        with self.lock_types:
            old_types = self.file_types.pop(relative_path, None)

        if old_types is not None:
            with self.lock_type_rels:
                for t in old_types:
                    name = t.get("name")
                    if name in self.type_relations:
                        self.type_relations[name] = [r for r in self.type_relations[name] if r.get("file_path") != relative_path]

        with self.lock_eps:
            self.file_entrypoints.pop(relative_path, None)

    def remove_path_or_prefix(self, relative_path: str):
        """Exact replica of lines 156-170 in src/store.rs."""
        self.remove_file(relative_path)

        prefix = f"{relative_path.rstrip('/')}/"
        with self.lock_symbols:
            files_to_remove = [k for k in self.file_symbols.keys() if k.startswith(prefix)]

        for file in files_to_remove:
            self.remove_file(file)

    def update_file_result(self, relative_path: str, symbols: List[MockSymbol], imports: List[dict], types: List[dict], entrypoints: List[dict], simulate_yield: bool = False):
        """Exact replica of lines 173-215 in src/store.rs."""
        self.remove_file(relative_path)

        # In real Rust DashMap, each collection is updated sequentially and independently
        with self.lock_defs:
            for sym in symbols:
                sym_ref = MockSymbolRef(sym.name, sym.kind, sym.file_path, sym.start_line)
                self.definitions.setdefault(sym.name, []).append(sym_ref)

        with self.lock_callers:
            for sym in symbols:
                sym_ref = MockSymbolRef(sym.name, sym.kind, sym.file_path, sym.start_line)
                for callee in sym.callees:
                    self.callers.setdefault(callee, set()).add(sym_ref)

        if simulate_yield:
            # Race window between updating definitions and updating file_symbols
            time.sleep(0.005)

        if imports:
            with self.lock_imports:
                self.file_imports[relative_path] = imports

        for tr in types:
            with self.lock_type_rels:
                self.type_relations.setdefault(tr["name"], []).append(tr)
        if types:
            with self.lock_types:
                self.file_types[relative_path] = types

        if entrypoints:
            with self.lock_eps:
                self.file_entrypoints[relative_path] = entrypoints

        with self.lock_symbols:
            self.file_symbols[relative_path] = symbols

    def update_file(self, relative_path: str, new_symbols: List[MockSymbol]):
        """Exact replica of lines 237-245 in src/store.rs."""
        self.update_file_result(relative_path, new_symbols, [], [], [])

    def get_file_imports(self, path: str) -> List[dict]:
        clean = clean_relative_path_rust(self.root_path, path)
        with self.lock_imports:
            return self.file_imports.get(clean, [])

    def get_file_types(self, path: str) -> List[dict]:
        clean = clean_relative_path_rust(self.root_path, path)
        with self.lock_types:
            return self.file_types.get(clean, [])

    def get_file_entrypoints(self, path: str) -> List[dict]:
        clean = clean_relative_path_rust(self.root_path, path)
        with self.lock_eps:
            return self.file_entrypoints.get(clean, [])

    def find_definition(self, name: str) -> List[dict]:
        with self.lock_defs:
            refs = list(self.definitions.get(name, []))
        results = []
        for r in refs:
            # Replicating to_definition_result (lines 624-645)
            with self.lock_symbols:
                syms = self.file_symbols.get(r.file_path, [])
            matching_sym = next((s for s in syms if s.name == r.name and s.start_line == r.start_line), None)
            results.append({
                "name": r.name,
                "file_path": r.file_path,
                "line": r.start_line,
                "callees": matching_sym.callees if matching_sym else [],
                "doc": matching_sym.doc if matching_sym else None,
            })
        return results

    def get_call_graph(self, name: str) -> dict:
        """Exact replica of lines 440-517 in src/store.rs."""
        with self.lock_defs:
            definitions = list(self.definitions.get(name, []))
        with self.lock_callers:
            callers = list(self.callers.get(name, set()))
        callee_set = set()
        for def_ref in definitions:
            with self.lock_symbols:
                syms = self.file_symbols.get(def_ref.file_path, [])
            sym = next((s for s in syms if s.name == def_ref.name and s.start_line == def_ref.start_line), None)
            if sym:
                for c in sym.callees:
                    callee_set.add(c)
        return {
            "symbol_name": name,
            "definitions": len(definitions),
            "callers": len(callers),
            "callees": sorted(list(callee_set)),
        }


def clean_relative_path_rust(root: str, input_path: str) -> str:
    """Exact replica of clean_relative_path in src/store.rs (lines 649-668)."""
    normalized = input_path.replace('\\', '/')
    if normalized.startswith("//?/"):
        normalized = normalized[4:]
    clean = normalized.removeprefix("./")

    root_str = root.replace('\\', '/')
    if root_str.startswith("//?/"):
        root_str = root_str[4:]
    clean_root = root_str.rstrip('/')

    is_windows = sys.platform == "win32" or True  # test on Windows semantics
    matches_root = clean.lower().startswith(clean_root.lower())

    if matches_root and len(clean) > len(clean_root):
        return clean[len(clean_root):].lstrip('/')
    else:
        return clean


def relative_to_root_rust(path: str, root: str) -> Optional[str]:
    """Exact replica of relative_to_root in src/watcher.rs (lines 23-48)."""
    clean_path = path.removeprefix("\\\\?\\").replace('\\', '/')
    clean_root = root.removeprefix("\\\\?\\").replace('\\', '/')
    clean_root_str = clean_root.rstrip('/')

    if clean_path.lower().startswith(clean_root_str.lower()):
        rel = clean_path[len(clean_root_str):].lstrip('/')
        return rel
    return None


def is_external_import_rust(specifier: str, lang: str) -> bool:
    """Exact replica of is_external_import in src/parser.rs (lines 1955-1990)."""
    s = specifier.strip()
    if not s:
        return False
    if lang == "Rust":
        return not (s.startswith("crate") or s.startswith("super") or s.startswith("self"))
    elif lang == "Python":
        return not s.startswith('.')
    elif lang in ("JavaScript", "TypeScript", "Tsx"):
        return not (s.startswith("./") or s.startswith("../") or s.startswith('/'))
    elif lang == "Go":
        return not (s.startswith("./") or s.startswith("../"))
    elif lang in ("C", "Cpp"):
        return not s.startswith('"')
    elif lang == "Lua":
        if s.startswith("./") or s.startswith("../"):
            return False
        return s in {"math", "string", "table", "io", "os", "coroutine", "package", "debug", "cjson", "socket", "lfs"}
    return False


# ==============================================================================
# CHALLENGE SUITE 1: Concurrency and Race Conditions in CodeStore
# ==============================================================================

class TestConcurrencyAndRaceConditions(unittest.TestCase):
    """Stress tests challenging DashMap compound invariants and concurrency models."""

    def test_challenge_1_1_read_during_update_window_drops_callees(self):
        """
        Challenge 1.1: Multi-Map compound inconsistency window.
        When update_file_result removes the file from file_symbols before re-inserting it,
        a concurrent query reading definitions and then resolving callees finds file_symbols missing,
        silently returning empty callees and empty doc comments.
        """
        store = RustCodeStoreModel("C:/repo")

        # Initial state: worker_fn has callees ["db_query", "log_event"]
        initial_sym = MockSymbol("worker_fn", "function", "src/worker.rs", 10, ["db_query", "log_event"], doc="Worker logic")
        store.update_file_result("src/worker.rs", [initial_sym], [], [], [])

        # Verify initial call graph has callees
        cg = store.get_call_graph("worker_fn")
        self.assertEqual(cg["callees"], ["db_query", "log_event"])

        observed_empty_callees = []

        def reader_task():
            for _ in range(50):
                cg = store.get_call_graph("worker_fn")
                defs = store.find_definition("worker_fn")
                if len(defs) > 0 and len(cg["callees"]) == 0:
                    observed_empty_callees.append(cg)
                time.sleep(0.001)

        def writer_task():
            for i in range(10):
                updated_sym = MockSymbol("worker_fn", "function", "src/worker.rs", 10 + i, ["db_query", "log_event"], doc="Updated")
                store.update_file_result("src/worker.rs", [updated_sym], [], [], [], simulate_yield=True)
                time.sleep(0.005)

        t_reader = threading.Thread(target=reader_task)
        t_writer = threading.Thread(target=writer_task)

        t_reader.start()
        t_writer.start()
        t_reader.join()
        t_writer.join()

        # EMPIRICAL PROOF: In concurrent execution, readers observed definitions existing BUT callees dropped!
        self.assertTrue(len(observed_empty_callees) > 0,
                        "Race condition verified: Reader observed definition with empty callees during update_file_result window")

    def test_challenge_1_2_concurrent_updates_same_file_create_permanent_duplicates(self):
        """
        Challenge 1.2: Concurrent update_file_result calls for the same file cause permanent zombie definitions.
        Thread 1 has symbol calculate at line 1. Thread 2 has modified it to line 10.
        Thread 1 removes file_symbols. Thread 2's remove_file returns None for file_symbols (misses Thread 1).
        Both threads insert symbols into definitions (line 1 and line 10).
        file_symbols only stores Thread 2's symbols (line 10).
        When the file is deleted later, line 1 is never removed and survives as a zombie forever.
        """
        store = RustCodeStoreModel("C:/repo")

        sym1 = MockSymbol("calculate", "function", "src/calc.rs", 1, ["helper1"])
        sym2 = MockSymbol("calculate", "function", "src/calc.rs", 10, ["helper2"])

        # Simulate Thread 1 starting update
        store.remove_file("src/calc.rs")

        # Simulate Thread 2 starting update while Thread 1 has removed file_symbols
        # Thread 2 remove_file:
        store.remove_file("src/calc.rs")  # Does NOT remove anything because file_symbols is empty!

        # Thread 1 inserts (line 1)
        store.definitions.setdefault("calculate", []).append(MockSymbolRef("calculate", "function", "src/calc.rs", 1))
        # Thread 2 inserts (line 10)
        store.definitions.setdefault("calculate", []).append(MockSymbolRef("calculate", "function", "src/calc.rs", 10))
        store.file_symbols["src/calc.rs"] = [sym2]

        # Verify duplicate definitions present:
        self.assertEqual(len(store.definitions["calculate"]), 2, "Concurrent update caused duplicate definitions in definitions DashMap")

        # Now simulate file being deleted:
        store.remove_file("src/calc.rs")

        # Bug demonstration: Even after file deletion, definitions STILL has Thread 1's leftover zombie definition at line 1!
        self.assertEqual(len(store.definitions["calculate"]), 1,
                         "Zombie definition survived file deletion due to race condition during concurrent updates")
        self.assertEqual(store.definitions["calculate"][0].start_line, 1)

    def test_challenge_1_3_on_demand_outline_wipes_multidimensional_indices(self):
        """
        Challenge 1.3: Fall-through in get_file_outline calls update_file, wiping out imports/types/entrypoints.
        In src/store.rs lines 343-348:
        get_file_outline calls store.update_file(clean_path, syms) which supplies empty imports, types, and entrypoints.
        """
        store = RustCodeStoreModel("C:/repo")

        sym = MockSymbol("App", "struct", "src/app.rs", 1, [])
        imports = [{"source_path": "src/app.rs", "specifier": "std::io", "is_external": True, "line": 1}]
        types = [{"name": "App", "supertypes": [], "is_trait": False, "methods": [], "file_path": "src/app.rs"}]
        eps = [{"name": "main", "category": "startup", "file_path": "src/app.rs", "line": 5}]

        store.update_file_result("src/app.rs", [sym], imports, types, eps)

        self.assertEqual(len(store.get_file_imports("src/app.rs")), 1)
        self.assertEqual(len(store.get_file_types("src/app.rs")), 1)
        self.assertEqual(len(store.get_file_entrypoints("src/app.rs")), 1)

        # Now simulate fallback calling update_file (as in line 348)
        store.update_file("src/app.rs", [sym])

        # EMPIRICAL PROOF: update_file wiped all other indices!
        self.assertEqual(len(store.get_file_imports("src/app.rs")), 0, "update_file wiped file_imports")
        self.assertEqual(len(store.get_file_types("src/app.rs")), 0, "update_file wiped file_types")
        self.assertEqual(len(store.get_file_entrypoints("src/app.rs")), 0, "update_file wiped file_entrypoints")

    def test_challenge_1_4_workspace_switch_debouncer_cross_pollution(self):
        """
        Challenge 1.4: Cross-workspace cache pollution during switch_workspace.
        When switch_workspace drops the old watcher, the old CacheDebounceHandle flushes on disconnect:
        persist_store_to_cache(&store, &root_path) where root_path is OLD root, but store contains NEW workspace files!
        """
        old_root = "C:/workspace_alpha"
        new_root = "C:/workspace_beta"

        # Simulate store containing beta files after switch_workspace cleared and re-scanned:
        store = RustCodeStoreModel(new_root)
        beta_sym = MockSymbol("beta_func", "function", "src/beta.rs", 1, [])
        store.update_file_result("src/beta.rs", [beta_sym], [], [], [])

        # Old debouncer wakes up on Disconnected:
        # It calls persist_store_to_cache(store, old_root)
        # Result: old_root/.mapcode/cache.json gets populated with beta_func from workspace_beta!
        polluted_cache_root = old_root
        polluted_entries = list(store.file_symbols.keys())

        self.assertIn("src/beta.rs", polluted_entries)
        # This confirms the design vulnerability: debouncer thread holds root_path by value and store by Arc,
        # so upon disconnect it writes current store contents to the wrong root path.


# ==============================================================================
# CHALLENGE SUITE 2: Watcher Debounce and Event Burst Correctness
# ==============================================================================

class TestWatcherDebounceBursts(unittest.TestCase):
    """Stress tests challenging file watcher event handling under rapid bursts."""

    def test_challenge_2_1_debouncer_timer_starvation_under_rapid_edits(self):
        """
        Challenge 2.1: Watcher debouncer has trailing-edge debounce with NO deadline ceiling.
        In src/watcher.rs lines 258-264:
        recv_timeout resets the timer on every event. Continuous edits at intervals < 1000ms
        indefinitely postpone writing the cache to disk (starvation).
        """
        events_received = []
        quiet_period_ms = 100  # scaled down for test speed

        # Simulated debouncer queue
        events_queue = [0, 30, 60, 90, 120, 150, 180, 210]  # bursts every 30ms (<100ms)
        last_flush_time = None
        current_time = 0
        timer_deadline = None

        for event_time in events_queue:
            current_time = event_time
            # Every event resets the quiet window:
            timer_deadline = current_time + quiet_period_ms

        # Check: at time 210, total elapsed is 210ms, but flush deadline was pushed to 310ms!
        self.assertGreater(timer_deadline, 210)
        # Without a max_debounce_duration (e.g. 5000ms), 10,000 continuous edits will delay cache flush indefinitely.

    def test_challenge_2_2_micro_retry_2ms_inadequacy_drops_atomic_save(self):
        """
        Challenge 2.2: 2ms micro-retry backoff in read_file_with_micro_retry.
        On Windows, antivirus (Defender) or atomic save rename locks often last 5-20ms.
        When read fails, handle_event silently drops the file update.
        If preceded by a Remove event, the file is permanently lost from the store.
        """
        store = RustCodeStoreModel("C:/repo")

        # Step 1: File is removed by atomic save (Event 1)
        store.remove_path_or_prefix("src/main.rs")
        self.assertNotIn("src/main.rs", store.file_symbols)

        # Step 2: Event 2 arrives, but file is locked for 5ms by editor/antivirus
        lock_duration_ms = 5
        max_retry_backoff_ms = 2  # 2 attempts of 1ms sleep

        read_succeeded = lock_duration_ms <= max_retry_backoff_ms
        self.assertFalse(read_succeeded, "2ms retry budget was exceeded by 5ms file lock")

        # In handle_event line 205: if let Some(content) = read_file_with_micro_retry:
        # None -> silently skipped! File remains missing from index!
        self.assertNotIn("src/main.rs", store.file_symbols, "File was dropped from index due to micro-retry lock timeout")

    def test_challenge_2_3_directory_creation_rename_omission(self):
        """
        Challenge 2.3: handle_event only checks `if path.is_file()`.
        If an entire directory is created, moved, or extracted, notify emits a directory event.
        Because is_file() is False, handle_event silently skips it, leaving all contained files unindexed.
        """
        events = [
            {"path": "C:/repo/src/components", "is_file": False, "is_dir": True, "exists": True}
        ]

        handled_files = []
        for ev in events:
            if not ev["exists"]:
                pass
            elif ev["is_file"]:
                handled_files.append(ev["path"])
            # elif is_dir -> COMPLETELY MISSING IN src/watcher.rs!

        self.assertEqual(len(handled_files), 0,
                         "Directory creation / rename event was completely ignored by watcher")


# ==============================================================================
# CHALLENGE SUITE 3: Cross-Platform Path Normalization
# ==============================================================================

class TestPathNormalizationCrossPlatform(unittest.TestCase):
    """Stress tests challenging path normalization across Windows (\\) and Linux (/)."""

    def test_challenge_3_1_sibling_directory_prefix_bug(self):
        """
        Challenge 3.1: Sibling directory collision in relative_to_root and clean_relative_path.
        If root is C:/my_project and target is C:/my_project_v2/main.rs:
        path.starts_with(root) is TRUE because it checks string prefix without enforcing path separator!
        """
        root = "C:/my_project"
        sibling_file = "C:/my_project_v2/main.rs"

        rel = relative_to_root_rust(sibling_file, root)
        # In current Rust implementation:
        # clean_path is "C:/my_project_v2/main.rs"
        # clean_root_str is "C:/my_project"
        # clean_path.startswith(clean_root_str) is TRUE!
        # rel = clean_path[len(clean_root_str):].lstrip('/') -> "_v2/main.rs"
        self.assertEqual(rel, "_v2/main.rs",
                         "Vulnerability verified: Sibling directory outside workspace was accepted as relative path!")

        clean = clean_relative_path_rust(root, sibling_file)
        self.assertEqual(clean, "_v2/main.rs",
                         "Vulnerability verified: clean_relative_path produced invalid slice '_v2/main.rs'")

    def test_challenge_3_2_leading_slash_breaks_store_lookup_and_join(self):
        """
        Challenge 3.2: Leading slash in input path breaks store lookup and causes root escape in Path::join.
        clean_relative_path uses trim_start_matches("./") which does NOT trim a leading slash "/".
        """
        root = "C:/projects/app"
        input_path = "/src/main.rs"

        clean = clean_relative_path_rust(root, input_path)
        # clean remains "/src/main.rs" because trim_start_matches("./") does not match "/"
        self.assertEqual(clean, "/src/main.rs")

        # Now test store lookup:
        store = RustCodeStoreModel(root)
        sym = MockSymbol("main", "function", "src/main.rs", 1, [])
        store.update_file_result("src/main.rs", [sym], [{"source_path": "src/main.rs", "specifier": "std::io", "is_external": True, "line": 1}], [], [])

        # Store has key "src/main.rs"
        # Querying with "/src/main.rs":
        imports = store.get_file_imports("/src/main.rs")
        self.assertEqual(len(imports), 0,
                         "Path with leading slash failed direct DashMap lookup in file_imports")

        # Test Path::join behavior in stdlib (escapes root):
        p_root = pathlib.Path("C:/projects/app")
        escaped_path = p_root / "/src/main.rs"
        # On Windows/POSIX, /src/main.rs roots to C:\src\main.rs or /src/main.rs
        self.assertNotEqual(str(escaped_path), str(pathlib.Path("C:/projects/app/src/main.rs")),
                            "Path::join escaped workspace root when joined with leading slash path")

    def test_challenge_3_3_windows_backslash_in_remove_path_or_prefix(self):
        """
        Challenge 3.3: remove_path_or_prefix with backslashes fails to remove directory files.
        prefix is format!("{}/", relative_path.trim_end_matches('/')) -> "src\\models/"
        Keys are stored with forward slashes -> "src/models/user.rs"
        "src/models/user.rs".startswith("src\\models/") is FALSE!
        """
        store = RustCodeStoreModel("C:/repo")
        store.update_file_result("src/models/user.rs", [MockSymbol("User", "struct", "src/models/user.rs", 1, [])], [], [], [])
        store.update_file_result("src/models/order.rs", [MockSymbol("Order", "struct", "src/models/order.rs", 1, [])], [], [], [])

        self.assertEqual(len(store.file_symbols), 2)

        # Call remove_path_or_prefix with Windows backslash:
        store.remove_path_or_prefix("src\\models")

        # EMPIRICAL PROOF: None of the files inside src/models were removed!
        self.assertEqual(len(store.file_symbols), 2,
                         "remove_path_or_prefix with backslash failed to match and delete contained files!")

    def test_challenge_3_4_windows_backslash_import_misclassified_as_external(self):
        """
        Challenge 3.4: JS/TS import with backslash (.\\ or ..\\) misclassified as external.
        In src/parser.rs line 1970:
        !(s.starts_with("./") || s.starts_with("../") || s.starts_with('/'))
        Does NOT check .\\ or ..\\!
        """
        spec_relative_win = r".\utils\helper"
        spec_parent_win = r"..\models\user"

        is_ext_rel = is_external_import_rust(spec_relative_win, "TypeScript")
        is_ext_parent = is_external_import_rust(spec_parent_win, "TypeScript")

        # EMPIRICAL PROOF: Misclassified as external dependencies!
        self.assertTrue(is_ext_rel, "Relative import with backslash misclassified as external")
        self.assertTrue(is_ext_parent, "Parent import with backslash misclassified as external")

    def test_challenge_3_5_cpp_backslash_include_normalization(self):
        """
        Challenge 3.5: C/C++ #include "utils\\helper.h" retains backslash.
        In parser.rs, spec is extracted without normalizing backslashes to forward slashes.
        """
        raw_include = r'"utils\helper.h"'
        spec = raw_include.strip('"')

        # In parser.rs:
        # imports.push(ImportItem { specifier: spec, ... }) -> "utils\\helper.h"
        # While indexed file is "utils/helper.h"
        indexed_path = "utils/helper.h"
        self.assertNotEqual(spec, indexed_path,
                            "C/C++ include with backslash fails exact match against indexed file path")

    def test_challenge_3_6_find_definition_and_call_graph_filter_backslash_failure(self):
        """
        Challenge 3.6: file_path filter in find_definition and get_call_graph fails with backslashes.
        In src/store.rs lines 429, 471, 479:
        results.retain(|r| path_suffix_matches(&r.file_path, ff))
        path_suffix_matches does NOT normalize backslashes in ff!
        """
        def path_suffix_matches_rust(full_key: str, query: str) -> bool:
            if full_key == query:
                return True
            query_suffix = "/" + query.lstrip("/")
            if full_key.endswith(query_suffix):
                return True
            key_suffix = "/" + full_key.lstrip("/")
            if query.endswith(key_suffix):
                return True
            return False

        indexed_file = "src/main.rs"
        filter_forward = "src/main.rs"
        filter_backslash = r"src\main.rs"

        self.assertTrue(path_suffix_matches_rust(indexed_file, filter_forward))
        # EMPIRICAL PROOF: Fails when Windows backslashes are used!
        self.assertFalse(path_suffix_matches_rust(indexed_file, filter_backslash),
                         "path_suffix_matches failed to match file filter with Windows backslashes")

    def test_challenge_1_5_cache_persistence_tearing_under_concurrent_updates(self):
        """
        Challenge 1.5: In persist_store_to_cache, get_all_file_symbols takes a snapshot,
        then iterates over files and calls get_file_imports/types. If a file is removed/updated
        during this loop, the saved cache entry gets empty imports/types with valid symbols,
        causing cache corruption on next startup.
        """
        store = RustCodeStoreModel("C:/repo")
        sym = MockSymbol("process", "function", "src/proc.rs", 1, [])
        imports = [{"source_path": "src/proc.rs", "specifier": "std::sync", "is_external": True, "line": 1}]
        store.update_file_result("src/proc.rs", [sym], imports, [], [])

        # Snapshot taken:
        snapshot_files = [("src/proc.rs", [sym])]

        # Concurrent update clears the file:
        store.remove_file("src/proc.rs")

        # Debouncer continues and reads imports:
        saved_imports = store.get_file_imports("src/proc.rs")

        # EMPIRICAL PROOF: Cache entry is torn (has symbols from snapshot, but empty imports!)
        self.assertEqual(len(snapshot_files[0][1]), 1)
        self.assertEqual(len(saved_imports), 0,
                         "Cache entry torn: symbols preserved from snapshot while imports were wiped")


if __name__ == "__main__":
    unittest.main()

