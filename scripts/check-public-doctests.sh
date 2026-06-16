#!/usr/bin/env bash
set -euo pipefail

# ─── check-public-doctests.sh ─────────────────────────────────────
# Enforces the L12 public-doc-test rule:
#   every public Rust API item must carry an adjacent rustdoc doctest.
#
# This is a coverage check for doctest presence. `cargo test --doc`
# remains the execution check for examples that exist.
#
# Checked items:
#   pub struct/enum/trait/fn/type/const/static/union/macro
#   public associated items inside impl blocks
#
# Intentionally skipped:
#   pub(crate), pub(super), pub(in ...)
#   pub use re-exports
#   pub mod declarations, whose module docs live in the target file
#   test files and fixture crates
#
# Usage:
#   ./scripts/check-public-doctests.sh --check
#   ./scripts/check-public-doctests.sh --check --strict
#   ./scripts/check-public-doctests.sh --print-baseline
#   ./scripts/check-public-doctests.sh --fix
# ────────────────────────────────────────────────────────────────────

show_help() {
    sed -n '2,33p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

SHOW_FIX=false
DO_CHECK=false
STRICT=false
PRINT_BASELINE=false
BASELINE_ARG=""
for arg in "$@"; do
    case "$arg" in
        --check) DO_CHECK=true ;;
        --fix) SHOW_FIX=true; DO_CHECK=true ;;
        --strict) STRICT=true ;;
        --print-baseline) PRINT_BASELINE=true; DO_CHECK=true ;;
        --baseline=*) BASELINE_ARG="${arg#*=}" ;;
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

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASELINE="${BASELINE_ARG:-$ROOT/scripts/public-doctests.baseline}"

SEARCH_DIRS=(
    apps
    arch
    clients
    ext
    lib
    server
    tools
    uapi
)

tmp_files="$(mktemp)"
tmp_missing="$(mktemp)"
tmp_missing_keys="$(mktemp)"
tmp_baseline_keys="$(mktemp)"
tmp_new="$(mktemp)"
tmp_stale="$(mktemp)"
trap 'rm -f "$tmp_files" "$tmp_missing" "$tmp_missing_keys" "$tmp_baseline_keys" "$tmp_new" "$tmp_stale"' EXIT

find_lib_sources() {
    local dir
    for dir in "${SEARCH_DIRS[@]}"; do
        [[ -d "$ROOT/$dir" ]] || continue
        find "$ROOT/$dir" -path '*/src/lib.rs' -type f \
            ! -path '*/target/*' \
            ! -path '*/archive/*' \
            ! -path '*/tests/fixtures/*' \
            ! -path '*/tests/*' \
            2>/dev/null
    done | sort -u | while IFS= read -r lib_file; do
        find "$(dirname "$lib_file")" -name '*.rs' -type f \
            ! -name '*_tests.rs' \
            ! -name 'tests.rs' \
            ! -name 'main.rs' \
            ! -path '*/bin/*' \
            ! -path '*/target/*' \
            ! -path '*/archive/*' \
            ! -path '*/tests/fixtures/*' \
            ! -path '*/tests/*' \
            2>/dev/null
    done | sort -u
}

find_lib_sources > "$tmp_files"

awk '
function ltrim(s) {
    sub(/^[ \t\r\n]+/, "", s)
    return s
}

function summarize(line, s) {
    s = ltrim(line)
    sub(/[ \t]*[{=].*$/, "", s)
    sub(/[ \t]+$/, "", s)
    return s
}

function brace_delta(line, i, ch, delta) {
    delta = 0
    for (i = 1; i <= length(line); i++) {
        ch = substr(line, i, 1)
        if (ch == "{") {
            delta++
        } else if (ch == "}") {
            delta--
        }
    }
    return delta
}

function doctest_fence(line, payload, info) {
    payload = line
    sub(/^[ \t]*\/\/\/[ \t]?/, "", payload)
    if (payload !~ /^```/) {
        return 0
    }

    info = payload
    sub(/^```[ \t]*/, "", info)
    sub(/[ \t].*$/, "", info)

    return info == "" \
        || info ~ /(^|,)(rust|no_run|ignore|compile_fail|should_panic)(,|$)/ \
        || info ~ /(^|,)edition[0-9][0-9][0-9][0-9](,|$)/
}

function is_public_api_item(line, s) {
    s = ltrim(line)

    if (s !~ /^pub([ \t]|$)/) {
        return 0
    }
    if (s ~ /^pub[ \t]*\(/) {
        return 0
    }
    if (s ~ /^pub[ \t]+use[ \t{]/) {
        return 0
    }
    if (s ~ /^pub[ \t]+mod[ \t]+/) {
        return 0
    }

    sub(/^pub[ \t]+/, "", s)

    return s ~ /^(struct|enum|trait|type|static|union|macro)([ \t<{(]|$)/ \
        || s ~ /^const[ \t]+fn[ \t<]/ \
        || s ~ /^const[ \t]+unsafe[ \t]+fn[ \t<]/ \
        || s ~ /^const[ \t]+extern[ \t]+"[^"]+"[ \t]+fn[ \t<]/ \
        || s ~ /^const[ \t]+[A-Za-z_][A-Za-z0-9_]*[ \t:=>]/ \
        || s ~ /^async[ \t]+fn[ \t<]/ \
        || s ~ /^unsafe[ \t]+fn[ \t<]/ \
        || s ~ /^unsafe[ \t]+extern[ \t]+"[^"]+"[ \t]+fn[ \t<]/ \
        || s ~ /^extern[ \t]+"[^"]+"[ \t]+fn[ \t<]/ \
        || s ~ /^fn[ \t<]/
}

FNR == 1 {
    pending_doc = 0
    pending_doctest = 0
    pending_doc_hidden = 0
    macro_depth = 0
}

{
    line = $0

    if (macro_depth > 0) {
        macro_depth += brace_delta(line)
        if (macro_depth < 0) {
            macro_depth = 0
        }
        next
    }

    if (line ~ /^[ \t]*\/\/\//) {
        pending_doc = 1
        if (doctest_fence(line)) {
            pending_doctest = 1
        }
        next
    }

    if (line ~ /^[ \t]*$/) {
        next
    }

    if (line ~ /^[ \t]*#\[/) {
        if (line ~ /#\[doc\(hidden\)\]/) {
            pending_doc_hidden = 1
        }
        next
    }

    if (line ~ /^[ \t]*macro_rules![ \t]+[A-Za-z_][A-Za-z0-9_]*[ \t]*[({]/) {
        macro_depth = brace_delta(line)
        pending_doc = 0
        pending_doctest = 0
        pending_doc_hidden = 0
        next
    }

    if (is_public_api_item(line)) {
        if (!pending_doc_hidden && (!pending_doc || !pending_doctest)) {
            rel = FILENAME
            root_prefix = root "/"
            if (index(rel, root_prefix) == 1) {
                rel = substr(rel, length(root_prefix) + 1)
            }
            print rel "\t" FNR "\t" summarize(line)
        }
    }

    pending_doc = 0
    pending_doctest = 0
    pending_doc_hidden = 0
}
' root="$ROOT" $(cat "$tmp_files") > "$tmp_missing"

awk -F '\t' '{ print $1 "\t" $3 }' "$tmp_missing" | sort -u > "$tmp_missing_keys"

if [[ "$PRINT_BASELINE" == true ]]; then
    cat <<EOF
# Existing public API doctest gaps when the L12 scanner landed.
# Format: relative/path<TAB>public item signature
EOF
    cat "$tmp_missing_keys"
    exit 0
fi

if [[ "$STRICT" == false && -f "$BASELINE" ]]; then
    grep -vE '^[[:space:]]*(#|$)' "$BASELINE" | sort -u > "$tmp_baseline_keys"
    comm -23 "$tmp_missing_keys" "$tmp_baseline_keys" > "$tmp_new"
    comm -13 "$tmp_missing_keys" "$tmp_baseline_keys" > "$tmp_stale"
else
    cp "$tmp_missing_keys" "$tmp_new"
    : > "$tmp_stale"
fi

if [[ ! -s "$tmp_new" && ! -s "$tmp_stale" ]]; then
    if [[ "$STRICT" == false && -f "$BASELINE" ]]; then
        baseline_count="$(wc -l < "$tmp_missing_keys" | tr -d ' ')"
        echo "OK: No new public doctest gaps. $baseline_count baseline gap(s) remain; run --strict to audit all."
    else
        echo "OK: Public API items have adjacent rustdoc doctests."
    fi
    exit 0
fi

if [[ -s "$tmp_new" ]]; then
    violation_count="$(wc -l < "$tmp_new" | tr -d ' ')"
    if [[ "$STRICT" == true || ! -f "$BASELINE" ]]; then
        echo "FAIL: Found $violation_count public API item(s) without adjacent doctests."
    else
        echo "FAIL: Found $violation_count new public API item(s) without adjacent doctests."
    fi
    echo ""
    while IFS=$'\t' read -r rel summary; do
        awk -F '\t' -v rel="$rel" -v summary="$summary" \
            '$1 == rel && $3 == summary { print "  " $1 ":" $2 ": missing public doctest for `" $3 "`"; exit }' \
            "$tmp_missing"
    done < "$tmp_new"
fi

if [[ -s "$tmp_stale" ]]; then
    stale_count="$(wc -l < "$tmp_stale" | tr -d ' ')"
    if [[ -s "$tmp_new" ]]; then
        echo ""
    fi
    echo "FAIL: Found $stale_count stale baseline public-doctest entrie(s)."
    echo ""
    sed 's/^/  /' "$tmp_stale"
    echo ""
    echo "Remove stale entrie(s) from $BASELINE."
fi

echo ""

if [[ "$SHOW_FIX" == true ]]; then
    cat <<'EOF'

Add an adjacent rustdoc example before each listed public item:

  /// ```rust
  /// use crate_name::TypeName;
  ///
  /// let value = TypeName::new();
  /// ```
  pub struct TypeName;

Use `no_run`, `compile_fail`, or `ignore` only when the API cannot be
executed in an ordinary doctest process.
EOF
else
    echo ""
    echo "Run with --fix for guidance on resolving each violation."
fi

exit 1
