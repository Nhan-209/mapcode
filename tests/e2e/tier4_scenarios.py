"""
tests/e2e/tier4_scenarios.py — Tier 4 Real-World Application Scenarios Test Suite.

Simulates realistic, multi-step developer and AI assistant workflows:
1. Rust Axum Microservice Architecture Audit
2. Blast Radius & Impact Audit Before Refactoring Core Model
3. TypeScript Express Microservice API & Event Gateway Inspection
4. Python FastAPI & Celery Background Processing Map
5. Go Microservice with Struct Embedding & Gin Routes
6. C/C++ Engine Polymorphism & Native Includes Traversal
7. Circular Dependency Diagnostic & Refactoring Alert
8. Cold to Warm Startup Indexing & Performance Validation
9. Multi-Language Cross-Cutting Symbol Search & Definition Resolution
10. Architecture Map Martin Metric Instability Profiling
"""

import os
import time
import unittest
from typing import Optional
from tests.e2e.client import McpClient


class TestTier4Scenarios(unittest.TestCase):
    client: Optional[McpClient] = None
    server_cmd = ["python", "tests/e2e/mock_server.py"]
    sample_dir = "tests/fixtures/sample_workspace"
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

    # Scenario 1: Rust Axum Microservice Architecture Audit
    def test_scenario1_axum_microservice_audit(self):
        # Step 1: Discover HTTP routes
        eps = self.client.call_tool("get_entrypoints", {"category": "http"})
        self.assertGreater(eps["total_entrypoints"], 0)
        axum_eps = [e for e in eps["entrypoints"] if e.get("framework") == "axum"]
        self.assertGreaterEqual(len(axum_eps), 1)

        # Step 2: Inspect routes file outline
        routes_file = axum_eps[0]["file_path"]
        outline = self.client.call_tool("get_file_outline", {"path": routes_file})
        self.assertGreater(outline["total_symbols"], 0)

        # Step 3: Inspect dependencies of routes file
        deps = self.client.call_tool("get_dependencies", {"path": routes_file})
        self.assertGreater(len(deps["forward_dependencies"]), 0)

        # Step 4: Architecture tree layer role
        arch = self.client.call_tool("get_architecture_map")
        src = next((s for s in arch["root"]["submodules"] if s["name"] == "src"), None)
        self.assertIsNotNone(src)

    # Scenario 2: Blast Radius & Impact Audit Before Refactoring Core Model
    def test_scenario2_refactoring_blast_radius_audit(self):
        # Step 1: Query type graph for User model
        tg = self.client.call_tool("get_type_graph", {"name": "User"})
        self.assertEqual(tg["name"], "User")
        supertypes = [s["name"] for s in tg["supertypes"]]
        self.assertTrue(any(t in supertypes for t in ("Display", "Default")))

        # Step 2: Full blast radius traversal
        impact = self.client.call_tool("get_impact_analysis", {"target": "src/models/user.rs", "max_depth": 5})
        b = impact["blast_radius"]
        self.assertGreaterEqual(b["total_affected_files"], 1)

        # Step 3: Risk assessment score validation
        risk = impact["risk_assessment"]
        self.assertIn(risk["rating"], ("low", "medium", "high", "critical"))
        self.assertTrue(0 <= risk["score"] <= 100)

        # Step 4: Affected files contain upstream consumers
        af = impact["affected_files"]
        self.assertTrue(any("user.rs" in f for f in af))

    # Scenario 3: TypeScript Express Microservice API & Event Gateway Inspection
    def test_scenario3_express_event_gateway(self):
        # Step 1: List Express endpoints
        eps = self.client.call_tool("get_entrypoints", {"category": "http"})
        express_eps = [e for e in eps["entrypoints"] if e.get("framework") == "express"]
        self.assertGreaterEqual(len(express_eps), 1)

        # Step 2: List background events
        workers = self.client.call_tool("get_entrypoints", {"category": "worker"})
        node_workers = [e for e in workers["entrypoints"] if e.get("framework") == "node_events"]
        self.assertGreaterEqual(len(node_workers), 1)

        # Step 3: Dependencies of ts_app/server.ts
        deps = self.client.call_tool("get_dependencies", {"path": "ts_app/server.ts"})
        self.assertTrue(any(imp["is_external"] for imp in deps["forward_dependencies"]))

    # Scenario 4: Python FastAPI & Celery Background Processing Map
    def test_scenario4_python_fastapi_celery(self):
        # Step 1: FastAPI endpoints
        eps = self.client.call_tool("get_entrypoints", {"category": "http"})
        py_eps = [e for e in eps["entrypoints"] if e.get("framework") == "fastapi"]
        self.assertGreaterEqual(len(py_eps), 1)

        # Step 2: Celery tasks
        workers = self.client.call_tool("get_entrypoints", {"category": "worker"})
        celery_eps = [e for e in workers["entrypoints"] if e.get("framework") == "celery"]
        self.assertGreaterEqual(len(celery_eps), 1)

        # Step 3: Click CLI
        cli_eps = self.client.call_tool("get_entrypoints", {"category": "cli"})
        click_eps = [e for e in cli_eps["entrypoints"] if e.get("framework") == "click"]
        self.assertGreaterEqual(len(click_eps), 1)

        # Step 4: Python class inheritance
        tg = self.client.call_tool("get_type_graph", {"name": "Product"})
        self.assertIn("BaseEntity", [s["name"] for s in tg["supertypes"]])

    # Scenario 5: Go Microservice with Struct Embedding & Gin Routes
    def test_scenario5_go_embedding_gin(self):
        # Step 1: Gin routes
        eps = self.client.call_tool("get_entrypoints", {"category": "http"})
        gin_eps = [e for e in eps["entrypoints"] if e.get("framework") == "gin"]
        self.assertGreaterEqual(len(gin_eps), 1)

        # Step 2: Struct embedding
        tg = self.client.call_tool("get_type_graph", {"name": "Customer"})
        self.assertIn("BaseRecord", tg["embedded_types"])

        # Step 3: Go main
        startup = self.client.call_tool("get_entrypoints", {"category": "startup"})
        go_mains = [e for e in startup["entrypoints"] if e.get("framework") == "go"]
        self.assertGreaterEqual(len(go_mains), 1)

    # Scenario 6: C/C++ Engine Polymorphism & Native Includes Traversal
    def test_scenario6_cpp_engine_polymorphism(self):
        # Step 1: Class inheritance
        tg = self.client.call_tool("get_type_graph", {"name": "AdvancedEngine"})
        self.assertIn("BaseEngine", [s["name"] for s in tg["supertypes"]])

        # Step 2: Include dependencies
        deps = self.client.call_tool("get_dependencies", {"path": "cpp_app/main.cpp"})
        self.assertTrue(any("engine.h" in imp["specifier"] for imp in deps["forward_dependencies"]))

        # Step 3: Startup main
        startup = self.client.call_tool("get_entrypoints", {"category": "startup"})
        cpp_mains = [e for e in startup["entrypoints"] if e.get("framework") == "c/cpp"]
        self.assertGreaterEqual(len(cpp_mains), 1)

    # Scenario 7: Circular Dependency Diagnostic & Refactoring Alert
    def test_scenario7_circular_dependency_diagnostic(self):
        # Step 1: Switch to circular imports workspace
        self.client.call_tool("set_workspace", {"path": self.cycles_dir})

        # Step 2: Detect 3-node cycle in TS
        deps_ts = self.client.call_tool("get_dependencies", {"path": "cycle_a.ts"})
        self.assertTrue(deps_ts["has_cycles"])
        cycle_ts = deps_ts["cycles"][0]
        self.assertEqual(cycle_ts[0], cycle_ts[-1])

        # Step 3: Detect 2-node cycle in Python
        deps_py = self.client.call_tool("get_dependencies", {"path": "py_cycle_1.py"})
        self.assertTrue(deps_py["has_cycles"])
        cycle_py = deps_py["cycles"][0]
        self.assertEqual(cycle_py[0], cycle_py[-1])

    # Scenario 8: Cold to Warm Startup Indexing & Performance Validation
    def test_scenario8_cold_warm_startup_benchmark(self):
        # Step 1: Query stats
        t0 = time.perf_counter()
        stats1 = self.client.call_tool("get_project_stats")
        cold_time = (time.perf_counter() - t0) * 1000

        # Step 2: Second query (warm)
        t1 = time.perf_counter()
        stats2 = self.client.call_tool("get_project_stats")
        warm_time = (time.perf_counter() - t1) * 1000

        self.assertEqual(stats1["total_symbols"], stats2["total_symbols"])
        self.assertLess(warm_time, 50.0)  # sub-50ms query overhead

    # Scenario 9: Multi-Language Cross-Cutting Symbol Search & Definition Resolution
    def test_scenario9_cross_cutting_symbol_resolution(self):
        # Step 1: Fuzzy search for "main" across all languages
        matches = self.client.call_tool("fuzzy_search_symbols", {"query": "main"})
        self.assertGreaterEqual(len(matches), 1)

        # Step 2: Find definition of UserService
        defs = self.client.call_tool("find_definition", {"name": "UserService"})
        self.assertGreaterEqual(len(defs), 1)
        user_def = defs[0]
        self.assertEqual(user_def["name"], "UserService")

        # Step 3: Get outline of defining file
        outline = self.client.call_tool("get_file_outline", {"path": user_def["file_path"]})
        self.assertGreater(outline["total_symbols"], 0)

    # Scenario 10: Architecture Map Martin Metric Instability Profiling
    def test_scenario10_martin_instability_profiling(self):
        # Step 1: Query architecture tree
        arch = self.client.call_tool("get_architecture_map", {"max_depth": 3})
        root = arch["root"]

        # Step 2: Validate Martin metrics for all modules
        def validate_node(node):
            m = node["metrics"]
            self.assertGreaterEqual(m["afferent_coupling_ca"], 0)
            self.assertGreaterEqual(m["efferent_coupling_ce"], 0)
            self.assertTrue(0.0 <= m["instability"] <= 1.0)
            self.assertGreaterEqual(m["symbol_density"], 0.0)
            for sub in node["submodules"]:
                validate_node(sub)

        validate_node(root)
        self.assertGreater(arch["total_modules"], 1)


if __name__ == "__main__":
    unittest.main()
