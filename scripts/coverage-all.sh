#!/usr/bin/env bash
set -euo pipefail

# Usage: ./scripts/coverage-all.sh [--report] [--clean]
#
# Runs all three coverage modes (line, branch, MC/DC) sequentially and saves
# each to a separate LCOV file. Prints a merged summary table at the end.
#
# Output files:
#   target/llvm-cov/lcov.line.info
#   target/llvm-cov/lcov.branch.info
#   target/llvm-cov/lcov.mcdc.info
#
# Options:
#   --report  Also generate COVERAGE.md via coverage-report.sh
#   --clean   Clean coverage artifacts before running
#
# LLVM bug (https://github.com/llvm/llvm-project/issues/119558):
#   Branch/MC/DC modes exclude reovim-server (SIGSEGV on #[tonic::async_trait]).
#   Line mode includes the full workspace.

REPORT=false
CLEAN=false
for arg in "$@"; do
  case "$arg" in
    --report) REPORT=true ;;
    --clean)  CLEAN=true ;;
    *) echo "Unknown option: $arg (expected: --report, --clean)" >&2; exit 1 ;;
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

OUT_DIR="target/llvm-cov"
mkdir -p "$OUT_DIR"

# Crate exclusions (always)
EXCLUDE=(
  --exclude reovim-module-macros
  --exclude reovim-driver-ffi-python
  --exclude reovim-bench
  --exclude perf-report
  --exclude runner
  --exclude reovim-driver-display
  --exclude reovim-module-layout
  --exclude reovim-module-statusline
  --exclude reovim-module-which-key
)

if $CLEAN; then
  echo -e "\033[1;33m==> Cleaning coverage artifacts...\033[0m"
  cargo +nightly llvm-cov clean --workspace 2>/dev/null || true
  echo -e "\033[0;32m✓ Clean\033[0m"
fi

# ─── Line coverage (includes reovim-server) ────────────────────────────
echo -e "\033[1;33m==> [1/3] Running line coverage...\033[0m"
cargo +nightly llvm-cov \
  --workspace "${EXCLUDE[@]}" \
  --lcov --output-path "$OUT_DIR/lcov.line.info"
echo -e "\033[0;32m✓ Line coverage: $OUT_DIR/lcov.line.info\033[0m"

# ─── Branch coverage (excludes reovim-server — LLVM #119558) ──────────
echo -e "\033[1;33m==> [2/3] Running branch coverage...\033[0m"
cargo +nightly llvm-cov --branch \
  --workspace "${EXCLUDE[@]}" --exclude reovim-server \
  --lcov --output-path "$OUT_DIR/lcov.branch.info"
echo -e "\033[0;32m✓ Branch coverage: $OUT_DIR/lcov.branch.info\033[0m"

# ─── MC/DC coverage (excludes reovim-server — LLVM #119558) ───────────
echo -e "\033[1;33m==> [3/3] Running MC/DC coverage...\033[0m"
RUSTFLAGS="-Z coverage-options=condition" cargo +nightly llvm-cov \
  --workspace "${EXCLUDE[@]}" --exclude reovim-server \
  --lcov --output-path "$OUT_DIR/lcov.mcdc.info"
echo -e "\033[0;32m✓ MC/DC coverage: $OUT_DIR/lcov.mcdc.info\033[0m"

# ─── Summary table ────────────────────────────────────────────────────
echo ""
echo -e "\033[1;36m==> Coverage Summary\033[0m"
python3 << 'PYEOF'
import os

def parse_lcov(path):
    hits = misses = 0
    n_files = n_miss_files = 0
    current_file = None
    file_misses = 0
    with open(path) as f:
        for line in f:
            if line.startswith("SF:"):
                current_file = line.strip()[3:]
                file_misses = 0
            elif line.startswith("DA:"):
                parts = line.strip()[3:].split(",")
                if int(parts[1]) == 0:
                    misses += 1
                    file_misses += 1
                else:
                    hits += 1
            elif line.startswith("end_of_record"):
                if current_file is not None:
                    n_files += 1
                    if file_misses > 0:
                        n_miss_files += 1
                current_file = None
    total = hits + misses
    pct = hits / total * 100 if total else 0
    return hits, misses, total, pct, n_files, n_miss_files

out = "target/llvm-cov"
modes = [
    ("Line",   f"{out}/lcov.line.info"),
    ("Branch", f"{out}/lcov.branch.info"),
    ("MC/DC",  f"{out}/lcov.mcdc.info"),
]

print(f"{'Mode':<10} {'Hit':>8} {'Miss':>6} {'Total':>8} {'Coverage':>10} {'Files':>6} {'w/ gaps':>8}")
print("-" * 60)
for name, path in modes:
    if not os.path.exists(path):
        print(f"{name:<10} {'(not found)':>40}")
        continue
    h, m, t, p, nf, nmf = parse_lcov(path)
    print(f"{name:<10} {h:>8,} {m:>6,} {t:>8,} {p:>9.2f}% {nf:>6} {nmf:>8}")
print("-" * 60)
PYEOF

# ─── Optional report generation ───────────────────────────────────────
if $REPORT; then
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  echo ""
  echo -e "\033[1;33m==> Generating COVERAGE.md...\033[0m"
  "$SCRIPT_DIR/coverage-report.sh" "$OUT_DIR/lcov.mcdc.info"
  echo -e "\033[0;32m✓ COVERAGE.md generated\033[0m"
fi

echo ""
echo -e "\033[1;32m==> All coverage runs complete!\033[0m"
echo "  LCOV files: $OUT_DIR/lcov.{line,branch,mcdc}.info"
