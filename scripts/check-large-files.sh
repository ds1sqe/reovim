#!/usr/bin/env bash
# Check for oversized Rust source files in reovim-os app sources.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MAX_LINES="${MAX_REOVIM_OS_SOURCE_LINES:-3400}"

check_dir="$ROOT/apps/os/src"
mapfile -d '' files < <(rg --files --glob '*.rs' --null "${check_dir}")

violations=()

for file in "${files[@]}"; do
    rel="${file#${ROOT}/}"
    if [[ "${rel}" == "apps/os/src/boot.rs" ]]; then
        continue
    fi

    line_count="$(wc -l < "${file}")"
    if (( line_count > MAX_LINES )); then
        violations+=("$(printf '%s\t%6d' "${rel}" "$line_count")")
    fi
done

if ((${#violations[@]} == 0)); then
    echo "OK: No files over ${MAX_LINES} lines under apps/os/src."
    exit 0
fi

printf 'FAIL: Found %s file(s) over %s lines\n' "${#violations[@]}" "${MAX_LINES}"
for hit in "${violations[@]}"; do
    rel="${hit%%$'\t'*}"
    count="${hit##*$'\t'}"
    printf '  %5s %s\n' "${count}" "${rel}"
done

exit 1

