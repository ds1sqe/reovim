#!/usr/bin/env bash
set -euo pipefail

# ─── check-test-layout.sh ──────────────────────────────────────────
# Enforces the L12 test-layout rule :
#   no inline #[cfg(test)] mod tests { ... } blocks in impl files.
#
# Detects inline #[cfg(test)] mod tests { ... } blocks in source files.
# These must be separated into dedicated test files (*_tests.rs or tests/).
#
# Allowed patterns:
#   #[cfg(test)] mod foo_tests;             — Rule 1 (sibling test file)
#   #[cfg(test)] mod tests;                 — Rule 2/3 (tests.rs or tests/)
#   #[cfg(test)] #[path = "..."] mod tests; — #[path] redirect
#
# Forbidden pattern:
#   #[cfg(test)]
#   mod tests {
#       ...
#   }
#
# Usage:
#   ./scripts/check-test-layout.sh --check  # check all source dirs
#   ./scripts/check-test-layout.sh --fix    # show how to fix violations
#   ./scripts/check-test-layout.sh          # show help (default)
# ────────────────────────────────────────────────────────────────────

show_help() {
    echo "Usage: ./scripts/check-test-layout.sh <--check|--fix>"
    echo ""
    echo "Checks for inline test blocks that should be in separate files."
    echo ""
    echo "Options:"
    echo "  --check   Run the check (exits 1 on violations)"
    echo "  --fix     Run the check and show guidance on how to fix each violation"
    echo "  --help    Show this help"
}

SHOW_FIX=false
DO_CHECK=false
for arg in "$@"; do
    case "$arg" in
        --check) DO_CHECK=true ;;
        --fix) SHOW_FIX=true; DO_CHECK=true ;;
        --help|-h)
            show_help
            exit 0
            ;;
        *)
            echo "Unknown option: $arg" >&2
            show_help
            exit 1
            ;;
    esac
done

if [[ "$DO_CHECK" == false ]]; then
    show_help
    exit 0
fi

# Source directories to scan
SEARCH_DIRS=(
    apps/
    arch/
    clients/
    ext/
    lib/
    server/
    tools/
    uapi/
)

violations=()

while IFS= read -r file; do
    # Skip test files themselves
    case "$file" in
        *_tests.rs) continue ;;
        */tests/*.rs) continue ;;
        */tests.rs) continue ;;
    esac

    # Check for inline test blocks: mod tests { (with opening brace)
    if ! grep -qP '^\s*mod\s+tests\s*\{' "$file"; then
        continue
    fi

    # Skip test-only files: files where #[cfg(test)] appears and there is
    # no production code before it (only comments, doc-comments, blank lines).
    cfg_test_line=$(grep -nP '^\s*#\[cfg\(test\)\]' "$file" | head -1 | cut -d: -f1)
    if [[ -n "$cfg_test_line" ]]; then
        # Count non-comment, non-blank, non-attribute lines before #[cfg(test)]
        prod_lines=$(head -n "$((cfg_test_line - 1))" "$file" \
            | grep -cvP '^\s*(//.*|$)' || true)
        if [[ "$prod_lines" -eq 0 ]]; then
            continue
        fi
    fi

    violations+=("$file")
done < <(find "${SEARCH_DIRS[@]}" -name '*.rs' -type f \
    ! -path '*/target/*' \
    ! -path '*/.claude/*' \
    ! -path '*/archive/*' \
    2>/dev/null | sort)

if [[ ${#violations[@]} -eq 0 ]]; then
    echo "OK: No inline test blocks found. All tests are in separated files."
    exit 0
fi

echo "FAIL: Found ${#violations[@]} file(s) with inline test blocks."
echo ""
echo "The following files contain 'mod tests {' (inline test blocks)."
echo "Tests must be in separated files (L12)"
echo ""

for file in "${violations[@]}"; do
    line_num=$(grep -nP '^\s*mod\s+tests\s*\{' "$file" | head -1 | cut -d: -f1)
    echo "  $file:$line_num"

    if [[ "$SHOW_FIX" == true ]]; then
        dir=$(dirname "$file")
        base=$(basename "$file" .rs)

        if [[ "$base" == "mod" || "$base" == "lib" ]]; then
            echo "    -> Rule 2/3: Move tests to ${dir}/tests.rs or ${dir}/tests/"
        else
            echo "    -> Rule 1: Move tests to ${dir}/${base}_tests.rs"
            echo "       Add '#[cfg(test)] mod ${base}_tests;' to parent mod.rs/lib.rs"
        fi
        echo "    -> If tests access private items, use #[path] redirect instead"
        echo ""
    fi
done

if [[ "$SHOW_FIX" != true ]]; then
    echo ""
    echo "Run with --fix for guidance on resolving each violation."
fi

exit 1
