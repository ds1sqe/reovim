"""Select standard unittest scenarios; no scenario registry is needed."""

import argparse
from pathlib import Path
import tempfile
import unittest

from .fixture import ROOT, Scenario


def tests(suite):
    for test in suite:
        if isinstance(test, unittest.TestSuite):
            yield from tests(test)
        else:
            yield test


def scenario_name(test):
    return test.id().split(".")[1].removeprefix("test_")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binaries", nargs="?", type=Path,
                        default=ROOT / "product/target/debug")
    parser.add_argument("--scenario", action="append", default=[],
                        help="run one scenario file by name; repeat to combine")
    parser.add_argument("--list", action="store_true",
                        help="list scenarios without starting a server")
    parser.add_argument("--artifacts", type=Path,
                        help="parent for a new run's retained logs")
    args = parser.parse_args()
    loader = unittest.TestLoader()
    suite = loader.discover(
        str(Path(__file__).parent), top_level_dir=str(ROOT / "scripts"))
    if loader.errors:
        parser.error("scenario discovery failed:\n" + "\n".join(loader.errors))
    discovered = list(tests(suite))
    if not discovered:
        parser.error("no E2E scenarios were discovered")
    available = sorted({scenario_name(test) for test in discovered})
    if args.list:
        print("\n".join(available))
        return 0
    unknown = set(args.scenario) - set(available)
    if unknown:
        parser.error(f"unknown scenarios: {', '.join(sorted(unknown))}")
    for name in ("reovim-server", "reovim-cli"):
        if not (args.binaries / name).is_file():
            parser.error(f"missing binary: {args.binaries / name}")
    Scenario.binaries = args.binaries.resolve()
    if args.artifacts:
        args.artifacts.mkdir(parents=True, exist_ok=True)
    Scenario.artifacts = Path(tempfile.mkdtemp(
        prefix="reovim-e2e-", dir=args.artifacts))
    selected = [test for test in discovered
                if not args.scenario or scenario_name(test) in args.scenario]
    print(f"E2E evidence: {Scenario.artifacts}", flush=True)
    result = unittest.TextTestRunner(verbosity=2).run(
        unittest.TestSuite(selected))
    print(f"E2E evidence: {Scenario.artifacts}", flush=True)
    return 0 if result.wasSuccessful() else 1
