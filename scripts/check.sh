#!/usr/bin/env bash
# Pre-commit verification for the reovim workspace.
#
# Usage: scripts/check.sh [MODE]
#
# Modes:
#   (none)         full check: fmt, then clippy and tests in parallel
#   --quick        fmt + clippy only (fast dev iteration)
#   --sequential   fmt, clippy, tests one after another (debugging /
#                  low-memory environments)
#   --clean-cache  remove the dedicated clippy target dir and exit
#
# Clippy runs against its own target dir (target/check-clippy) so the
# parallel test build and the clippy build do not invalidate each
# other's incremental caches.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLIPPY_TARGET="$ROOT/target/check-clippy"

usage() {
    sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

mode=full
case "${1:-}" in
    "") mode=full ;;
    --quick) mode=quick ;;
    --sequential) mode=sequential ;;
    --clean-cache)
        rm -rf "$CLIPPY_TARGET"
        echo "removed $CLIPPY_TARGET"
        exit 0
        ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        echo "error: unknown argument: $1" >&2
        usage >&2
        exit 2
        ;;
esac

run_fmt() { cargo fmt --all --check; }
run_clippy() {
    CARGO_TARGET_DIR="$CLIPPY_TARGET" \
        cargo clippy --workspace --all-targets -- -D warnings
}
run_tests() { cargo test --workspace; }
run_test_layout() { "$ROOT/scripts/check-test-layout.sh" --check; }

cd "$ROOT"

echo "==> test-layout (L12)"
if ! run_test_layout; then
    echo "check.sh: test-layout FAILED (inline #[cfg(test)] mod tests blocks found)" >&2
    exit 1
fi

echo "==> fmt"
if ! run_fmt; then
    echo "check.sh: fmt FAILED" >&2
    exit 1
fi

case "$mode" in
    quick)
        echo "==> clippy"
        if ! run_clippy; then
            echo "check.sh: clippy FAILED" >&2
            exit 1
        fi
        ;;
    sequential)
        echo "==> clippy"
        if ! run_clippy; then
            echo "check.sh: clippy FAILED" >&2
            exit 1
        fi
        echo "==> tests"
        if ! run_tests; then
            echo "check.sh: tests FAILED" >&2
            exit 1
        fi
        ;;
    full)
        echo "==> clippy + tests (parallel)"
        run_clippy &
        clippy_pid=$!
        run_tests &
        tests_pid=$!
        clippy_rc=0
        tests_rc=0
        wait "$clippy_pid" || clippy_rc=$?
        wait "$tests_pid" || tests_rc=$?
        if [ "$clippy_rc" -ne 0 ]; then
            echo "check.sh: clippy FAILED (exit $clippy_rc)" >&2
        fi
        if [ "$tests_rc" -ne 0 ]; then
            echo "check.sh: tests FAILED (exit $tests_rc)" >&2
        fi
        if [ "$clippy_rc" -ne 0 ] || [ "$tests_rc" -ne 0 ]; then
            exit 1
        fi
        ;;
esac

echo "check.sh: all checks passed"
