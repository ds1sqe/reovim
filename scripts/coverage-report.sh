#!/usr/bin/env bash
set -eo pipefail

# Usage: ./scripts/coverage-report.sh [lcov-file] [--server]
#
# Generates a human-readable markdown coverage report from LCOV data.
#
# Arguments:
#   lcov-file   Path to lcov.info (default: target/llvm-cov/lcov.info)
#   --server    Include only reovim-server (for line coverage report)
#
# Output:
#   COVERAGE.md (or COVERAGE-server.md with --server)

LCOV_FILE="target/llvm-cov/lcov.info"
SERVER_ONLY=false
for arg in "$@"; do
  case "$arg" in
    --server) SERVER_ONLY=true ;;
    --*) ;;
    *) LCOV_FILE="$arg" ;;
  esac
done

if [ ! -f "$LCOV_FILE" ]; then
  echo "Error: LCOV file not found: $LCOV_FILE" >&2
  echo "Run './scripts/coverage.sh line --lcov' or './scripts/coverage.sh mcdc --lcov' first." >&2
  exit 1
fi

WORKSPACE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Build crate mapping: directory -> crate name
CRATE_MAP_FILE=$(mktemp)
trap 'rm -f "$CRATE_MAP_FILE"' EXIT

# Find all Cargo.toml and extract package names
find "$WORKSPACE_ROOT" -name Cargo.toml -not -path '*/target/*' -not -path '*/archive/*' \
  -exec grep -l '^\[package\]' {} \; 2>/dev/null | while read -r toml; do
    dir="$(dirname "$toml")"
    name="$(sed -n '/^\[package\]/,/^\[/{s/^name\s*=\s*"\(.*\)"/\1/p}' "$toml" | head -1)"
    if [ -n "$name" ]; then
      echo "$dir	$name"
    fi
done | sort -t$'\t' -k1,1 > "$CRATE_MAP_FILE"

if $SERVER_ONLY; then
  OUTPUT="$WORKSPACE_ROOT/COVERAGE-server.md"
  TITLE="Server Line Coverage Report"
  MODE="line"
  FILTER="reovim-server"
else
  OUTPUT="$WORKSPACE_ROOT/COVERAGE.md"
  TITLE="Workspace MC/DC Coverage Report"
  MODE="mcdc"
  FILTER=""
fi

# Main processing: awk parses LCOV, maps to crates, outputs markdown
awk -v workspace="$WORKSPACE_ROOT" \
    -v crate_map_file="$CRATE_MAP_FILE" \
    -v title="$TITLE" \
    -v mode="$MODE" \
    -v filter="$FILTER" \
    -v server_only="$SERVER_ONLY" \
    -v date_str="$(date '+%Y-%m-%d %H:%M')" '
BEGIN {
  # Load crate map (longest prefix match)
  while ((getline line < crate_map_file) > 0) {
    split(line, parts, "\t")
    crate_dirs[++n_crates] = parts[1]
    crate_names[n_crates] = parts[2]
  }
  close(crate_map_file)
}

function map_crate(filepath,    i, best, best_len) {
  best = "unknown"
  best_len = 0
  for (i = 1; i <= n_crates; i++) {
    d = crate_dirs[i]
    if (substr(filepath, 1, length(d)) == d && length(d) > best_len) {
      best = crate_names[i]
      best_len = length(d)
    }
  }
  return best
}

function short(path) {
  if (substr(path, 1, length(workspace) + 1) == workspace "/")
    return substr(path, length(workspace) + 2)
  return path
}

function pct(hit, total) {
  if (total == 0) return "—"
  return sprintf("%.1f%%", (hit / total) * 100)
}

function status_str(hit, total) {
  if (total == 0) return "—"
  p = (hit / total) * 100
  if (p >= 99.95) return "PASS"
  if (p >= 90) return "90%+"
  if (p >= 70) return "70%+"
  return "LOW"
}

# Parse LCOV records
/^SF:/ {
  cur_file = substr($0, 4)
  cur_uncov = ""
  cur_uncov_n = 0
  next
}

/^DA:/ {
  val = substr($0, 4)
  split(val, da, ",")
  if (da[2] + 0 == 0) {
    cur_uncov = cur_uncov (cur_uncov_n > 0 ? "," : "") da[1]
    cur_uncov_n++
  }
  next
}

/^LF:/ { file_lf[cur_file] = substr($0, 4) + 0; next }
/^LH:/ { file_lh[cur_file] = substr($0, 4) + 0; next }
/^BRF:/ { file_brf[cur_file] = substr($0, 4) + 0; next }
/^BRH:/ { file_brh[cur_file] = substr($0, 4) + 0; next }

/^end_of_record/ {
  if (cur_file == "") next
  crate = map_crate(cur_file)
  file_crate[cur_file] = crate

  # Filter
  if (server_only == "true" && crate != "reovim-server") { cur_file = ""; next }
  if (server_only == "false" && crate == "reovim-server") { cur_file = ""; next }

  # Aggregate per-crate
  crate_lf[crate] += file_lf[cur_file]
  crate_lh[crate] += file_lh[cur_file]
  crate_brf[crate] += file_brf[cur_file]
  crate_brh[crate] += file_brh[cur_file]
  crate_files[crate]++
  crate_seen[crate] = 1

  # Track uncovered lines
  if (cur_uncov_n > 0) {
    uncov_file[++n_uncov] = cur_file
    uncov_lines[n_uncov] = cur_uncov
    uncov_count[n_uncov] = cur_uncov_n
  }

  cur_file = ""
}

END {
  # Compute totals
  total_lf = 0; total_lh = 0; total_brf = 0; total_brh = 0; total_files = 0
  for (c in crate_seen) {
    total_lf += crate_lf[c]
    total_lh += crate_lh[c]
    total_brf += crate_brf[c]
    total_brh += crate_brh[c]
    total_files += crate_files[c]
  }

  # Header
  print "# " title
  print ""
  print "Generated: " date_str " | Mode: " mode " | Target: 100%"
  print ""
  printf "**Overall**: %s lines (%d/%d)", pct(total_lh, total_lf), total_lh, total_lf
  if (total_brf > 0)
    printf " | %s branches (%d/%d)", pct(total_brh, total_brf), total_brh, total_brf
  printf " | %d files\n\n", total_files

  # Crate summary — sort by coverage ascending
  n_sorted = 0
  for (c in crate_seen) {
    n_sorted++
    sorted_crates[n_sorted] = c
    if (crate_lf[c] > 0)
      sorted_pct[n_sorted] = (crate_lh[c] / crate_lf[c]) * 10000
    else
      sorted_pct[n_sorted] = 0
  }
  # Bubble sort (small N)
  for (i = 1; i <= n_sorted; i++)
    for (j = i + 1; j <= n_sorted; j++)
      if (sorted_pct[i] > sorted_pct[j]) {
        tmp = sorted_crates[i]; sorted_crates[i] = sorted_crates[j]; sorted_crates[j] = tmp
        tmp = sorted_pct[i]; sorted_pct[i] = sorted_pct[j]; sorted_pct[j] = tmp
      }

  print "## Crate Summary"
  print ""
  print "| Crate | Files | Lines | Hit | Miss | Line % | Status |"
  print "|-------|------:|------:|----:|-----:|-------:|--------|"

  for (i = 1; i <= n_sorted; i++) {
    c = sorted_crates[i]
    miss = crate_lf[c] - crate_lh[c]
    printf "| %s | %d | %d | %d | %d | %s | %s |\n", \
      c, crate_files[c], crate_lf[c], crate_lh[c], miss, \
      pct(crate_lh[c], crate_lf[c]), status_str(crate_lh[c], crate_lf[c])
  }
  miss_total = total_lf - total_lh
  printf "| **Total** | **%d** | **%d** | **%d** | **%d** | **%s** | |\n\n", \
    total_files, total_lf, total_lh, miss_total, pct(total_lh, total_lf)

  # Per-file gaps — grouped by crate (sorted by crate coverage ascending)
  print "## Gaps (files below 100%)"
  print ""

  any_gap = 0
  for (ci = 1; ci <= n_sorted; ci++) {
    c = sorted_crates[ci]
    if (crate_lf[c] == 0 || crate_lh[c] == crate_lf[c]) continue

    # Collect gap files for this crate, sort by coverage ascending
    n_gap = 0
    for (f in file_crate) {
      if (file_crate[f] != c) continue
      lf = file_lf[f] + 0; lh = file_lh[f] + 0
      if (lf > 0 && lh < lf) {
        n_gap++
        gap_file[n_gap] = f
        if (lf > 0) gap_pct[n_gap] = (lh / lf) * 10000
        else gap_pct[n_gap] = 0
      }
    }
    if (n_gap == 0) continue

    # Sort gap files
    for (i = 1; i <= n_gap; i++)
      for (j = i + 1; j <= n_gap; j++)
        if (gap_pct[i] > gap_pct[j]) {
          tmp = gap_file[i]; gap_file[i] = gap_file[j]; gap_file[j] = tmp
          tmp = gap_pct[i]; gap_pct[i] = gap_pct[j]; gap_pct[j] = tmp
        }

    any_gap = 1
    printf "### %s (%s)\n\n", c, pct(crate_lh[c], crate_lf[c])
    print "| File | Lines | Hit | Miss | Coverage |"
    print "|------|------:|----:|-----:|---------:|"

    for (i = 1; i <= n_gap; i++) {
      f = gap_file[i]
      lf = file_lf[f] + 0; lh = file_lh[f] + 0
      printf "| `%s` | %d | %d | %d | %s |\n", \
        short(f), lf, lh, lf - lh, pct(lh, lf)
    }
    print ""
  }
  if (!any_gap) print "All files at 100% coverage.\n"

  # Top 20 worst files with uncovered line numbers
  print "## Uncovered Lines (top 20 files by miss count)"
  print ""

  # Sort by miss count descending
  for (i = 1; i <= n_uncov; i++)
    for (j = i + 1; j <= n_uncov; j++)
      if (uncov_count[i] < uncov_count[j]) {
        tmp = uncov_file[i]; uncov_file[i] = uncov_file[j]; uncov_file[j] = tmp
        tmp = uncov_lines[i]; uncov_lines[i] = uncov_lines[j]; uncov_lines[j] = tmp
        tmp = uncov_count[i]; uncov_count[i] = uncov_count[j]; uncov_count[j] = tmp
      }

  shown = 0
  for (i = 1; i <= n_uncov && shown < 20; i++) {
    f = uncov_file[i]
    # Compress line numbers into ranges
    n_nums = split(uncov_lines[i], nums, ",")
    ranges = ""
    first = nums[1] + 0; last = first
    for (k = 2; k <= n_nums; k++) {
      v = nums[k] + 0
      if (v == last + 1) { last = v; continue }
      ranges = ranges (ranges != "" ? "," : "") \
        (first == last ? first : first "-" last)
      first = v; last = v
    }
    ranges = ranges (ranges != "" ? "," : "") \
      (first == last ? first : first "-" last)

    printf "- `%s` (**%d** uncovered): `%s`\n", short(f), uncov_count[i], ranges
    shown++
  }

  print ""
  print "---"
  print "*Generated by `scripts/coverage-report.sh`*"
}
' "$LCOV_FILE" > "$OUTPUT"

echo "Report written to: $OUTPUT"
# Show quick summary
head -6 "$OUTPUT" | tail -3
