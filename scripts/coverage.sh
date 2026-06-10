#!/usr/bin/env bash
# Coverage measurement for the reovim workspace (10.1 DEV1).
#
# Usage: scripts/coverage.sh [MODE] [OPTIONS]
#
# Modes:
#   mcdc    condition (MC/DC) coverage — nightly, default
#   line    line coverage
#
# Options:
#   --lcov               also write target/llvm-cov/lcov.<mode>.info
#   --fail-under <pct>   fail when line coverage is below <pct>
#                        (default 100)
#   --no-fail            report only, never fail
#
# Note: nightly 1.95+ renamed `-Z coverage-options=mcdc` to
# `condition`; cargo-llvm-cov's --mcdc flag still passes the old
# name, so mcdc mode injects the option via RUSTFLAGS instead.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="${1:-mcdc}"
case "$MODE" in
    mcdc | line) shift ;;
    --*) MODE="mcdc" ;;
    *)
        echo "error: unknown mode: $MODE (expected: mcdc, line)" >&2
        exit 2
        ;;
esac

LCOV=false
FAIL_UNDER="100"
while [ $# -gt 0 ]; do
    case "$1" in
        --lcov) LCOV=true; shift ;;
        --fail-under)
            FAIL_UNDER="${2:?--fail-under requires a percentage}"
            shift 2
            ;;
        --no-fail) FAIL_UNDER=""; shift ;;
        *)
            echo "error: unknown option: $1" >&2
            exit 2
            ;;
    esac
done

if ! command -v cargo-llvm-cov > /dev/null 2>&1; then
    echo "cargo-llvm-cov not found; install with: cargo install cargo-llvm-cov" >&2
    exit 1
fi

# EXCLUDE array: intentionally empty for the sovereign workspace.
#
# The baseline (d668591a) listed prost-generated and proc-macro crates
# here. None of those crates exist in v0.16. Any future exclusion MUST
# name the crate and explain why it is genuinely untestable (e.g. a
# proc-macro crate whose codegen paths cannot be driven by unit tests).
# Until such a crate lands, this list stays empty.
EXCLUDE=()

ARGS=(--workspace)
RUSTFLAGS_EXTRA=""
if [ "$MODE" = "mcdc" ]; then
    RUSTFLAGS_EXTRA="-Z coverage-options=condition"
fi
if [ -n "$FAIL_UNDER" ]; then
    ARGS+=(--fail-under-lines "$FAIL_UNDER")
fi
for crate in "${EXCLUDE[@]}"; do
    ARGS+=(--exclude "$crate")
done

echo "==> $MODE coverage"
RUSTFLAGS="${RUSTFLAGS:-} $RUSTFLAGS_EXTRA" cargo llvm-cov "${ARGS[@]}" || exit 1

if [ "$LCOV" = true ]; then
    mkdir -p target/llvm-cov
    LCOV_PATH="target/llvm-cov/lcov.${MODE}.info"
    cargo llvm-cov report --lcov --output-path "$LCOV_PATH" || exit 1
    echo "lcov report: $LCOV_PATH"
fi
