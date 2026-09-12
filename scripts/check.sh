#!/usr/bin/env bash
# Component-aware contributor checks for the editing-system workspace.

set -euo pipefail

export LC_ALL=C
unset GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_CEILING_DIRECTORIES \
    GIT_COMMON_DIR GIT_CONFIG_COUNT GIT_CONFIG_PARAMETERS \
    GIT_CONFIG_SYSTEM GIT_DIR GIT_DISCOVERY_ACROSS_FILESYSTEM \
    GIT_EXEC_PATH GIT_GRAFT_FILE GIT_INDEX_FILE GIT_NAMESPACE \
    GIT_OBJECT_DIRECTORY GIT_PREFIX GIT_REPLACE_REF_BASE \
    GIT_SHALLOW_FILE GIT_WORK_TREE
export GIT_CONFIG_GLOBAL=/dev/null
export GIT_CONFIG_NOSYSTEM=1
unset RIPGREP_CONFIG_PATH

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly PRODUCT_MANIFEST="$ROOT/product/Cargo.toml"
readonly PRODUCT_TARGET=x86_64-unknown-linux-gnu
readonly PORTABLE_TARGET=x86_64-unknown-none

# Rustdoc indexes from different compiler versions cannot share one slot.
PRODUCT_TARGET_SLOT="$(rustc -Vv | git hash-object --stdin)"
readonly PRODUCT_TARGET_SLOT
PRODUCT_TARGET_DIR="$ROOT/target/product-contributor/$PRODUCT_TARGET_SLOT"
readonly PRODUCT_TARGET_DIR
CHECK_RUSTDOCFLAGS="${RUSTDOCFLAGS:+$RUSTDOCFLAGS }"
CHECK_RUSTDOCFLAGS+='-D warnings -D unsafe-code'
readonly CHECK_RUSTDOCFLAGS

usage() {
    cat <<'EOF'
Usage: ./scripts/check.sh [--full]

Checks documentation, style, and affected Product components. Successful
toolchain-bound local coverage selects changed components and their reverse
dependents. Missing or uncertain coverage runs the full gate. --full always
runs uncached checks and cache fixtures; use it in CI and before checkpoints.

The current workspace contains portable contracts and public consumers.
Checks include x86_64-unknown-none library compilation, hosted tests, Clippy,
and rustdoc. Both targets must already be installed. Warm runtime targets
about one minute; this gate does not qualify a release or custom host.
EOF
}

force_full=no
case "${1:-}" in
    "") ;;
    --full) force_full=yes ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        echo "check.sh: unexpected argument: $1" >&2
        usage >&2
        exit 2
        ;;
esac
if [[ $# -gt 1 ]]; then
    echo 'check.sh: accepts at most one argument' >&2
    usage >&2
    exit 2
fi

cd "$ROOT"

run_product_checks() {
    local -a packages=("$@")

    echo "==> portable Product libraries and public consumers"
    CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
        cargo check --locked --offline \
            --manifest-path "$PRODUCT_MANIFEST" "${packages[@]}" \
            --lib --no-default-features --target "$PORTABLE_TARGET" --release

    echo "==> hosted Product workspace check"
    CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
        cargo check --locked --offline \
            --manifest-path "$PRODUCT_MANIFEST" "${packages[@]}" \
            --all-targets --all-features --target "$PRODUCT_TARGET" --release

    echo "==> Product public-boundary tests"
    CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
        cargo test --locked --offline \
            --manifest-path "$PRODUCT_MANIFEST" "${packages[@]}" \
            --all-features --target "$PRODUCT_TARGET" --release

    echo "==> Product Clippy"
    CLIPPY_CONF_DIR="$ROOT" \
        CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
        cargo clippy --locked --offline \
            --manifest-path "$PRODUCT_MANIFEST" "${packages[@]}" \
            --all-targets --all-features --target "$PRODUCT_TARGET" \
            --release -- -D warnings -D unsafe-code

    echo "==> Product rustdoc"
    CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
        RUSTDOCFLAGS="$CHECK_RUSTDOCFLAGS" \
        cargo doc --locked --offline \
            --manifest-path "$PRODUCT_MANIFEST" "${packages[@]}" \
            --all-features --target "$PRODUCT_TARGET" --release --no-deps
}

run_full_checks() {
    local fixture_directory="$1"

    echo "==> check-cache fixtures"
    ./scripts/check-cache.sh fixtures "$fixture_directory"
    echo "==> repository style"
    ./scripts/check-style.sh
    echo "==> Product rustfmt"
    cargo fmt --manifest-path "$PRODUCT_MANIFEST" --all --check
    echo "==> documentation"
    ./scripts/check-documentation.sh
    run_product_checks --workspace
}

component_package() {
    local graph="$1"
    local wanted="$2"
    local kind component workspace package root

    while IFS=$'\t' read -r kind component workspace package root; do
        [[ "$kind" == package && "$component" == "$wanted" ]] || continue
        [[ "$workspace" == product ]] || return 1
        printf '%s\n' "$package"
        return
    done <"$graph"
    return 1
}

run_selected_checks() {
    local directory="$1"
    local mode component package
    local documentation_selected=no
    local -a direct=() selected=()

    echo "==> selected repository style"
    ./scripts/check-style.sh --files0-from "$directory/changed-paths.z"
    while IFS=$'\t' read -r mode component; do
        [[ -n "$component" ]] || continue
        if [[ "$component" == documentation ]]; then
            documentation_selected=yes
            continue
        fi
        package="$(component_package \
            "$directory/graph.tsv" "$component")" || return 1
        selected+=( -p "$package" )
        if [[ "$mode" == direct ]]; then
            direct+=( -p "$package" )
        fi
    done <"$directory/plan.tsv"
    if [[ "$documentation_selected" == yes ]]; then
        echo "==> selected documentation"
        ./scripts/check-documentation.sh
    fi
    if [[ ${#direct[@]} -ne 0 ]]; then
        echo "==> selected Product rustfmt"
        cargo fmt --manifest-path "$PRODUCT_MANIFEST" \
            "${direct[@]}" --check
    fi
    [[ ${#selected[@]} -ne 0 ]] || return 0
    run_product_checks "${selected[@]}"
}

# Browser checks are small and uncached; Rust coverage cannot substitute them.
echo '==> Browser build and transport tests'
bash "$ROOT/apps/web/check.sh"

selection_directory="$(mktemp -d)"
if [[ -z "$selection_directory" || "$selection_directory" != /* \
    || ! -d "$selection_directory" || -L "$selection_directory" \
    || ! -O "$selection_directory" ]]; then
    echo 'check.sh: mktemp returned an unsafe selection directory' >&2
    exit 1
fi
readonly selection_directory
cleanup_selection() {
    if [[ -n "$selection_directory" \
        && -d "$selection_directory" \
        && ! -L "$selection_directory" \
        && -O "$selection_directory" ]]; then
        rm -rf -- "$selection_directory"
    fi
}
trap cleanup_selection EXIT

./scripts/check-cache.sh prepare "$selection_directory" "$force_full"
selection_mode="$(sed -n '1p' "$selection_directory/mode")"
selection_reason="$(sed -n '1p' "$selection_directory/reason")"
printf 'check.sh: selection=%s (%s)\n' \
    "$selection_mode" "$selection_reason"

case "$selection_mode" in
    full) run_full_checks "$selection_directory" ;;
    selected)
        sed 's/^/check.sh: selected /' "$selection_directory/plan.tsv"
        run_selected_checks "$selection_directory"
        ;;
    hit) echo 'check.sh: all cacheable component checks are current' ;;
    *)
        echo 'check.sh: cache selector returned an invalid mode' >&2
        exit 1
        ;;
esac

if ! ./scripts/check-cache.sh publish "$selection_directory"; then
    echo 'check.sh: checks passed but input coverage was not published' >&2
    exit 1
fi

echo "check.sh: editing-system contributor checks passed"
