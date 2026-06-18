#!/usr/bin/env bash
# Pre-commit verification for the reovim workspace.
#
# Usage: scripts/check.sh [MODE]
#
# Modes:
#   (none)         full check: layout/docs/fmt, then clippy and tests in parallel
#   --quick        layout/docs/fmt + doctests + clippy (fast dev iteration)
#   --sequential   layout/docs/fmt, clippy, tests one after another (debugging /
#                  low-memory environments)
#   --clean-cache  remove the dedicated clippy target dir and exit
#
# Clippy runs against its own target dir (target/check-clippy) so the
# parallel test build and the clippy build do not invalidate each
# other's incremental caches.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLIPPY_TARGET="$ROOT/target/check-clippy"
# The freestanding floor (reovim-arch on the bare-metal targets) is cfg-gated
# out of the host build, so the host clippy above never sees it. Each
# bare-metal target gets its own clippy pass against its own target dir.
CLIPPY_NONE_TARGET="$ROOT/target/check-clippy-none"
CLIPPY_NONE_X86_TARGET="$ROOT/target/check-clippy-none-x86"

usage() {
    sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

mode=full
case "${1:-}" in
    "") mode=full ;;
    --quick) mode=quick ;;
    --sequential) mode=sequential ;;
    --clean-cache)
        rm -rf "$CLIPPY_TARGET" "$CLIPPY_NONE_TARGET" "$CLIPPY_NONE_X86_TARGET"
        echo "removed $CLIPPY_TARGET $CLIPPY_NONE_TARGET $CLIPPY_NONE_X86_TARGET"
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
# Freestanding-floor lints. Lib only (`reovim-arch` integration tests host a
# std harness that cannot cross-compile to the bare-metal target); the
# `runtime` feature links the `_start`/panic-handler surface the floor ships.
run_clippy_none() {
    CARGO_TARGET_DIR="$CLIPPY_NONE_TARGET" \
        cargo clippy -p reovim-arch --target aarch64-unknown-none \
        --features runtime -- -D warnings
}
run_clippy_none_x86() {
    CARGO_TARGET_DIR="$CLIPPY_NONE_X86_TARGET" \
        cargo clippy -p reovim-arch --target x86_64-unknown-none \
        --features runtime -- -D warnings
}
run_tests() { cargo test --workspace; }
run_doc_tests() { cargo test --workspace --doc; }
run_test_layout() { "$ROOT/scripts/check-test-layout.sh" --check; }
run_public_doctests() { "$ROOT/scripts/check-public-doctests.sh" --check; }

cd "$ROOT"

echo "==> test-layout (L12)"
if ! run_test_layout; then
    echo "check.sh: test-layout FAILED (inline #[cfg(test)] mod tests blocks found)" >&2
    exit 1
fi

echo "==> public-doctests (L12)"
if ! run_public_doctests; then
    echo "check.sh: public-doctests FAILED (public API items without doctests)" >&2
    exit 1
fi

echo "==> fmt"
if ! run_fmt; then
    echo "check.sh: fmt FAILED" >&2
    exit 1
fi

echo "==> clippy (freestanding: aarch64-unknown-none)"
if ! run_clippy_none; then
    echo "check.sh: clippy-none FAILED (freestanding arch lints)" >&2
    exit 1
fi

echo "==> clippy (freestanding: x86_64-unknown-none)"
if ! run_clippy_none_x86; then
    echo "check.sh: clippy-none-x86 FAILED (freestanding arch lints)" >&2
    exit 1
fi

case "$mode" in
    quick)
        echo "==> doc-tests"
        if ! run_doc_tests; then
            echo "check.sh: doc-tests FAILED" >&2
            exit 1
        fi
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
