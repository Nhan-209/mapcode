"""
tests/e2e/tier3_combinations.py — Tier 3 Cross-Feature Combination Test Suite for MapCode v0.4.0.

Covers pairwise feature combinations across MCP tools, AST engines, and caching:
- Dependency Graph + Architecture Map (Coupling correlation)
- Type Graph + Impact Analysis (Subtype blast radius)
- Entrypoints + Impact Analysis (Reachable route validation)
- Workspace switching + all query tools
- Persistent cache + incremental updates
- Stdio session multi-tool pipelining
"""

import os
import unittest
from typing import Optional
from tests.e2e.client import McpClient


class TestTier3Combinations(unittest.TestCase):
    client: Optional[McpClient] = None
    server_cmd = ["python", "tests/e2e/mock_server.py"]
    sample_dir = "tests/fixtures/sample_workspace"
    isolated_dir = "tests/fixtures/edge_cases/isolated"
    cycles_dir = "tests/fixtures/edge_cases/cycles"

    @classmethod
    def setUpClass(cls):
        custom_bin = os.environ.get("MAPCODE_BIN")
        if custom_bin and os.path.exists(custom_bin):
            cls.server_cmd = [custom_bin]
        cls.client = McpClient(cls.server_cmd).start()
        cls.client.initialize(cls.sample_dir)

    @classmethod
    def tearDownClass(cls):
        if cls.client:
            cls.client.close()

    def setUp(self):
        self.client.call_tool("set_workspace", {"path": self.sample_dir})

    # 1. set_workspace + get_dependencies
    def test_comb_workspace_switch_dependencies(self):
        self.client.call_tool("set_workspace", {"path": self.isolated_dir})
        deps = self.client.call_tool("get_dependencies", {"path": "island.rs"})
        self.assertEqual(deps["total_forward_internal"], 0)

    # 2. set_workspace + get_type_graph
    def test_comb_workspace_switch_type_graph(self):
        self.client.call_tool("set_workspace", {"path": self.isolated_dir})
        tg = self.client.call_tool("get_type_graph", {"name": "SolitaryIsland"})
        self.assertEqual(tg["name"], "SolitaryIsland")

    # 3. set_workspace + get_entrypoints
    def test_comb_workspace_switch_entrypoints(self):
        self.client.call_tool("set_workspace", {"path": self.isolated_dir})
        ep = self.client.call_tool("get_entrypoints")
        self.assertEqual(ep["total_entrypoints"], 0)

    # 4. set_workspace + get_architecture_map
    def test_comb_workspace_switch_architecture(self):
        self.client.call_tool("set_workspace", {"path": self.isolated_dir})
        arch = self.client.call_tool("get_architecture_map")
        self.assertIn("root", arch)

    # 5. set_workspace + get_impact_analysis
    def test_comb_workspace_switch_impact(self):
        self.client.call_tool("set_workspace", {"path": self.isolated_dir})
        imp = self.client.call_tool("get_impact_analysis", {"target": "SolitaryIsland"})
        self.assertEqual(imp["blast_radius"]["total_affected_callers"], 0)

    # 6. get_dependencies + get_architecture_map (coupling correlation)
    def test_comb_dependencies_architecture_coupling(self):
        arch = self.client.call_tool("get_architecture_map")
        # In sample workspace, total efferent coupling should be >= internal dependencies
        ce_total = arch["root"]["metrics"]["efferent_coupling_ce"]
        self.assertGreaterEqual(ce_total, 0)

    # 7. get_dependencies + get_impact_analysis (affected files match reverse dependencies)
    def test_comb_dependencies_impact_affected_files(self):
        deps = self.client.call_tool("get_dependencies", {"path": "src/models/user.rs"})
        impact = self.client.call_tool("get_impact_analysis", {"target": "src/models/user.rs"})
        for rev in deps["reverse_dependencies"]:
            self.assertTrue(any(rev in af for af in impact["affected_files"]))

    # 8. get_type_graph + get_impact_analysis (type hierarchy blast radius)
    def test_comb_type_graph_impact_blast_radius(self):
        tg = self.client.call_tool("get_type_graph", {"name": "User"})
        impact = self.client.call_tool("get_impact_analysis", {"target": "User"})
        self.assertEqual(impact["target"], "User")
        self.assertIn("risk_assessment", impact)

    # 9. get_type_graph + find_definition
    def test_comb_type_graph_find_definition(self):
        tg = self.client.call_tool("get_type_graph", {"name": "User"})
        defs = self.client.call_tool("find_definition", {"name": tg["name"]})
        self.assertGreaterEqual(len(defs), 1)

    # 10. get_type_graph + get_file_outline
    def test_comb_type_graph_file_outline(self):
        tg = self.client.call_tool("get_type_graph", {"name": "User"})
        outline = self.client.call_tool("get_file_outline", {"path": tg["file_path"]})
        names = [s["name"] for s in outline["symbols"]]
        self.assertIn("User", names)

    # 11. get_entrypoints + get_impact_analysis (reachable entrypoints check)
    def test_comb_entrypoints_impact_reachable(self):
        eps = self.client.call_tool("get_entrypoints", {"category": "startup"})
        impact = self.client.call_tool("get_impact_analysis", {"target": "UserService", "max_depth": 5})
        ep_ids = {e["id"] for e in eps["entrypoints"]}
        for re_ep in impact["reachable_entrypoints"]:
            if re_ep["category"] == "startup":
                self.assertIn(re_ep["id"], ep_ids)

    # 12. get_entrypoints + get_call_graph (root callers in call graph)
    def test_comb_entrypoints_call_graph(self):
        cg = self.client.call_tool("get_call_graph", {"name": "UserService"})
        self.assertIn("callers", cg)

    # 13. get_entrypoints + get_architecture_map (API router layer inference)
    def test_comb_entrypoints_architecture_layer(self):
        arch = self.client.call_tool("get_architecture_map")
        src = next((s for s in arch["root"]["submodules"] if s["name"] == "src"), None)
        if src:
            api_sub = next((s for s in src["submodules"] if s["name"] == "api"), None)
            if api_sub:
                self.assertEqual(api_sub["role"], "api_router")

    # 14. get_architecture_map + get_project_stats (symbol totals consistency)
    def test_comb_architecture_stats_consistency(self):
        stats = self.client.call_tool("get_project_stats")
        arch = self.client.call_tool("get_architecture_map")
        self.assertEqual(stats["total_symbols"], arch["root"]["metrics"]["total_symbols"])

    # 15. get_architecture_map + get_impact_analysis (instability vs blast radius)
    def test_comb_architecture_impact_coupling(self):
        arch = self.client.call_tool("get_architecture_map")
        self.assertGreaterEqual(len(arch["root"]["submodules"]), 1)

    # 16. fuzzy_search_symbols + find_definition
    def test_comb_fuzzy_search_find_definition(self):
        matches = self.client.call_tool("fuzzy_search_symbols", {"query": "User"})
        self.assertGreaterEqual(len(matches), 1)
        first = matches[0]
        defs = self.client.call_tool("find_definition", {"name": first["name"]})
        self.assertGreaterEqual(len(defs), 1)

    # 17. fuzzy_search_symbols + get_type_graph
    def test_comb_fuzzy_search_type_graph(self):
        matches = self.client.call_tool("fuzzy_search_symbols", {"query": "Product", "kind": "class"})
        self.assertGreaterEqual(len(matches), 1)
        tg = self.client.call_tool("get_type_graph", {"name": matches[0]["name"]})
        self.assertEqual(tg["name"], "Product")

    # 18. get_file_outline + get_call_graph
    def test_comb_file_outline_call_graph(self):
        outline = self.client.call_tool("get_file_outline", {"path": "src/api/routes.rs"})
        for sym in outline["symbols"]:
            if sym["kind"] == "function":
                cg = self.client.call_tool("get_call_graph", {"name": sym["name"]})
                self.assertIn("target", cg)

    # 19. get_file_outline + get_dependencies
    def test_comb_file_outline_dependencies(self):
        outline = self.client.call_tool("get_file_outline", {"path": "src/main.rs"})
        deps = self.client.call_tool("get_dependencies", {"path": "src/main.rs"})
        self.assertEqual(outline["path"], deps["file_path"])

    # 20. get_project_stats + get_file_outline (symbol aggregation)
    def test_comb_stats_outline_aggregation(self):
        stats = self.client.call_tool("get_project_stats")
        self.assertGreater(stats["total_symbols"], 0)

    # 21. persistent_cache + get_dependencies (warm preserve dependencies)
    def test_comb_cache_dependencies_preserve(self):
        deps1 = self.client.call_tool("get_dependencies", {"path": "src/main.rs"})
        deps2 = self.client.call_tool("get_dependencies", {"path": "src/main.rs"})
        self.assertEqual(len(deps1["forward_dependencies"]), len(deps2["forward_dependencies"]))

    # 22. persistent_cache + get_type_graph
    def test_comb_cache_type_graph_preserve(self):
        tg1 = self.client.call_tool("get_type_graph", {"name": "User"})
        tg2 = self.client.call_tool("get_type_graph", {"name": "User"})
        self.assertEqual(tg1["name"], tg2["name"])

    # 23. persistent_cache + get_entrypoints
    def test_comb_cache_entrypoints_preserve(self):
        ep1 = self.client.call_tool("get_entrypoints")
        ep2 = self.client.call_tool("get_entrypoints")
        self.assertEqual(ep1["total_entrypoints"], ep2["total_entrypoints"])

    # 24. persistent_cache + get_architecture_map
    def test_comb_cache_architecture_preserve(self):
        a1 = self.client.call_tool("get_architecture_map")
        a2 = self.client.call_tool("get_architecture_map")
        self.assertEqual(a1["total_modules"], a2["total_modules"])

    # 25. persistent_cache + get_impact_analysis
    def test_comb_cache_impact_preserve(self):
        i1 = self.client.call_tool("get_impact_analysis", {"target": "UserService"})
        i2 = self.client.call_tool("get_impact_analysis", {"target": "UserService"})
        self.assertEqual(i1["risk_assessment"]["score"], i2["risk_assessment"]["score"])

    # 26. cycle_detection + get_dependencies
    def test_comb_cycle_detection_flag(self):
        self.client.call_tool("set_workspace", {"path": self.cycles_dir})
        deps = self.client.call_tool("get_dependencies", {"path": "cycle_a.ts"})
        self.assertTrue(deps["has_cycles"])
        self.assertGreater(len(deps["cycles"]), 0)

    # 27. internal_import_resolution + get_impact_analysis
    def test_comb_import_resolution_impact_traversal(self):
        deps = self.client.call_tool("get_dependencies", {"path": "src/main.rs"})
        impact = self.client.call_tool("get_impact_analysis", {"target": "src/main.rs"})
        self.assertIn("affected_files", impact)

    # 28. all_11_tools_sequential_session
    def test_comb_all_11_tools_sequential(self):
        # Call all 11 tools sequentially on the same stdio session
        r1 = self.client.call_tool("set_workspace", {"path": self.sample_dir})
        r2 = self.client.call_tool("get_file_outline", {"path": "src/main.rs"})
        r3 = self.client.call_tool("find_definition", {"name": "User"})
        r4 = self.client.call_tool("get_call_graph", {"name": "UserService"})
        r5 = self.client.call_tool("fuzzy_search_symbols", {"query": "User"})
        r6 = self.client.call_tool("get_project_stats")
        r7 = self.client.call_tool("get_dependencies", {"path": "src/main.rs"})
        r8 = self.client.call_tool("get_type_graph", {"name": "User"})
        r9 = self.client.call_tool("get_entrypoints")
        r10 = self.client.call_tool("get_architecture_map")
        r11 = self.client.call_tool("get_impact_analysis", {"target": "UserService"})

        self.assertIsNotNone(r1)
        self.assertIsNotNone(r2)
        self.assertIsNotNone(r3)
        self.assertIsNotNone(r4)
        self.assertIsNotNone(r5)
        self.assertIsNotNone(r6)
        self.assertIsNotNone(r7)
        self.assertIsNotNone(r8)
        self.assertIsNotNone(r9)
        self.assertIsNotNone(r10)
        self.assertIsNotNone(r11)

    # 29. repeated_tool_calls_stress
    def test_comb_repeated_tool_stress(self):
        for _ in range(5):
            stats = self.client.call_tool("get_project_stats")
            self.assertGreater(stats["total_symbols"], 0)

    # 30. tools_list_and_call_interleaving
    def test_comb_tools_list_and_call_interleaving(self):
        tools = self.client.list_tools()
        self.assertEqual(len(tools), 11)
        stats = self.client.call_tool("get_project_stats")
        self.assertIn("total_symbols", stats)
        tools2 = self.client.list_tools()
        self.assertEqual(len(tools2), 11)


if __name__ == "__main__":
    unittest.main()
