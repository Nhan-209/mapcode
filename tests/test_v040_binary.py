import subprocess
import json
import os
import sys
import time

BINARY_PATH = r"D:\laptrinh\duan\mapcode\bin\mapcode.exe"
WORKSPACE = r"D:\laptrinh\duan\mapcode"

def run_test():
    print(f"Testing binary: {BINARY_PATH}")
    proc = subprocess.Popen(
        [BINARY_PATH],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0
    )

    req_id = 1
    def send_recv(method, params=None):
        nonlocal req_id
        payload = {
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method
        }
        if params is not None:
            payload["params"] = params
        req_id += 1
        data = json.dumps(payload) + "\n"
        proc.stdin.write(data)
        proc.stdin.flush()
        line = proc.stdout.readline()
        if not line:
            err = proc.stderr.read()
            raise RuntimeError(f"Server closed connection. Stderr: {err}")
        return json.loads(line)

    try:
        # 1. initialize
        init_res = send_recv("initialize", {
            "protocolVersion": "2024-11-05",
            "clientInfo": {"name": "test-client", "version": "1.0"},
            "capabilities": {}
        })
        print("[✓] Initialized successfully:", init_res.get("result", {}).get("serverInfo"))

        # 2. tools/list
        tools_res = send_recv("tools/list")
        tool_names = [t["name"] for t in tools_res.get("result", {}).get("tools", [])]
        print(f"[✓] Tools found ({len(tool_names)}): {tool_names}")
        expected_tools = [
            "set_workspace", "get_file_outline", "find_definition", "get_call_graph",
            "fuzzy_search_symbols", "get_project_stats", "get_dependencies",
            "get_type_graph", "get_entrypoints", "get_architecture_map", "get_impact_analysis"
        ]
        for t in expected_tools:
            assert t in tool_names, f"Missing tool: {t}"
        print("[✓] All 11 expected MCP tools are registered!")

        # 3. set_workspace
        sw_res = send_recv("tools/call", {
            "name": "set_workspace",
            "arguments": {"path": WORKSPACE}
        })
        print("[✓] set_workspace response:", sw_res.get("result", {}).get("content", [{}])[0].get("text", "")[:120])

        # Give background indexer a moment to finish indexing
        time.sleep(2)

        # 4. get_project_stats
        stats_res = send_recv("tools/call", {
            "name": "get_project_stats",
            "arguments": {}
        })
        print("[✓] Project stats:\n", stats_res.get("result", {}).get("content", [{}])[0].get("text", "")[:300])

        # 5. get_dependencies
        dep_res = send_recv("tools/call", {
            "name": "get_dependencies",
            "arguments": {"path": "src/main.rs"}
        })
        print("[✓] Dependencies of src/main.rs:\n", dep_res.get("result", {}).get("content", [{}])[0].get("text", "")[:300])

        # 6. get_type_graph
        tg_res = send_recv("tools/call", {
            "name": "get_type_graph",
            "arguments": {"name": "CodeStore"}
        })
        print("[✓] Type graph for CodeStore:\n", tg_res.get("result", {}).get("content", [{}])[0].get("text", "")[:300])

        # 7. get_entrypoints
        ep_res = send_recv("tools/call", {
            "name": "get_entrypoints",
            "arguments": {}
        })
        print("[✓] Entrypoints:\n", ep_res.get("result", {}).get("content", [{}])[0].get("text", "")[:300])

        # 8. get_architecture_map
        arch_res = send_recv("tools/call", {
            "name": "get_architecture_map",
            "arguments": {}
        })
        print("[✓] Architecture Map:\n", arch_res.get("result", {}).get("content", [{}])[0].get("text", "")[:400])

        # 9. get_impact_analysis
        impact_res = send_recv("tools/call", {
            "name": "get_impact_analysis",
            "arguments": {"target": "CodeStore"}
        })
        print("[✓] Impact Analysis for CodeStore:\n", impact_res.get("result", {}).get("content", [{}])[0].get("text", "")[:400])

        # Check .mapcode/cache.json
        cache_file = os.path.join(WORKSPACE, ".mapcode", "cache.json")
        if os.path.exists(cache_file):
            size = os.path.getsize(cache_file)
            print(f"[✓] Persistent cache exists at {cache_file} (Size: {size} bytes)")
        else:
            print("[!] Cache file not yet written (will be synced upon shutdown or debounce)")

    finally:
        proc.terminate()
        try:
            proc.wait(timeout=2)
        except Exception:
            proc.kill()

if __name__ == "__main__":
    run_test()
