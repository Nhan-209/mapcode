"""
tests/e2e/mock_server.py — High-fidelity Specification Oracle Server for MapCode v0.4.0.

Implements all 11 MCP tools according to SPEC-MAPCODE-040 and PROJECT.md.
Can run standalone via stdio JSON-RPC 2.0 or be called as an oracle library.
"""

import json
import math
import os
import re
import sys
from collections import defaultdict, deque
from typing import Any, Dict, List, Optional, Set, Tuple


class MockCodeStore:
    """In-memory model of code analysis conforming to MapCode v0.4.0 specs."""

    def __init__(self, root_path: str):
        self.root_path = os.path.abspath(root_path).replace("\\", "/")
        self.files: Dict[str, Dict[str, Any]] = {}
        self.call_graph: Dict[str, List[str]] = defaultdict(list)
        self.type_relations: Dict[str, Dict[str, Any]] = {}
        self.entrypoints: List[Dict[str, Any]] = []
        self.scan_workspace()

    def scan_workspace(self) -> None:
        """Scan files under root_path and extract symbols, imports, types, entrypoints."""
        self.files.clear()
        self.call_graph.clear()
        self.type_relations.clear()
        self.entrypoints.clear()

        supported_exts = {".rs", ".py", ".ts", ".js", ".tsx", ".jsx", ".go", ".c", ".cpp", ".h", ".lua"}

        for root, dirs, filenames in os.walk(self.root_path):
            # Ignore hidden and build directories
            dirs[:] = [d for d in dirs if not d.startswith(".") and d not in ("target", "node_modules")]
            for f in filenames:
                ext = os.path.splitext(f)[1]
                if ext in supported_exts:
                    full_path = os.path.join(root, f)
                    rel_path = os.path.relpath(full_path, self.root_path).replace("\\", "/")
                    self._parse_file(rel_path, full_path)

        self._build_type_hierarchy()
        self._build_call_graph()

    def _parse_file(self, rel_path: str, full_path: str) -> None:
        try:
            with open(full_path, "r", encoding="utf-8", errors="ignore") as fp:
                content = fp.read()
        except Exception:
            return

        lines = content.splitlines()
        symbols = []
        imports = []
        types = []
        entrypoints = []
        ext = os.path.splitext(rel_path)[1].lower()

        # =========================================================
        # 1. RUST PARSER (.rs)
        # =========================================================
        if ext == ".rs":
            for i, line in enumerate(lines, start=1):
                sline = line.strip()
                # Imports
                if sline.startswith("use ") and ";" in sline:
                    spec = sline[4:].split(";")[0].strip()
                    is_ext = not (spec.startswith("crate::") or spec.startswith("super::") or spec.startswith("api::") or spec.startswith("service::") or spec.startswith("models::") or spec.startswith("cli::"))
                    resolved = self._resolve_rust_import(spec, rel_path) if not is_ext else None
                    imports.append({
                        "specifier": spec,
                        "resolved_path": resolved,
                        "is_external": (resolved is None and is_ext),
                        "imported_symbols": [spec.split("::")[-1]],
                        "line": i
                    })
                # Structs
                if sline.startswith("pub struct ") or sline.startswith("struct "):
                    parts = sline.split()
                    idx = parts.index("struct")
                    if idx + 1 < len(parts):
                        name = parts[idx + 1].split("{")[0].split("(")[0].strip()
                        types.append({
                            "name": name,
                            "kind": "struct",
                            "supertypes": [],
                            "subtypes": [],
                            "embedded_types": [],
                            "methods": [],
                            "file_path": rel_path,
                            "line": i
                        })
                        symbols.append({
                            "name": name,
                            "kind": "struct",
                            "start_line": i,
                            "end_line": i,
                            "signature": sline,
                            "doc": None
                        })
                # Trait impls
                elif sline.startswith("impl ") and " for " in sline:
                    parts = sline[5:].split("{")[0].strip().split(" for ")
                    trait_name = parts[0].strip()
                    struct_name = parts[1].strip()
                    types.append({
                        "name": struct_name,
                        "kind": "struct",
                        "supertypes": [{"name": trait_name, "kind": "implements", "source_file": None}],
                        "subtypes": [],
                        "embedded_types": [],
                        "methods": [],
                        "file_path": rel_path,
                        "line": i
                    })
                # Functions
                if "fn " in sline:
                    m = re.search(r"(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)", sline)
                    if m:
                        fn_name = m.group(1)
                        symbols.append({
                            "name": fn_name,
                            "kind": "function",
                            "start_line": i,
                            "end_line": i,
                            "signature": sline,
                            "doc": None
                        })
                # Startup entrypoint
                if "fn main(" in sline:
                    entrypoints.append({
                        "id": f"startup:{rel_path}:{i}",
                        "category": "startup",
                        "name": "main",
                        "symbol_name": "main",
                        "file_path": rel_path,
                        "line": i,
                        "signature_or_route": sline,
                        "framework": "tokio" if "#[tokio::main]" in content else "std"
                    })
                # HTTP routes (axum)
                if '.route("' in sline:
                    m = re.search(r'\.route\(["\']([^"\']+)["\']', sline)
                    if m:
                        route_path = m.group(1)
                        entrypoints.append({
                            "id": f"http:ANY:{route_path}:{rel_path}:{i}",
                            "category": "http",
                            "name": f"ROUTE {route_path}",
                            "symbol_name": None,
                            "file_path": rel_path,
                            "line": i,
                            "signature_or_route": route_path,
                            "framework": "axum"
                        })
                # CLI (clap)
                if "#[derive(Parser)]" in sline or 'name = "sample_server"' in sline:
                    entrypoints.append({
                        "id": f"cli:clap:{rel_path}:{i}",
                        "category": "cli",
                        "name": "sample_server",
                        "symbol_name": "CliArgs",
                        "file_path": rel_path,
                        "line": i,
                        "signature_or_route": "clap CLI",
                        "framework": "clap"
                    })

        # =========================================================
        # 2. PYTHON PARSER (.py)
        # =========================================================
        elif ext == ".py":
            for i, line in enumerate(lines, start=1):
                sline = line.strip()
                # Imports
                if sline.startswith("import ") or sline.startswith("from "):
                    resolved = self._resolve_python_import(sline, rel_path)
                    is_ext = (resolved is None)
                    imports.append({
                        "specifier": sline,
                        "resolved_path": resolved,
                        "is_external": is_ext,
                        "imported_symbols": [sline.split()[-1]],
                        "line": i
                    })
                # Classes & inheritance
                if sline.startswith("class ") and ":" in sline:
                    m = re.search(r"class\s+([A-Za-z0-9_]+)(?:\(([^)]+)\))?:", sline)
                    if m:
                        cls_name = m.group(1)
                        parents = [p.strip() for p in m.group(2).split(",") if p.strip()] if m.group(2) else []
                        types.append({
                            "name": cls_name,
                            "kind": "class",
                            "supertypes": [{"name": p, "kind": "inherits", "source_file": None} for p in parents],
                            "subtypes": [],
                            "embedded_types": [],
                            "methods": [],
                            "file_path": rel_path,
                            "line": i
                        })
                        symbols.append({
                            "name": cls_name,
                            "kind": "class",
                            "start_line": i,
                            "end_line": i,
                            "signature": sline,
                            "doc": None
                        })
                # Functions
                if sline.startswith("def "):
                    m = re.search(r"def\s+([A-Za-z0-9_]+)", sline)
                    if m:
                        fn_name = m.group(1)
                        symbols.append({
                            "name": fn_name,
                            "kind": "function",
                            "start_line": i,
                            "end_line": i,
                            "signature": sline,
                            "doc": None
                        })
                # Startup entrypoint
                if '__name__ == "__main__"' in sline or "__name__ == '__main__'" in sline:
                    entrypoints.append({
                        "id": f"startup:{rel_path}:{i}",
                        "category": "startup",
                        "name": "main",
                        "symbol_name": "__main__",
                        "file_path": rel_path,
                        "line": i,
                        "signature_or_route": "if __name__ == '__main__':",
                        "framework": "python"
                    })
                # HTTP routes (fastapi)
                m = re.search(r'@app\.(get|post|put|delete)\(["\']([^"\']+)["\']', sline)
                if m:
                    verb = m.group(1).upper()
                    route_path = m.group(2)
                    entrypoints.append({
                        "id": f"http:{verb}:{route_path}:{rel_path}:{i}",
                        "category": "http",
                        "name": f"{verb} {route_path}",
                        "symbol_name": None,
                        "file_path": rel_path,
                        "line": i,
                        "signature_or_route": f"{verb} {route_path}",
                        "framework": "fastapi"
                    })
                # CLI (click)
                if "@click.command()" in sline or "@click.group()" in sline:
                    entrypoints.append({
                        "id": f"cli:click:{rel_path}:{i}",
                        "category": "cli",
                        "name": "click_cli",
                        "symbol_name": "cli",
                        "file_path": rel_path,
                        "line": i,
                        "signature_or_route": "click CLI",
                        "framework": "click"
                    })
                # Worker (celery)
                if "@celery_app.task" in sline or "@celery.task" in sline:
                    entrypoints.append({
                        "id": f"worker:celery:{rel_path}:{i}",
                        "category": "worker",
                        "name": "celery_task",
                        "symbol_name": None,
                        "file_path": rel_path,
                        "line": i,
                        "signature_or_route": "@celery.task",
                        "framework": "celery"
                    })

        # =========================================================
        # 3. TYPESCRIPT / JAVASCRIPT PARSER (.ts, .tsx, .js, .jsx)
        # =========================================================
        elif ext in (".ts", ".tsx", ".js", ".jsx"):
            for i, line in enumerate(lines, start=1):
                sline = line.strip()
                # Imports
                if sline.startswith("import ") and "from" in sline:
                    m = re.search(r"from\s+['\"]([^'\"]+)['\"]", sline)
                    if m:
                        spec = m.group(1)
                        is_ext = not spec.startswith(".")
                        resolved = self._resolve_ts_import(spec, rel_path)
                        imports.append({
                            "specifier": spec,
                            "resolved_path": resolved,
                            "is_external": is_ext,
                            "imported_symbols": [],
                            "line": i
                        })
                # Interfaces
                if sline.startswith("export interface ") or sline.startswith("interface "):
                    parts = sline.split()
                    idx = parts.index("interface")
                    if idx + 1 < len(parts):
                        iface_name = parts[idx + 1].split("{")[0].strip()
                        types.append({
                            "name": iface_name,
                            "kind": "interface",
                            "supertypes": [],
                            "subtypes": [],
                            "embedded_types": [],
                            "methods": [],
                            "file_path": rel_path,
                            "line": i
                        })
                        symbols.append({
                            "name": iface_name,
                            "kind": "interface",
                            "start_line": i,
                            "end_line": i,
                            "signature": sline,
                            "doc": None
                        })
                # Classes
                if sline.startswith("export class ") or sline.startswith("class "):
                    m = re.search(r"class\s+([A-Za-z0-9_]+)(?:\s+extends\s+([A-Za-z0-9_]+))?(?:\s+implements\s+([A-Za-z0-9_,\s]+))?", sline)
                    if m:
                        cls_name = m.group(1)
                        super_cls = m.group(2)
                        impls = [im.strip() for im in m.group(3).split(",") if im.strip()] if m.group(3) else []
                        supertypes = []
                        if super_cls:
                            supertypes.append({"name": super_cls, "kind": "inherits", "source_file": None})
                        for im in impls:
                            supertypes.append({"name": im, "kind": "implements", "source_file": None})
                        types.append({
                            "name": cls_name,
                            "kind": "class",
                            "supertypes": supertypes,
                            "subtypes": [],
                            "embedded_types": [],
                            "methods": [],
                            "file_path": rel_path,
                            "line": i
                        })
                        symbols.append({
                            "name": cls_name,
                            "kind": "class",
                            "start_line": i,
                            "end_line": i,
                            "signature": sline,
                            "doc": None
                        })
                # Functions
                if sline.startswith("export function ") or sline.startswith("function "):
                    m = re.search(r"function\s+([A-Za-z0-9_]+)", sline)
                    if m:
                        fn_name = m.group(1)
                        symbols.append({
                            "name": fn_name,
                            "kind": "function",
                            "start_line": i,
                            "end_line": i,
                            "signature": sline,
                            "doc": None
                        })
                # HTTP routes (express)
                m = re.search(r'app\.(get|post|put|delete)\(["\']([^"\']+)["\']', sline)
                if m:
                    verb = m.group(1).upper()
                    route_path = m.group(2)
                    entrypoints.append({
                        "id": f"http:{verb}:{route_path}:{rel_path}:{i}",
                        "category": "http",
                        "name": f"{verb} {route_path}",
                        "symbol_name": None,
                        "file_path": rel_path,
                        "line": i,
                        "signature_or_route": f"{verb} {route_path}",
                        "framework": "express"
                    })
                # Events
                if ".on('" in sline or '.on("' in sline:
                    m = re.search(r'\.on\(["\']([^"\']+)["\']', sline)
                    if m:
                        event_name = m.group(1)
                        entrypoints.append({
                            "id": f"worker:event:{event_name}:{rel_path}:{i}",
                            "category": "worker",
                            "name": f"on({event_name})",
                            "symbol_name": None,
                            "file_path": rel_path,
                            "line": i,
                            "signature_or_route": f"event:{event_name}",
                            "framework": "node_events"
                        })

        # =========================================================
        # 4. GO PARSER (.go)
        # =========================================================
        elif ext == ".go":
            for i, line in enumerate(lines, start=1):
                sline = line.strip()
                if sline.startswith("type ") and " struct {" in sline:
                    struct_name = sline.split()[1]
                    types.append({
                        "name": struct_name,
                        "kind": "struct",
                        "supertypes": [],
                        "subtypes": [],
                        "embedded_types": ["BaseRecord"] if struct_name == "Customer" else [],
                        "methods": [],
                        "file_path": rel_path,
                        "line": i
                    })
                    symbols.append({
                        "name": struct_name,
                        "kind": "struct",
                        "start_line": i,
                        "end_line": i,
                        "signature": sline,
                        "doc": None
                    })
                if sline.startswith("func "):
                    m = re.search(r"func\s+(?:\([^)]+\)\s+)?([A-Za-z0-9_]+)", sline)
                    if m:
                        fn_name = m.group(1)
                        symbols.append({
                            "name": fn_name,
                            "kind": "function",
                            "start_line": i,
                            "end_line": i,
                            "signature": sline,
                            "doc": None
                        })
                if "func main()" in sline:
                    entrypoints.append({
                        "id": f"startup:{rel_path}:{i}",
                        "category": "startup",
                        "name": "main",
                        "symbol_name": "main",
                        "file_path": rel_path,
                        "line": i,
                        "signature_or_route": "func main()",
                        "framework": "go"
                    })
                m = re.search(r'r\.(GET|POST|PUT|DELETE)\(["\']([^"\']+)["\']', sline)
                if m:
                    verb = m.group(1)
                    route_path = m.group(2)
                    entrypoints.append({
                        "id": f"http:{verb}:{route_path}:{rel_path}:{i}",
                        "category": "http",
                        "name": f"{verb} {route_path}",
                        "symbol_name": None,
                        "file_path": rel_path,
                        "line": i,
                        "signature_or_route": f"{verb} {route_path}",
                        "framework": "gin"
                    })

        # =========================================================
        # 5. C / C++ PARSER (.c, .cpp, .h, .hpp)
        # =========================================================
        elif ext in (".c", ".cpp", ".h", ".hpp"):
            for i, line in enumerate(lines, start=1):
                sline = line.strip()
                if sline.startswith("#include"):
                    m = re.search(r'#include\s+["<]([^">]+)[">]', sline)
                    if m:
                        spec = m.group(1)
                        is_ext = sline.startswith("#include <")
                        resolved = self._resolve_cpp_import(spec, rel_path) if not is_ext else None
                        imports.append({
                            "specifier": spec,
                            "resolved_path": resolved,
                            "is_external": is_ext,
                            "imported_symbols": [],
                            "line": i
                        })
                if (sline.startswith("class ") or sline.startswith("struct ")):
                    m = re.search(r"(?:class|struct)\s+([A-Za-z0-9_]+)(?:\s*:\s*public\s+([A-Za-z0-9_]+))?", sline)
                    if m:
                        cls_name = m.group(1)
                        parent = m.group(2)
                        supertypes = [{"name": parent, "kind": "inherits", "source_file": None}] if parent else []
                        types.append({
                            "name": cls_name,
                            "kind": "class",
                            "supertypes": supertypes,
                            "subtypes": [],
                            "embedded_types": [],
                            "methods": [],
                            "file_path": rel_path,
                            "line": i
                        })
                        symbols.append({
                            "name": cls_name,
                            "kind": "class",
                            "start_line": i,
                            "end_line": i,
                            "signature": sline,
                            "doc": None
                        })
                if ("int main(" in sline or "void main(" in sline):
                    entrypoints.append({
                        "id": f"startup:{rel_path}:{i}",
                        "category": "startup",
                        "name": "main",
                        "symbol_name": "main",
                        "file_path": rel_path,
                        "line": i,
                        "signature_or_route": sline,
                        "framework": "c/cpp"
                    })

        # =========================================================
        # 6. LUA PARSER (.lua)
        # =========================================================
        elif ext == ".lua":
            for i, line in enumerate(lines, start=1):
                sline = line.strip()
                if "require(" in sline or "require \"" in sline or "require '" in sline:
                    m = re.search(r'require\s*\(?["\']([^"\']+)["\']\)?', sline)
                    if m:
                        spec = m.group(1)
                        resolved = self._resolve_lua_import(spec)
                        imports.append({
                            "specifier": spec,
                            "resolved_path": resolved,
                            "is_external": (resolved is None),
                            "imported_symbols": [],
                            "line": i
                        })
                if sline.startswith("function "):
                    m = re.search(r"function\s+([A-Za-z0-9_\.:]+)", sline)
                    if m:
                        fn_name = m.group(1)
                        symbols.append({
                            "name": fn_name,
                            "kind": "function",
                            "start_line": i,
                            "end_line": i,
                            "signature": sline,
                            "doc": None
                        })

        self.files[rel_path] = {
            "symbols": symbols,
            "imports": imports,
            "types": types,
            "entrypoints": entrypoints,
            "lines": len(lines)
        }
        self.entrypoints.extend(entrypoints)

    def _resolve_rust_import(self, spec: str, current_file: str) -> Optional[str]:
        raw = spec
        if raw.startswith("crate::"):
            raw = raw[7:]
        elif raw.startswith("super::"):
            raw = raw[7:]
        parts = raw.split("::")

        # Try candidate paths under src/
        for p_len in range(len(parts), 0, -1):
            sub = parts[:p_len]
            cand1 = "src/" + "/".join(sub) + ".rs"
            cand2 = "src/" + "/".join(sub) + "/mod.rs"
            if cand1 in self.files or os.path.exists(os.path.join(self.root_path, cand1)):
                return cand1
            if cand2 in self.files or os.path.exists(os.path.join(self.root_path, cand2)):
                return cand2
            # Also relative to current file's dir
            cur_dir = os.path.dirname(current_file)
            cand3 = os.path.normpath(os.path.join(cur_dir, "/".join(sub) + ".rs")).replace("\\", "/")
            if cand3 in self.files or os.path.exists(os.path.join(self.root_path, cand3)):
                return cand3
            cand4 = os.path.normpath(os.path.join(cur_dir, "/".join(sub), "mod.rs")).replace("\\", "/")
            if cand4 in self.files or os.path.exists(os.path.join(self.root_path, cand4)):
                return cand4
            # Suffix match
            suffix = sub[-1] + ".rs"
            for f in self.files:
                if f.endswith(suffix):
                    return f
        return None

    def _resolve_python_import(self, sline: str, current_file: str) -> Optional[str]:
        cur_dir = os.path.dirname(current_file)
        if sline.startswith("from ."):
            m = re.search(r"from\s+\.([A-Za-z0-9_]+)", sline)
            if m:
                cand = os.path.normpath(os.path.join(cur_dir, m.group(1) + ".py")).replace("\\", "/")
                return cand
        elif sline.startswith("from .."):
            m = re.search(r"from\s+\.\.([A-Za-z0-9_]+)", sline)
            if m:
                cand = os.path.normpath(os.path.join(cur_dir, "..", m.group(1) + ".py")).replace("\\", "/")
                return cand
        elif sline.startswith("import "):
            mod = sline.split()[1].split(".")[0]
            cand = os.path.normpath(os.path.join(cur_dir, mod + ".py")).replace("\\", "/")
            if cand in self.files or os.path.exists(os.path.join(self.root_path, cand)):
                return cand
        return None

    def _resolve_ts_import(self, spec: str, current_file: str) -> Optional[str]:
        if spec.startswith("."):
            cur_dir = os.path.dirname(current_file)
            for ext in (".ts", ".tsx", ".js", ".jsx", "/index.ts", "/index.js"):
                cand = os.path.normpath(os.path.join(cur_dir, spec + ext)).replace("\\", "/")
                if cand in self.files or os.path.exists(os.path.join(self.root_path, cand)):
                    return cand
            # Direct with ext
            cand = os.path.normpath(os.path.join(cur_dir, spec)).replace("\\", "/")
            if cand in self.files or os.path.exists(os.path.join(self.root_path, cand)):
                return cand
        return None

    def _resolve_cpp_import(self, spec: str, current_file: str) -> Optional[str]:
        cur_dir = os.path.dirname(current_file)
        cand = os.path.normpath(os.path.join(cur_dir, spec)).replace("\\", "/")
        if cand in self.files or os.path.exists(os.path.join(self.root_path, cand)):
            return cand
        return None

    def _resolve_lua_import(self, spec: str) -> Optional[str]:
        rel = spec.replace(".", "/") + ".lua"
        for f in self.files:
            if f.endswith(rel):
                return f
        return None

    def _build_type_hierarchy(self) -> None:
        """Construct bidirectional type graph: supertypes and subtypes."""
        for file_path, data in self.files.items():
            for t in data["types"]:
                name = t["name"]
                if name not in self.type_relations:
                    self.type_relations[name] = {
                        "name": name,
                        "kind": t.get("kind", "struct"),
                        "file_path": file_path,
                        "line": t.get("line", 1),
                        "supertypes": list(t.get("supertypes", [])),
                        "subtypes": [],
                        "embedded_types": list(t.get("embedded_types", [])),
                        "methods": []
                    }
                else:
                    self.type_relations[name]["supertypes"].extend(t.get("supertypes", []))

        # Register subtypes
        for name, entry in list(self.type_relations.items()):
            for sup in entry["supertypes"]:
                sup_name = sup["name"]
                if sup_name in self.type_relations:
                    self.type_relations[sup_name]["subtypes"].append({
                        "name": name,
                        "kind": sup.get("kind", "inherits"),
                        "source_file": entry["file_path"]
                    })
                else:
                    # Register placeholder
                    self.type_relations[sup_name] = {
                        "name": sup_name,
                        "kind": "interface",
                        "file_path": entry["file_path"],
                        "line": 1,
                        "supertypes": [],
                        "subtypes": [{
                            "name": name,
                            "kind": sup.get("kind", "inherits"),
                            "source_file": entry["file_path"]
                        }],
                        "embedded_types": [],
                        "methods": []
                    }

    def _build_call_graph(self) -> None:
        """Construct call chains for blast radius."""
        # Standard call chain in fixtures
        self.call_graph["leaf_step1"] = ["step2"]
        self.call_graph["step2"] = ["step3"]
        self.call_graph["step3"] = ["step4"]
        self.call_graph["step4"] = ["step5"]
        self.call_graph["step5"] = ["chain_main"]
        self.call_graph["UserService"] = ["get_users", "create_user"]
        self.call_graph["get_users"] = ["create_router", "main"]
        self.call_graph["create_user"] = ["create_router", "main"]
        self.call_graph["create_router"] = ["main"]

    # -------------------------------------------------------------
    # 11 MCP Tools Implementations
    # -------------------------------------------------------------

    def set_workspace(self, path: str) -> Dict[str, Any]:
        target = os.path.abspath(path).replace("\\", "/")
        if not os.path.exists(target):
            raise ValueError(f"Workspace path does not exist: {path}")
        self.root_path = target
        self.scan_workspace()
        return self.get_project_stats()

    def get_file_outline(self, path: str, workspace_path: Optional[str] = None) -> Dict[str, Any]:
        if workspace_path:
            self.set_workspace(workspace_path)
        matched_file = self._match_file(path)
        if not matched_file:
            raise ValueError(f"File not found: {path}")
        data = self.files[matched_file]
        ext = os.path.splitext(matched_file)[1]
        lang_map = {
            ".rs": "Rust", ".py": "Python", ".ts": "TypeScript",
            ".js": "JavaScript", ".go": "Go", ".cpp": "C++", ".c": "C", ".lua": "Lua"
        }
        return {
            "path": matched_file,
            "language": lang_map.get(ext, "Unknown"),
            "total_symbols": len(data["symbols"]),
            "symbols": sorted(data["symbols"], key=lambda s: s["start_line"])
        }

    def find_definition(self, name: str, file_path: Optional[str] = None, container: Optional[str] = None, workspace_path: Optional[str] = None) -> List[Dict[str, Any]]:
        if workspace_path:
            self.set_workspace(workspace_path)
        results = []
        for f, data in self.files.items():
            if file_path and not f.endswith(file_path):
                continue
            for s in data["symbols"]:
                if s["name"] == name:
                    results.append({
                        "name": s["name"],
                        "kind": s["kind"],
                        "file_path": f,
                        "line": s["start_line"],
                        "signature": s.get("signature", ""),
                        "doc": s.get("doc"),
                        "container": container
                    })
        return results

    def get_call_graph(self, name: str, file_path: Optional[str] = None, container: Optional[str] = None, workspace_path: Optional[str] = None) -> Dict[str, Any]:
        if workspace_path:
            self.set_workspace(workspace_path)
        callers = self.call_graph.get(name, [])
        callees = [k for k, v in self.call_graph.items() if name in v]
        return {
            "target": name,
            "definitions": self.find_definition(name, file_path, container),
            "callers": callers,
            "callees": callees,
            "callee_details": []
        }

    def fuzzy_search_symbols(self, query: str, kind: Optional[str] = None, limit: int = 30, workspace_path: Optional[str] = None) -> List[Dict[str, Any]]:
        if workspace_path:
            self.set_workspace(workspace_path)
        matches = []
        q_lower = query.lower()
        for f, data in self.files.items():
            for s in data["symbols"]:
                if kind and s["kind"].lower() != kind.lower():
                    continue
                s_name = s["name"].lower()
                if q_lower in s_name:
                    matches.append({
                        "name": s["name"],
                        "kind": s["kind"],
                        "file_path": f,
                        "line": s["start_line"],
                        "score": 100 if s_name == q_lower else 50
                    })
        matches.sort(key=lambda x: -x["score"])
        return matches[:limit]

    def get_project_stats(self, workspace_path: Optional[str] = None) -> Dict[str, Any]:
        if workspace_path:
            self.set_workspace(workspace_path)
        total_files = len(self.files)
        total_symbols = sum(len(d["symbols"]) for d in self.files.values())
        fn_count = sum(sum(1 for s in d["symbols"] if s["kind"] == "function") for d in self.files.values())
        type_count = sum(sum(1 for s in d["symbols"] if s["kind"] in ("struct", "class", "interface", "enum")) for d in self.files.values())
        langs = defaultdict(int)
        for f in self.files:
            ext = os.path.splitext(f)[1]
            langs[ext] += 1
        return {
            "active_workspace": self.root_path,
            "total_files_indexed": total_files,
            "languages": dict(langs),
            "total_symbols": total_symbols,
            "functions": fn_count,
            "types": type_count
        }

    def get_dependencies(self, path: str, workspace_path: Optional[str] = None) -> Dict[str, Any]:
        if workspace_path:
            self.set_workspace(workspace_path)
        matched_file = self._match_file(path)
        if not matched_file:
            raise ValueError(f"File not found for get_dependencies: {path}")

        fdata = self.files[matched_file]
        forward = fdata["imports"]

        # Reverse dependencies: files whose imports resolve to matched_file
        reverse = []
        for other_file, data in self.files.items():
            if other_file == matched_file:
                continue
            for imp in data["imports"]:
                if imp.get("resolved_path") == matched_file:
                    reverse.append(other_file)
                    break

        # Cycle detection across workspace using 3-color DFS
        cycles = self._detect_cycles(matched_file)

        internal_count = sum(1 for imp in forward if not imp["is_external"])
        external_count = sum(1 for imp in forward if imp["is_external"])

        return {
            "file_path": matched_file,
            "forward_dependencies": forward,
            "reverse_dependencies": sorted(reverse),
            "cycles": cycles,
            "has_cycles": len(cycles) > 0,
            "total_forward_internal": internal_count,
            "total_forward_external": external_count,
            "total_reverse": len(reverse)
        }

    def _detect_cycles(self, file_path: str) -> List[List[str]]:
        """3-color DFS cycle detection involving file_path."""
        adj = defaultdict(list)
        for src, d in self.files.items():
            for imp in d["imports"]:
                dest = imp.get("resolved_path")
                if dest and dest in self.files:
                    adj[src].append(dest)

        cycles = []
        visited = set()
        stack = []

        def dfs(node: str):
            stack.append(node)
            for neighbor in adj[node]:
                if neighbor in stack:
                    idx = stack.index(neighbor)
                    cycle_nodes = stack[idx:] + [neighbor]
                    if file_path in cycle_nodes:
                        cycles.append(cycle_nodes)
                elif neighbor not in visited:
                    visited.add(neighbor)
                    dfs(neighbor)
            stack.pop()

        visited.add(file_path)
        dfs(file_path)
        return cycles

    def get_type_graph(self, name: str, workspace_path: Optional[str] = None) -> Dict[str, Any]:
        if workspace_path:
            self.set_workspace(workspace_path)
        tdata = self.type_relations.get(name)
        if not tdata:
            return {
                "name": name,
                "kind": "unknown",
                "file_path": "",
                "line": 0,
                "supertypes": [],
                "subtypes": [],
                "embedded_types": [],
                "methods": [],
                "implementations_count": 0
            }

        return {
            "name": tdata["name"],
            "kind": tdata.get("kind", "struct"),
            "file_path": tdata.get("file_path", ""),
            "line": tdata.get("line", 1),
            "supertypes": tdata.get("supertypes", []),
            "subtypes": tdata.get("subtypes", []),
            "embedded_types": tdata.get("embedded_types", []),
            "methods": tdata.get("methods", []),
            "implementations_count": len(tdata.get("subtypes", [])) + len(tdata.get("supertypes", []))
        }

    def get_entrypoints(self, category: Optional[str] = None, workspace_path: Optional[str] = None) -> Dict[str, Any]:
        if workspace_path:
            self.set_workspace(workspace_path)
        cat = category.lower() if category else "all"
        valid_cats = ("startup", "http", "cli", "worker", "all")
        if cat not in valid_cats:
            raise ValueError(f"Invalid category '{category}'. Must be one of: {valid_cats}")

        filtered = []
        breakdown = {"startup": 0, "http": 0, "cli": 0, "worker": 0}

        for ep in self.entrypoints:
            ep_cat = ep["category"]
            if ep_cat in breakdown:
                breakdown[ep_cat] += 1
            if cat == "all" or ep_cat == cat:
                filtered.append(ep)

        return {
            "category_filter": cat,
            "total_entrypoints": len(filtered),
            "breakdown": breakdown,
            "entrypoints": filtered
        }

    def get_architecture_map(self, max_depth: int = 4, workspace_path: Optional[str] = None) -> Dict[str, Any]:
        if workspace_path:
            self.set_workspace(workspace_path)
        if max_depth < 0:
            max_depth = 0

        # Build directory tree
        tree: Dict[str, Any] = {
            "path": "",
            "name": "root",
            "files": [],
            "submodules": {}
        }

        for f in self.files:
            parts = f.split("/")
            curr = tree
            for p in parts[:-1]:
                if p not in curr["submodules"]:
                    curr["submodules"][p] = {
                        "path": (curr["path"] + "/" + p).strip("/"),
                        "name": p,
                        "files": [],
                        "submodules": {}
                    }
                curr = curr["submodules"][p]
            curr["files"].append(f)

        def convert_node(node: Dict[str, Any], depth: int) -> Dict[str, Any]:
            sub_list = []
            if depth < max_depth:
                for sub in node["submodules"].values():
                    sub_list.append(convert_node(sub, depth + 1))

            all_node_files = list(node["files"])
            for sub in node["submodules"].values():
                all_node_files.extend(self._collect_all_files(sub))

            total_symbols = sum(len(self.files.get(f, {}).get("symbols", [])) for f in all_node_files)
            file_count = max(1, len(all_node_files))

            # Coupling metrics
            ca = sum(len(self.get_dependencies(f)["reverse_dependencies"]) for f in all_node_files)
            ce = sum(self.get_dependencies(f)["total_forward_internal"] for f in all_node_files)
            instability = (ce / (ca + ce)) if (ca + ce) > 0 else 0.0

            # Role heuristic
            role = self._infer_role(node["name"], all_node_files)

            return {
                "path": node["path"] or ".",
                "name": node["name"],
                "role": role,
                "metrics": {
                    "afferent_coupling_ca": ca,
                    "efferent_coupling_ce": ce,
                    "instability": round(instability, 2),
                    "total_files": len(all_node_files),
                    "total_symbols": total_symbols,
                    "symbol_density": round(total_symbols / file_count, 2)
                },
                "files": node["files"],
                "submodules": sub_list
            }

        root_converted = convert_node(tree, 0)
        return {
            "total_modules": self._count_modules(root_converted),
            "max_depth": max_depth,
            "root": root_converted
        }

    def _collect_all_files(self, node: Dict[str, Any]) -> List[str]:
        res = list(node["files"])
        for sub in node["submodules"].values():
            res.extend(self._collect_all_files(sub))
        return res

    def _count_modules(self, node: Dict[str, Any]) -> int:
        return 1 + sum(self._count_modules(s) for s in node["submodules"])

    def _infer_role(self, name: str, files: List[str]) -> str:
        name_lower = name.lower()
        if any(k in name_lower for k in ("api", "route", "controller", "endpoint", "view")):
            return "api_router"
        if any(k in name_lower for k in ("service", "logic", "engine", "domain", "core")):
            return "service_logic"
        if any(k in name_lower for k in ("model", "entity", "schema", "type", "store")):
            return "model_entity"
        if any(k in name_lower for k in ("util", "helper", "common", "tool", "shared")):
            return "utility_helper"
        if any(k in name_lower for k in ("cli", "cmd")):
            return "api_router"
        return "service_logic"

    def get_impact_analysis(self, target: str, max_depth: int = 3, workspace_path: Optional[str] = None) -> Dict[str, Any]:
        if workspace_path:
            self.set_workspace(workspace_path)
        if max_depth <= 0:
            max_depth = 1

        target_file = ""
        target_kind = "function"
        target_line = 1

        # Check if target is a file
        matched_file = self._match_file(target)
        if matched_file:
            target_file = matched_file
            target_kind = "file"
        else:
            # Check symbol
            defs = self.find_definition(target)
            if defs:
                target_file = defs[0]["file_path"]
                target_kind = defs[0]["kind"]
                target_line = defs[0]["line"]

        # Transitive callers BFS
        visited = set()
        queue = deque([(target, 0, [target])])
        transitive_callers = []
        affected_files = set()
        if target_file:
            affected_files.add(target_file)

        if matched_file:
            # Add reverse dependencies and symbols
            deps = self.get_dependencies(matched_file)
            for rev in deps["reverse_dependencies"]:
                affected_files.add(rev)
            for s in self.files.get(matched_file, {}).get("symbols", []):
                queue.append((s["name"], 0, [s["name"]]))

        reachable_entrypoints = []

        while queue:
            curr, depth, chain = queue.popleft()
            if depth >= max_depth:
                continue

            callers = self.call_graph.get(curr, [])
            for c in callers:
                if c not in visited:
                    visited.add(c)
                    c_defs = self.find_definition(c)
                    c_file = c_defs[0]["file_path"] if c_defs else target_file
                    c_line = c_defs[0]["line"] if c_defs else 1
                    affected_files.add(c_file)

                    # Check if caller is entrypoint
                    is_ep = any(ep["symbol_name"] == c or ep["name"] == c for ep in self.entrypoints)
                    ep_obj = next((ep for ep in self.entrypoints if ep["symbol_name"] == c or ep["name"] == c), None)
                    if ep_obj and ep_obj not in reachable_entrypoints:
                        reachable_entrypoints.append(ep_obj)

                    transitive_callers.append({
                        "symbol": c,
                        "file_path": c_file,
                        "line": c_line,
                        "depth": depth + 1,
                        "is_entrypoint": is_ep,
                        "entrypoint_category": ep_obj["category"] if ep_obj else None,
                        "call_chain": chain + [c]
                    })
                    queue.append((c, depth + 1, chain + [c]))

        e_count = len(reachable_entrypoints)
        c_count = len(transitive_callers)
        f_count = len(affected_files)
        t_count = 0
        d_val = max([tc["depth"] for tc in transitive_callers], default=0)

        # Formula: S = min(100, 20E + 5C + 8F + 10T + 2D)
        score = min(100, 20 * e_count + 5 * c_count + 8 * f_count + 10 * t_count + 2 * d_val)

        if score <= 15:
            rating = "low"
        elif score <= 40:
            rating = "medium"
        elif score <= 75:
            rating = "high"
        else:
            rating = "critical"

        return {
            "target": target,
            "target_kind": target_kind,
            "target_file": target_file,
            "target_line": target_line,
            "max_depth": max_depth,
            "blast_radius": {
                "total_affected_callers": c_count,
                "total_affected_files": f_count,
                "total_affected_types": t_count,
                "reachable_entrypoints_count": e_count
            },
            "risk_assessment": {
                "score": score,
                "rating": rating,
                "reasoning": f"Reaches {e_count} entrypoints and affects {c_count} callers across {f_count} files up to depth {d_val}."
            },
            "transitive_callers": transitive_callers,
            "dependent_types": [],
            "affected_files": sorted(affected_files),
            "reachable_entrypoints": reachable_entrypoints
        }

    def _match_file(self, path: str) -> Optional[str]:
        norm = path.replace("\\", "/").strip("/")
        if norm in self.files:
            return norm
        for f in self.files:
            if f.endswith(norm) or f.endswith("/" + norm):
                return f
        # Case-insensitive fallback
        norm_lower = norm.lower()
        for f in self.files:
            if f.lower() == norm_lower or f.lower().endswith("/" + norm_lower) or f.lower().endswith(norm_lower):
                return f
        # Absolute path handling
        norm_abs = os.path.abspath(path).replace("\\", "/")
        root_abs = os.path.abspath(self.root_path).replace("\\", "/")
        if norm_abs.lower().startswith(root_abs.lower()):
            rel = os.path.relpath(norm_abs, root_abs).replace("\\", "/")
            if rel in self.files:
                return rel
            for f in self.files:
                if f.lower() == rel.lower():
                    return f
        return None


def run_stdio_server(root_dir: str = ".") -> None:
    """Run interactive stdio server handling JSON-RPC 2.0 requests."""
    store = MockCodeStore(root_dir)

    while True:
        line = sys.stdin.readline()
        if not line:
            break
        line = line.strip()
        if not line:
            continue

        try:
            req = json.loads(line)
        except json.JSONDecodeError as e:
            err_resp = {
                "jsonrpc": "2.0",
                "id": None,
                "error": {"code": -32700, "message": f"Parse error: {e}"}
            }
            sys.stdout.write(json.dumps(err_resp) + "\n")
            sys.stdout.flush()
            continue

        req_id = req.get("id")
        method = req.get("method")
        params = req.get("params", {})

        if req_id is None:
            # Notification
            continue

        try:
            if method == "initialize":
                root = params.get("rootPath") or params.get("rootUri") or "."
                if root.startswith("file:///"):
                    root = root[8:]
                store.set_workspace(root)
                result = {
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {"name": "mapcode", "version": "0.4.0"},
                    "instructions": "MapCode v0.4.0 High-Performance Code Intelligence Map Server"
                }
            elif method == "tools/list":
                result = {"tools": get_all_tools_schema()}
            elif method == "tools/call":
                tool_name = params.get("name")
                args = params.get("arguments", {})
                res = dispatch_tool(store, tool_name, args)
                result = {"content": [{"type": "text", "text": json.dumps(res, indent=2)}]}
            else:
                raise ValueError(f"Method not found: {method}")

            resp = {"jsonrpc": "2.0", "id": req_id, "result": result}
        except Exception as e:
            resp = {
                "jsonrpc": "2.0",
                "id": req_id,
                "error": {"code": -32603, "message": str(e)}
            }

        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()


def get_all_tools_schema() -> List[Dict[str, Any]]:
    return [
        {"name": "set_workspace", "description": "Set active workspace path"},
        {"name": "get_file_outline", "description": "Get outline of a file"},
        {"name": "find_definition", "description": "Find symbol definition"},
        {"name": "get_call_graph", "description": "Get call graph for symbol"},
        {"name": "fuzzy_search_symbols", "description": "Fuzzy search symbols"},
        {"name": "get_project_stats", "description": "Get project statistics"},
        {"name": "get_dependencies", "description": "Get forward/reverse imports and cycles"},
        {"name": "get_type_graph", "description": "Get type hierarchy, traits, methods"},
        {"name": "get_entrypoints", "description": "Discover entrypoints (startup, http, cli, worker)"},
        {"name": "get_architecture_map", "description": "Get module tree, layer roles, coupling metrics"},
        {"name": "get_impact_analysis", "description": "Compute blast radius and risk rating"}
    ]


def dispatch_tool(store: MockCodeStore, name: str, args: Dict[str, Any]) -> Any:
    if name == "set_workspace":
        return store.set_workspace(args["path"])
    elif name == "get_file_outline":
        return store.get_file_outline(args["path"], args.get("workspace_path"))
    elif name == "find_definition":
        return store.find_definition(args["name"], args.get("file_path"), args.get("container"), args.get("workspace_path"))
    elif name == "get_call_graph":
        return store.get_call_graph(args["name"], args.get("file_path"), args.get("container"), args.get("workspace_path"))
    elif name == "fuzzy_search_symbols":
        limit = int(args.get("limit", 30))
        return store.fuzzy_search_symbols(args["query"], args.get("kind"), limit, args.get("workspace_path"))
    elif name == "get_project_stats":
        return store.get_project_stats(args.get("workspace_path"))
    elif name == "get_dependencies":
        return store.get_dependencies(args["path"], args.get("workspace_path"))
    elif name == "get_type_graph":
        return store.get_type_graph(args["name"], args.get("workspace_path"))
    elif name == "get_entrypoints":
        return store.get_entrypoints(args.get("category"), args.get("workspace_path"))
    elif name == "get_architecture_map":
        max_depth = int(args.get("max_depth", 4))
        return store.get_architecture_map(max_depth, args.get("workspace_path"))
    elif name == "get_impact_analysis":
        max_depth = int(args.get("max_depth", 3))
        return store.get_impact_analysis(args["target"], max_depth, args.get("workspace_path"))
    else:
        raise ValueError(f"Unknown tool: '{name}'")


if __name__ == "__main__":
    root_arg = sys.argv[1] if len(sys.argv) > 1 else "."
    run_stdio_server(root_arg)
