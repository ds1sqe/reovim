#!/usr/bin/env bash
set -euo pipefail

# Usage: ./scripts/coverage.sh [line|branch|mcdc|server] [--open] [--lcov]
#
# Modes:
#   line    — Line/region coverage (nightly, includes full workspace)
#   branch  — Branch coverage (nightly, default, excludes reovim-server)
#   mcdc    — MC/DC coverage (nightly, slowest, excludes reovim-server)
#   server  — Server-only branch coverage via --text (workaround for LLVM bug)
#
# Options:
#   --open  — Generate HTML report and open in browser
#   --lcov  — Generate lcov.info (for CI / codecov upload)
#
# Default: branch coverage, HTML report to target/llvm-cov/html/
#
# LLVM bug (https://github.com/llvm/llvm-project/issues/119558):
#   llvm-cov crashes (SIGSEGV in getInstantiationGroups) when processing branch
#   coverage data from #[tonic::async_trait] service implementations. Root cause:
#   branch instrumentation (-Z coverage-options=branch) generates malformed
#   coverage mapping records for async trait macro expansions. Affects 8 files
#   in server/lib/server/src/grpc/ that implement gRPC service traits.
#
#   Line coverage is NOT affected — reovim-server is included in line mode.
#   Branch/MC/DC modes exclude it. Use "server" mode for text-only branch output.

MODE="${1:-branch}"
case "$MODE" in
  line|branch|mcdc|server) shift ;;
  --*) MODE="branch" ;;  # no mode given, first arg is an option
  *) echo "Unknown mode: $MODE (expected: line, branch, mcdc, server)" >&2; exit 1 ;;
esac

OPEN=false
LCOV=false
for arg in "$@"; do
  case "$arg" in
    --open) OPEN=true ;;
    --lcov) LCOV=true ;;
    *) echo "Unknown option: $arg" >&2; exit 1 ;;
  esac
done

# Prerequisites
if ! command -v cargo-llvm-cov &>/dev/null; then
  echo "cargo-llvm-cov not found. Install with:" >&2
  echo "  cargo install cargo-llvm-cov" >&2
  exit 1
fi

if ! rustup component list --toolchain nightly 2>/dev/null | grep -q 'llvm-tools.*installed'; then
  echo "Installing llvm-tools-preview for nightly..."
  rustup component add llvm-tools-preview --toolchain nightly
fi

# Server mode: plain-text branch coverage (HTML/LCOV crash with LLVM bug)
if [ "$MODE" = "server" ]; then
  echo -e "\033[1;33m==> Running server branch coverage (text output)...\033[0m"
  OUTPUT="target/llvm-cov/server-coverage.txt"
  cargo +nightly llvm-cov --branch -p reovim-server --text > "$OUTPUT" 2>&1
  echo -e "\033[0;32m✓ Server coverage: $OUTPUT\033[0m"
  echo -e "\033[0;33m  (HTML/LCOV unavailable — LLVM #119558 SIGSEGV on branch data)\033[0m"
  echo -e "\033[0;33m  Use 'line' mode for full workspace coverage including server.\033[0m"
  exit 0
fi

# Crate exclusions (always):
#   reovim-module-macros     — proc-macro, runs at compile time
#   reovim-driver-ffi-python — pyo3 cdylib, linker issues under coverage
#   reovim-bench / perf-report — tools, not product code
#   runner / layout / statusline / which-key — legacy (same as check.sh)
EXCLUDE=(
  --exclude reovim-module-macros
  --exclude reovim-driver-ffi-python
  --exclude reovim-bench
  --exclude perf-report
  --exclude runner
  --exclude reovim-module-layout
  --exclude reovim-module-statusline
  --exclude reovim-module-which-key
)

# reovim-server: excluded from branch/mcdc (LLVM #119558), included in line mode
if [ "$MODE" != "line" ]; then
  EXCLUDE+=(--exclude reovim-server)
fi

# Mode flag and environment
# Note: cargo-llvm-cov --mcdc passes -Z coverage-options=mcdc, but nightly 1.95+
# renamed it to "condition". We pass it via RUSTFLAGS instead.
EXTRA_RUSTFLAGS="--cfg coverage_nightly"
case "$MODE" in
  line)   MODE_FLAG=() ;;
  branch) MODE_FLAG=(--branch) ;;
  mcdc)   MODE_FLAG=(); EXTRA_RUSTFLAGS="$EXTRA_RUSTFLAGS -Z coverage-options=condition" ;;
esac

echo -e "\033[1;33m==> Running $MODE coverage...\033[0m"

if $LCOV; then
  OUTPUT_PATH="target/llvm-cov/lcov.${MODE}.info"
  RUSTFLAGS="$EXTRA_RUSTFLAGS" cargo +nightly llvm-cov "${MODE_FLAG[@]}" \
    --workspace "${EXCLUDE[@]}" \
    --lcov --output-path "$OUTPUT_PATH"
  echo -e "\033[0;32m✓ LCOV report: $OUTPUT_PATH\033[0m"
else
  OUTPUT_DIR="target/llvm-cov/html"
  OPEN_FLAG=()
  if $OPEN; then
    OPEN_FLAG=(--open)
  fi
  RUSTFLAGS="$EXTRA_RUSTFLAGS" cargo +nightly llvm-cov "${MODE_FLAG[@]}" \
    --workspace "${EXCLUDE[@]}" \
    --html --output-dir "$OUTPUT_DIR" "${OPEN_FLAG[@]}"
  echo -e "\033[0;32m✓ HTML report: $OUTPUT_DIR/index.html\033[0m"
fi
