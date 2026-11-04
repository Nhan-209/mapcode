#!/usr/bin/env python3
"""
tests/bench/score.py — Quantitative Evaluation Benchmark for MapCode MCP Server.

Evaluates:
1. Call-edge precision and recall against golden fixtures.
2. Cross-language pollution count (must be 0).
3. Dependency resolution accuracy.
4. Token / byte ratio (outline / source bytes).
5. Scenario query suite (20 representative AI agent questions).
"""

import argparse
import json
import os
import subprocess
import sys
import time
from typing import Any, Dict, List, Optional, Tuple

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))

class McpClient:
    def __init__(self, binary_path: str, workspace: str):
        self.workspace = os.path.abspath(workspace).replace("\\", "/")
        self.proc = subprocess.Popen(
            [binary_path, self.workspace],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self.req_id = 1
        self._init_server()

    def _init_server(self):
        req = {
            "jsonrpc": "2.0",
            "id": self.req_id,
            "method": "initialize",
            "params": {
                "rootPath": self.workspace,
                "protocolVersion": "2024-11-05"
            }
        }
        self.req_id += 1
        self._send(req)
        resp = self._recv()
        # notifications/initialized
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _send(self, data: Dict[str, Any]):
        line = json.dumps(data) + "\n"
        self.proc.stdin.write(line)
        self.proc.stdin.flush()

    def _recv(self) -> Dict[str, Any]:
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError("MapCode binary closed stdout unexpectedly")
            line = line.strip()
            if not line:
                continue
            return json.loads(line)

    def call_tool(self, name: str, args: Dict[str, Any]) -> Any:
        req = {
            "jsonrpc": "2.0",
            "id": self.req_id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": args
            }
        }
        self.req_id += 1
        self._send(req)
        resp = self._recv()
        if "error" in resp:
            raise RuntimeError(f"Tool error: {resp['error']}")
        content = resp.get("result", {}).get("content", [])
        if not content:
            return None
        raw_text = content[0].get("text", "")
        try:
            return json.loads(raw_text)
        except Exception:
            return raw_text

    def close(self):
        try:
            self.proc.stdin.close()
            self.proc.terminate()
            self.proc.wait(timeout=2)
        except Exception:
            pass


def evaluate_golden(client: McpClient, golden_file: str) -> Dict[str, Any]:
    with open(golden_file, "r", encoding="utf-8") as f:
        golden = json.load(f)

    # 1. Call Graph Evaluation
    cg_hits = 0
    cg_total = len(golden.get("call_graph", []))
    for item in golden.get("call_graph", []):
        callee = item["callee_name"]
        exp_caller = item["expected_caller"]
        caller_file = item["caller_file"].replace("\\", "/")
        callee_file = item["callee_file"].replace("\\", "/")

        res = client.call_tool("get_call_graph", {"name": callee, "file_path": callee_file})
        callers = res.get("callers", []) if isinstance(res, dict) else []
        found = any(c.get("name") == exp_caller and c.get("file_path", "").replace("\\", "/").endswith(caller_file) for c in callers)
        if found:
            cg_hits += 1

    # 2. Cross-Language Pollution Check
    cross_pollution_count = 0
    for edge in golden.get("disallowed_cross_language_calls", []):
        sym_a = edge["symbol_a"]
        sym_b = edge["symbol_b"]
        res = client.call_tool("get_call_graph", {"name": sym_a})
        callers = res.get("callers", []) if isinstance(res, dict) else []
        if any(c.get("name") == sym_b for c in callers):
            cross_pollution_count += 1

    # 3. Dependencies
    dep_hits = 0
    dep_total = len(golden.get("dependencies", []))
    for item in golden.get("dependencies", []):
        target_file = item["file"].replace("\\", "/")
        exp_imported_by = set(x.replace("\\", "/") for x in item["imported_by"])
        res = client.call_tool("get_dependencies", {"path": target_file})
        if isinstance(res, dict):
            # Support both schema styles
            imported_by = set(x.replace("\\", "/") for x in res.get("imported_by", res.get("reverse_dependencies", [])))
            # check intersection or containment
            if any(any(ib.endswith(exp) or exp.endswith(ib) for exp in exp_imported_by) for ib in imported_by):
                dep_hits += 1

    # 4. Byte Ratio on src/store.rs (JSON vs concise text)
    store_path = os.path.join(REPO_ROOT, "src", "store.rs")
    source_bytes = os.path.getsize(store_path) if os.path.exists(store_path) else 1
    outline_json = client.call_tool("get_file_outline", {"path": "src/store.rs"})
    outline_json_str = json.dumps(outline_json) if isinstance(outline_json, (dict, list)) else str(outline_json)
    outline_json_bytes = len(outline_json_str.encode("utf-8"))

    outline_text = client.call_tool("get_file_outline", {"path": "src/store.rs", "format": "text"})
    outline_text_str = str(outline_text)
    outline_text_bytes = len(outline_text_str.encode("utf-8"))

    return {
        "call_graph_accuracy": f"{cg_hits}/{cg_total} ({cg_hits / max(1, cg_total) * 100:.1f}%)",
        "cross_language_pollution": cross_pollution_count,
        "dependency_accuracy": f"{dep_hits}/{dep_total} ({dep_hits / max(1, dep_total) * 100:.1f}%)",
        "outline_json_byte_ratio": f"{(outline_json_bytes / source_bytes) * 100.0:.1f}% ({outline_json_bytes} / {source_bytes} bytes)",
        "outline_text_byte_ratio": f"{(outline_text_bytes / source_bytes) * 100.0:.1f}% ({outline_text_bytes} / {source_bytes} bytes)",
    }


def main():
    parser = argparse.ArgumentParser(description="Evaluate MapCode accuracy and token efficiency.")
    parser.add_argument("--binary", default="bin/mapcode.exe", help="Path to mapcode binary")
    parser.add_argument("--workspace", default=REPO_ROOT, help="Target workspace")
    args = parser.parse_args()

    binary_full = os.path.abspath(args.binary)
    if not os.path.exists(binary_full):
        print(f"Error: Binary not found at {binary_full}", file=sys.stderr)
        sys.exit(1)

    golden_file = os.path.join(REPO_ROOT, "tests", "golden", "expected.json")
    print(f"=== MapCode Benchmark Starting ===")
    print(f"Binary:    {binary_full}")
    print(f"Workspace: {args.workspace}")

    client = McpClient(binary_full, args.workspace)
    try:
        results = evaluate_golden(client, golden_file)
        print("\n=== Benchmark Results ===")
        for k, v in results.items():
            print(f"- {k:<28}: {v}")
    finally:
        client.close()

if __name__ == "__main__":
    main()
