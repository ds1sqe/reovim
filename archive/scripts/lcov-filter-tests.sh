#!/usr/bin/env bash
set -euo pipefail

# Usage: ./scripts/lcov-filter-tests.sh <input.lcov> [output.lcov]
#
# Strips #[cfg(test)] regions from LCOV data. For each source file, finds the
# first #[cfg(test)] line and removes all DA/BRDA entries at or after that line.
# Recalculates LF/LH/BRF/BRH counts.
#
# If output is omitted, overwrites the input file in-place.

INPUT="${1:?Usage: lcov-filter-tests.sh <input.lcov> [output.lcov]}"
OUTPUT="${2:-$INPUT}"

if [ ! -f "$INPUT" ]; then
  echo "Error: $INPUT not found" >&2
  exit 1
fi

awk '
function find_test_line(filepath,    cmd, line, num) {
  if (filepath in test_line_cache) return test_line_cache[filepath]
  # Find the first #[cfg(test)] in the source file
  num = 0
  cmd = "grep -n \"#\\[cfg(test)\\]\" \"" filepath "\" 2>/dev/null | head -1"
  if ((cmd | getline line) > 0) {
    split(line, parts, ":")
    num = parts[1] + 0
  }
  close(cmd)
  test_line_cache[filepath] = num
  return num
}

/^SF:/ {
  cur_file = substr($0, 4)
  test_start = find_test_line(cur_file)
  # Buffer the SF line
  buf[++buf_n] = $0
  lf = 0; lh = 0; brf = 0; brh = 0
  next
}

/^DA:/ {
  val = substr($0, 4)
  split(val, da, ",")
  line_num = da[1] + 0
  count = da[2] + 0
  # Skip if inside test region
  if (test_start > 0 && line_num >= test_start) next
  buf[++buf_n] = $0
  lf++
  if (count > 0) lh++
  next
}

/^BRDA:/ {
  # BRDA:line,block,branch,count
  val = substr($0, 6)
  split(val, br, ",")
  line_num = br[1] + 0
  if (test_start > 0 && line_num >= test_start) next
  buf[++buf_n] = $0
  brf++
  if (br[4] != "-" && br[4] + 0 > 0) brh++
  next
}

# Skip original LF/LH/BRF/BRH (we recalculate)
/^LF:/ || /^LH:/ || /^BRF:/ || /^BRH:/ { next }

# Skip FN/FNDA/FNF/FNH in test regions
/^FN:/ {
  val = substr($0, 4)
  split(val, fn_parts, ",")
  line_num = fn_parts[1] + 0
  if (test_start > 0 && line_num >= test_start) next
  buf[++buf_n] = $0
  next
}

/^FNDA:/ {
  # FNDA:count,name - cannot filter by line, keep all (they reference function names)
  buf[++buf_n] = $0
  next
}

/^FNF:/ || /^FNH:/ {
  # Recalculating function counts is complex; pass through for now
  buf[++buf_n] = $0
  next
}

/^end_of_record/ {
  # Only emit if there are DA lines left
  if (lf > 0) {
    for (i = 1; i <= buf_n; i++) print buf[i]
    print "LF:" lf
    print "LH:" lh
    if (brf > 0) {
      print "BRF:" brf
      print "BRH:" brh
    }
    print "end_of_record"
  }
  # Reset
  delete buf
  buf_n = 0
  lf = 0; lh = 0; brf = 0; brh = 0
  cur_file = ""
  test_start = 0
  next
}

# Other lines (TN: etc) - pass through
{ buf[++buf_n] = $0 }
' "$INPUT" > "${OUTPUT}.tmp"

mv "${OUTPUT}.tmp" "$OUTPUT"
