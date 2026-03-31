#!/usr/bin/env bash
set -euo pipefail

# Usage: ./scripts/coverage.sh [line|branch|mcdc|server] [--open] [--lcov] [--fail-under PCT]
#
# Modes:
#   line    — Line/region coverage (nightly, includes full workspace)
#   branch  — Branch coverage (nightly, excludes reovim-server)
#   mcdc    — MC/DC coverage (nightly, default, excludes reovim-server)
#   server  — Server-only branch coverage via --text (workaround for LLVM bug)
#
# Options:
#   --open            — Generate HTML report and open in browser
#   --lcov            — Generate lcov.info (for CI / codecov upload)
#   --fail-under PCT  — Fail if any source file's line coverage is below PCT (default: 100)
#   --no-fail         — Skip threshold enforcement
#
# Default: MC/DC coverage, 100% threshold, HTML report to target/llvm-cov/html/
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

MODE="${1:-mcdc}"
case "$MODE" in
  line|branch|mcdc|server) shift ;;
  --*) MODE="mcdc" ;;  # no mode given, first arg is an option
  *) echo "Unknown mode: $MODE (expected: line, branch, mcdc, server)" >&2; exit 1 ;;
esac

OPEN=false
LCOV=false
FAIL_UNDER="100"
while [ $# -gt 0 ]; do
  case "$1" in
    --open) OPEN=true; shift ;;
    --lcov) LCOV=true; shift ;;
    --fail-under) FAIL_UNDER="${2:?--fail-under requires a percentage}"; shift 2 ;;
    --no-fail) FAIL_UNDER=""; shift ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
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

# Server-only mode: plain-text branch coverage (HTML/LCOV crash with LLVM bug)
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
#   reovim-driver-display    — v1 cell-grid rendering (client-side in v2)
EXCLUDE=(
  --exclude reovim-module-macros
  --exclude reovim-driver-ffi-python
  --exclude reovim-bench
  --exclude perf-report
  --exclude reovim-driver-display
)

# reovim-server: excluded from branch/mcdc (LLVM #119558 SIGSEGV), included in line mode.
# For branch/mcdc we run server separately with line coverage and merge the LCOV.
INCLUDE_SERVER_LINE=false
if [ "$MODE" != "line" ]; then
  EXCLUDE+=(--exclude reovim-server)
  INCLUDE_SERVER_LINE=true
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

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LCOV_PATH="target/llvm-cov/lcov.${MODE}.info"

# ─── Step 1: Remove stale server binaries from coverage target ───────────────
# LLVM #119558: server binaries with branch/mcdc instrumentation cause SIGSEGV
# in llvm-cov export. Remove them before running to prevent the crash.
rm -f target/llvm-cov-target/debug/deps/reovim_server-* 2>/dev/null || true

# ─── Step 2: Run workspace coverage (LCOV always generated) ─────────────────

echo -e "\033[1;33m==> Running $MODE coverage...\033[0m"

RUSTFLAGS="$EXTRA_RUSTFLAGS" cargo +nightly llvm-cov "${MODE_FLAG[@]}" \
  --workspace "${EXCLUDE[@]}" \
  --lcov --output-path "$LCOV_PATH"

# Strip #[cfg(test)] regions from LCOV (test code is not a coverage target)
"$SCRIPT_DIR/lcov-filter-tests.sh" "$LCOV_PATH"

# ─── Step 3: Merge server line coverage (branch/mcdc exclude server) ────────
# LLVM #119558: branch/mcdc coverage SIGSEGV on server async trait impls.
# Line coverage works fine, so we run server with line coverage separately.

if $INCLUDE_SERVER_LINE; then
  echo -e "\033[1;33m==> Running server line coverage (LLVM #119558 workaround)...\033[0m"
  SERVER_LCOV="target/llvm-cov/lcov.server-line.info"
  # Run in isolated target dir to avoid llvm-cov export picking up mcdc-instrumented
  # workspace binaries which cause SIGSEGV.
  SERVER_TARGET="target/llvm-cov-server"
  CARGO_TARGET_DIR="$SERVER_TARGET" RUSTFLAGS="--cfg coverage_nightly" \
    cargo +nightly llvm-cov \
    -p reovim-server \
    --lcov --output-path "$SERVER_LCOV"
  "$SCRIPT_DIR/lcov-filter-tests.sh" "$SERVER_LCOV"
  # Append server LCOV to main LCOV
  cat "$SERVER_LCOV" >> "$LCOV_PATH"
  echo -e "\033[0;32m  Server line coverage merged into $LCOV_PATH\033[0m"
fi

echo -e "\033[0;32m✓ LCOV report: $LCOV_PATH\033[0m"

# ─── Step 3: Generate HTML report (optional) ────────────────────────────────

if ! $LCOV; then
  OUTPUT_DIR="target/llvm-cov/html"
  if command -v genhtml &>/dev/null; then
    genhtml "$LCOV_PATH" --output-directory "$OUTPUT_DIR" --quiet
    if $OPEN; then
      xdg-open "$OUTPUT_DIR/index.html" 2>/dev/null || open "$OUTPUT_DIR/index.html" 2>/dev/null || true
    fi
    echo -e "\033[0;32m✓ HTML report: $OUTPUT_DIR/index.html\033[0m"
  else
    echo -e "\033[0;33m⚠ genhtml not found, falling back to cargo llvm-cov HTML (unfiltered)\033[0m"
    OPEN_FLAG=()
    if $OPEN; then OPEN_FLAG=(--open); fi
    RUSTFLAGS="$EXTRA_RUSTFLAGS" cargo +nightly llvm-cov "${MODE_FLAG[@]}" \
      --workspace "${EXCLUDE[@]}" \
      --html --output-dir "$OUTPUT_DIR" "${OPEN_FLAG[@]}"
    echo -e "\033[0;32m✓ HTML report: $OUTPUT_DIR/index.html\033[0m"
  fi
fi

# ─── Threshold enforcement ───────────────────────────────────────────────────

if [ -n "$FAIL_UNDER" ]; then
  LCOV_FILE="target/llvm-cov/lcov.${MODE}.info"
  if [ ! -f "$LCOV_FILE" ]; then
    echo "Error: LCOV file not found at $LCOV_FILE (needed for --fail-under)" >&2
    exit 1
  fi

  echo -e "\033[1;33m==> Checking coverage threshold (${FAIL_UNDER}%)...\033[0m"

  # Parse LCOV: for each file compute line hit rate.
  # Print files below threshold sorted by coverage ascending.
  FAILED_FILES=$(awk -v threshold="$FAIL_UNDER" '
    /^SF:/ {
      file = substr($0, 4)
      # Make path relative
      rel = file
      sub(/.*\/server\//, "server/", rel)
      sub(/.*\/clients\//, "clients/", rel)
      sub(/.*\/shared\//, "shared/", rel)
      sub(/.*\/apps\//, "apps/", rel)
      sub(/.*\/tools\//, "tools/", rel)
      lf = 0; lh = 0
      next
    }
    /^LF:/ { lf = substr($0, 4) + 0; next }
    /^LH:/ { lh = substr($0, 4) + 0; next }
    /^end_of_record/ {
      if (lf > 0) {
        pct = (lh / lf) * 100.0
        if (pct < threshold + 0) {
          printf "  %6.2f%%  %4d/%4d  %s\n", pct, lh, lf, rel
          fail_count++
        }
        total_lf += lf
        total_lh += lh
      }
      next
    }
    END {
      if (total_lf > 0) {
        overall = (total_lh / total_lf) * 100.0
        printf "SUMMARY\t%d\t%d\t%.2f\t%d\n", total_lh, total_lf, overall, fail_count + 0
      }
    }
  ' "$LCOV_FILE")

  # Extract summary line
  SUMMARY=$(echo "$FAILED_FILES" | grep '^SUMMARY' | head -1)
  TOTAL_LH=$(echo "$SUMMARY" | cut -f2)
  TOTAL_LF=$(echo "$SUMMARY" | cut -f3)
  OVERALL=$(echo "$SUMMARY" | cut -f4)
  FAIL_COUNT=$(echo "$SUMMARY" | cut -f5)

  echo "  Overall: ${OVERALL}% (${TOTAL_LH}/${TOTAL_LF} lines)"

  if [ "${FAIL_COUNT:-0}" -gt 0 ]; then
    echo ""
    echo -e "\033[1;31m  Files below ${FAIL_UNDER}% threshold:\033[0m"
    echo "$FAILED_FILES" | grep -v '^SUMMARY' | sort
    echo ""
    echo -e "\033[1;31m✗ ${FAIL_COUNT} file(s) below ${FAIL_UNDER}% coverage\033[0m"
    exit 1
  else
    echo -e "\033[1;32m✓ All files meet ${FAIL_UNDER}% coverage threshold\033[0m"
  fi
fi
