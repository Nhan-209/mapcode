"""
tests/e2e/runner.py — Master Test Runner for MapCode MCP Server E2E Test Suite.

Usage:
  python tests/e2e/runner.py                   # Run all tiers (Tiers 1-4)
  python tests/e2e/runner.py --tier 1          # Run Tier 1 Feature Coverage
  python tests/e2e/runner.py --tier 2          # Run Tier 2 Boundary Cases
  python tests/e2e/runner.py --tier 3          # Run Tier 3 Combinations
  python tests/e2e/runner.py --tier 4          # Run Tier 4 Real-World Scenarios
  python tests/e2e/runner.py --feature get_dependencies  # Filter by feature
  python tests/e2e/runner.py --binary bin/mapcode.exe    # Run against binary
  python tests/e2e/runner.py --json            # Output structured JSON summary
"""

import argparse
import io
import json
import os
import sys
import time
import unittest
from typing import Any, Dict, List, Optional

# Ensure repository root is on sys.path
REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
if REPO_ROOT not in sys.path:
    sys.path.insert(0, REPO_ROOT)

# Import test modules
import tests.e2e.tier1_features as t1
import tests.e2e.tier2_boundary as t2
import tests.e2e.tier3_combinations as t3
import tests.e2e.tier4_scenarios as t4


def build_suite(tier: str, feature_filter: Optional[str] = None) -> unittest.TestSuite:
    loader = unittest.TestLoader()
    suite = unittest.TestSuite()

    tier_map = {
        "1": [
            t1.TestTier1Dependencies,
            t1.TestTier1TypeGraph,
            t1.TestTier1Entrypoints,
            t1.TestTier1Architecture,
            t1.TestTier1ImpactAnalysis,
            t1.TestTier1CycleDetection,
            t1.TestTier1Persistence,
            t1.TestTier1ExistingTools,
            t1.TestTier1CloudAndProtocol,
        ],
        "2": [
            t2.TestTier2ImportsAndDeps,
            t2.TestTier2TypeGraph,
            t2.TestTier2Entrypoints,
            t2.TestTier2Architecture,
            t2.TestTier2ImpactAnalysis,
            t2.TestTier2PersistenceAndProtocol,
        ],
        "3": [
            t3.TestTier3Combinations,
        ],
        "4": [
            t4.TestTier4Scenarios,
        ]
    }

    selected_classes = []
    if tier == "all":
        for classes in tier_map.values():
            selected_classes.extend(classes)
    elif tier in tier_map:
        selected_classes.extend(tier_map[tier])
    else:
        raise ValueError(f"Unknown tier: {tier}")

    for test_class in selected_classes:
        test_names = loader.getTestCaseNames(test_class)
        class_doc = (test_class.__doc__ or "").lower()
        for name in test_names:
            if feature_filter:
                f_low = feature_filter.lower()
                method = getattr(test_class, name, None)
                method_doc = (getattr(method, "__doc__", "") or "").lower()
                matched = (
                    f_low in name.lower()
                    or f_low in test_class.__name__.lower()
                    or f_low in class_doc
                    or f_low in method_doc
                )
                # Also support alias mapping for tools like get_dependencies -> f10/dependencies
                if not matched:
                    alias_map = {
                        "get_dependencies": ["dependencies", "f01", "f07", "f08", "f10"],
                        "get_type_graph": ["type", "f02", "f11", "f12", "f13", "f14"],
                        "get_entrypoints": ["entry", "f03", "f15", "f16", "f17", "f18", "f19"],
                        "get_architecture_map": ["architecture", "f20", "f21", "f22", "f23"],
                        "get_impact_analysis": ["impact", "f24", "f25", "f26", "f27", "f28"],
                        "cycle_detection": ["cycle", "f09"],
                        "cache": ["f04", "f05", "f06", "persistence"],
                    }
                    for tool_key, aliases in alias_map.items():
                        if f_low in tool_key or any(a in f_low for a in aliases):
                            if any(a in name.lower() or a in test_class.__name__.lower() for a in aliases):
                                matched = True
                                break
                if not matched:
                    continue
            suite.addTest(test_class(name))

    return suite


def run_tests(args: argparse.Namespace) -> int:
    if args.binary:
        os.environ["MAPCODE_BIN"] = os.path.abspath(args.binary)
    elif args.mock:
        if "MAPCODE_BIN" in os.environ:
            del os.environ["MAPCODE_BIN"]

    suite = build_suite(args.tier, args.feature)
    total_cases = suite.countTestCases()

    if total_cases == 0:
        print(f"No tests found matching tier='{args.tier}', feature='{args.feature}'")
        return 1

    start_time = time.perf_counter()
    stream = io.StringIO() if args.json else sys.stderr
    runner = unittest.TextTestRunner(
        stream=stream,
        verbosity=2 if args.verbose else 1,
        failfast=args.failfast
    )
    result = runner.run(suite)
    duration_ms = (time.perf_counter() - start_time) * 1000

    passed = total_cases - len(result.failures) - len(result.errors) - len(result.skipped)
    success = result.wasSuccessful()

    if args.json:
        report = {
            "summary": {
                "total": total_cases,
                "passed": passed,
                "failed": len(result.failures),
                "errors": len(result.errors),
                "skipped": len(result.skipped),
                "duration_ms": round(duration_ms, 2),
                "success": success,
                "tier": args.tier,
                "feature_filter": args.feature,
                "binary": os.environ.get("MAPCODE_BIN", "mock_server")
            },
            "failures": [
                {"test": str(test), "traceback": tb} for test, tb in result.failures
            ],
            "errors": [
                {"test": str(test), "traceback": tb} for test, tb in result.errors
            ]
        }
        print(json.dumps(report, indent=2))
    else:
        print("\n" + "=" * 60)
        print(f"MapCode E2E Test Suite Run Summary — Tier [{args.tier.upper()}]")
        print("=" * 60)
        print(f"Total Test Cases: {total_cases}")
        print(f"Passed:           {passed}")
        print(f"Failed:           {len(result.failures)}")
        print(f"Errors:           {len(result.errors)}")
        print(f"Skipped:          {len(result.skipped)}")
        print(f"Execution Time:   {duration_ms:.2f} ms")
        print(f"Target Mode:      {os.environ.get('MAPCODE_BIN', 'Specification Oracle (mock_server)')}")
        print(f"Overall Result:   {'PASS (100%)' if success else 'FAIL'}")
        print("=" * 60)

    return 0 if success else 1


def main() -> None:
    parser = argparse.ArgumentParser(description="MapCode MCP Server E2E Test Runner")
    parser.add_argument("--tier", default="all", choices=["1", "2", "3", "4", "all"], help="Test tier to run")
    parser.add_argument("--feature", default=None, help="Filter test methods by feature substring")
    parser.add_argument("--binary", default=None, help="Path to compiled MapCode binary executable")
    parser.add_argument("--mock", action="store_true", help="Force specification oracle mock server")
    parser.add_argument("--json", action="store_true", help="Output results as JSON")
    parser.add_argument("--failfast", action="store_true", help="Stop on first error or failure")
    parser.add_argument("-v", "--verbose", action="store_true", help="Verbose test reporting")

    args = parser.parse_args()
    exit_code = run_tests(args)
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
