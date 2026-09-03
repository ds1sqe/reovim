#!/usr/bin/env bash
# Bounded hosted contributor checks for the v0.16 workspaces.

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

case "${RUSTUP_TOOLCHAIN:-}" in
    1.92.0 | 1.92.0-*)
        PRODUCT_TARGET_SLOT=rust-1.92
        ;;
    stable | stable-*)
        PRODUCT_TARGET_SLOT=rust-stable
        ;;
    *)
        PRODUCT_TARGET_SLOT=active
        ;;
esac
PRODUCT_TARGET_DIR="$ROOT/target/product-contributor/$PRODUCT_TARGET_SLOT"
readonly PRODUCT_TARGET_DIR
CHECK_RUSTDOCFLAGS="${RUSTDOCFLAGS:+$RUSTDOCFLAGS }"
CHECK_RUSTDOCFLAGS+='-D warnings -F unsafe-code'
readonly CHECK_RUSTDOCFLAGS

usage() {
    cat <<'EOF'
Usage: ./scripts/check.sh [--full]

Checks normative/public documentation and every fast hosted Repository Tooling
and Product workspace target affected by working inputs. The first run for one
tool context is full and seeds ignored local coverage. --full always executes
the complete uncached gate. This command targets a warm runtime of about one
minute. Exact clean-revision Product evidence uses check-conformance.sh.

The exhaustive `product_empty_v2_full_cli_and_mutation_matrix` and
`exact_git_generate_verify_and_raw_identity_mutants` tests are pre-checkpoint
gates, not per-edit checks.
EOF
}

force_full=no
case "${1:-}" in
    "") ;;
    --full)
        force_full=yes
        ;;
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

run_full_checks() {
    local fixture_directory="$1"

    echo "==> check-cache fixtures"
    ./scripts/check-cache.sh fixtures "$fixture_directory"

    echo "==> repository style"
    ./scripts/check-style.sh

    echo "==> Repository Tooling rustfmt"
    cargo fmt --all --check

    echo "==> Product rustfmt"
    cargo fmt --manifest-path "$PRODUCT_MANIFEST" --all --check

    echo "==> documentation"
    ./scripts/check-documentation.sh

    echo "==> workspace library and binary tests"
    cargo test --locked --offline --workspace --lib --bins --all-features

    echo "==> fast checker integration tests"
    cargo test --locked --offline -p reovim-conformance --test checker \
        --all-features -- \
        --skip product_empty_v2_full_cli_and_mutation_matrix \
        --skip exact_git_generate_verify_and_raw_identity_mutants

    echo "==> dependency-graph integration tests"
    cargo test --locked --offline -p reovim-depgraph --test depgraph \
        --all-features

    echo "==> Product workspace check"
    CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
        cargo check --locked --offline \
            --manifest-path "$PRODUCT_MANIFEST" --workspace --all-targets \
            --all-features --target "$PRODUCT_TARGET" --release

    echo "==> Product public-boundary tests"
    CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
        cargo test --locked --offline \
            --manifest-path "$PRODUCT_MANIFEST" --workspace --all-features \
            --target "$PRODUCT_TARGET" --release

    echo "==> workspace doctests"
    cargo test --locked --offline --workspace --doc --all-features

    echo "==> workspace clippy"
    CLIPPY_CONF_DIR="$ROOT" \
        cargo clippy --locked --offline --workspace --all-targets \
            --all-features -- -D warnings -F unsafe-code

    echo "==> Product workspace clippy"
    CLIPPY_CONF_DIR="$ROOT" \
        CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
        cargo clippy --locked --offline \
            --manifest-path "$PRODUCT_MANIFEST" --workspace --all-targets \
            --all-features --target "$PRODUCT_TARGET" --release -- \
            -D warnings -F unsafe-code

    echo "==> workspace rustdoc"
    RUSTDOCFLAGS="$CHECK_RUSTDOCFLAGS" \
        cargo doc --locked --offline --workspace --all-features --no-deps

    echo "==> Product workspace rustdoc"
    CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
        RUSTDOCFLAGS="$CHECK_RUSTDOCFLAGS" \
        cargo doc --locked --offline \
            --manifest-path "$PRODUCT_MANIFEST" --workspace --all-features \
            --target "$PRODUCT_TARGET" --release --no-deps
}

component_fields() {
    local graph="$1"
    local wanted="$2"
    local kind component workspace package root

    while IFS=$'\t' read -r \
        kind component workspace package root; do
        [[ "$kind" == package ]] || continue
        [[ "$component" == "$wanted" ]] || continue
        printf '%s\t%s\n' "$workspace" "$package"
        return
    done <"$graph"
    return 1
}

append_package_args() {
    local output_name="$1"
    shift
    local -n output="$output_name"
    local package

    for package in "$@"; do
        output+=(
            -p
            "$package"
        )
    done
}

package_selected() {
    local wanted="$1"
    shift
    local package

    for package in "$@"; do
        [[ "$package" == "$wanted" ]] && return 0
    done
    return 1
}

run_root_integration_tests() {
    local -a packages=("$@")
    local package

    if package_selected reovim-conformance "${packages[@]}"; then
        echo "==> selected fast checker integration tests"
        cargo test --locked --offline \
            -p reovim-conformance --test checker --all-features -- \
            --skip product_empty_v2_full_cli_and_mutation_matrix \
            --skip exact_git_generate_verify_and_raw_identity_mutants
    fi
    if package_selected reovim-depgraph "${packages[@]}"; then
        echo "==> selected dependency-graph integration tests"
        cargo test --locked --offline \
            -p reovim-depgraph --test depgraph --all-features
    fi
    for package in "${packages[@]}"; do
        case "$package" in
            reovim-conformance | reovim-depgraph) continue ;;
        esac
        echo "==> selected $package integration tests"
        cargo test --locked --offline \
            -p "$package" --tests --all-features
    done
}

run_selected_root_checks() {
    local direct_name="$1"
    local selected_name="$2"
    local -n direct="$direct_name"
    local -n selected="$selected_name"
    local -a direct_args=() selected_args=()

    [[ ${#selected[@]} -ne 0 ]] || return 0
    append_package_args selected_args "${selected[@]}"
    if [[ ${#direct[@]} -ne 0 ]]; then
        append_package_args direct_args "${direct[@]}"
        echo "==> selected Repository Tooling rustfmt"
        cargo fmt "${direct_args[@]}" --check
    fi
    echo "==> selected workspace library and binary tests"
    cargo test --locked --offline "${selected_args[@]}" \
        --lib --bins --all-features
    run_root_integration_tests "${selected[@]}"
    echo "==> selected workspace doctests"
    cargo test --locked --offline "${selected_args[@]}" --doc --all-features
    echo "==> selected workspace clippy"
    CLIPPY_CONF_DIR="$ROOT" \
        cargo clippy --locked --offline "${selected_args[@]}" \
            --all-targets --all-features -- -D warnings -F unsafe-code
    echo "==> selected workspace rustdoc"
    RUSTDOCFLAGS="$CHECK_RUSTDOCFLAGS" \
        cargo doc --locked --offline "${selected_args[@]}" \
            --all-features --no-deps
}

run_selected_product_checks() {
    local direct_name="$1"
    local selected_name="$2"
    local -n direct="$direct_name"
    local -n selected="$selected_name"
    local -a direct_args=() selected_args=()

    [[ ${#selected[@]} -ne 0 ]] || return 0
    append_package_args selected_args "${selected[@]}"
    if [[ ${#direct[@]} -ne 0 ]]; then
        append_package_args direct_args "${direct[@]}"
        echo "==> selected Product rustfmt"
        cargo fmt --manifest-path "$PRODUCT_MANIFEST" \
            "${direct_args[@]}" --check
    fi
    echo "==> selected Product workspace check"
    CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
        cargo check --locked --offline \
            --manifest-path "$PRODUCT_MANIFEST" "${selected_args[@]}" \
            --all-targets --all-features --target "$PRODUCT_TARGET" --release
    echo "==> selected Product public-boundary tests"
    CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
        cargo test --locked --offline \
            --manifest-path "$PRODUCT_MANIFEST" "${selected_args[@]}" \
            --all-features --target "$PRODUCT_TARGET" --release
    echo "==> selected Product workspace clippy"
    CLIPPY_CONF_DIR="$ROOT" \
        CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
        cargo clippy --locked --offline \
            --manifest-path "$PRODUCT_MANIFEST" "${selected_args[@]}" \
            --all-targets --all-features --target "$PRODUCT_TARGET" \
            --release -- -D warnings -F unsafe-code
    echo "==> selected Product workspace rustdoc"
    CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
        RUSTDOCFLAGS="$CHECK_RUSTDOCFLAGS" \
        cargo doc --locked --offline \
            --manifest-path "$PRODUCT_MANIFEST" "${selected_args[@]}" \
            --all-features --target "$PRODUCT_TARGET" --release --no-deps
}

run_selected_checks() {
    local directory="$1"
    local graph="$directory/graph.tsv"
    local mode component fields workspace package
    local documentation_selected=no
    local -a root_direct=() root_selected=()
    local -a product_direct=() product_selected=()

    echo "==> selected repository style"
    ./scripts/check-style.sh \
        --files0-from "$directory/changed-paths.z"
    while IFS=$'\t' read -r mode component; do
        [[ -n "$component" ]] || continue
        if [[ "$component" == documentation ]]; then
            documentation_selected=yes
            continue
        fi
        fields="$(component_fields "$graph" "$component")" || return 1
        IFS=$'\t' read -r workspace package <<<"$fields"
        case "$workspace" in
            repository-tooling)
                root_selected+=(
                    "$package"
                )
                if [[ "$mode" == direct ]]; then
                    root_direct+=(
                        "$package"
                    )
                fi
                ;;
            product)
                product_selected+=(
                    "$package"
                )
                if [[ "$mode" == direct ]]; then
                    product_direct+=(
                        "$package"
                    )
                fi
                ;;
            *) return 1 ;;
        esac
    done <"$directory/plan.tsv"
    if [[ "$documentation_selected" == yes ]]; then
        echo "==> selected documentation"
        ./scripts/check-documentation.sh
    fi
    run_selected_root_checks root_direct root_selected
    run_selected_product_checks product_direct product_selected
}

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
    full)
        run_full_checks "$selection_directory"
        ;;
    selected)
        sed 's/^/check.sh: selected /' \
            "$selection_directory/plan.tsv"
        run_selected_checks "$selection_directory"
        ;;
    hit)
        echo 'check.sh: all cacheable component checks are current'
        ;;
    *)
        echo 'check.sh: cache selector returned an invalid mode' >&2
        exit 1
        ;;
esac

if ! ./scripts/check-cache.sh publish "$selection_directory"; then
    echo 'check.sh: checks passed but input coverage was not published' >&2
    exit 1
fi

echo "check.sh: hosted Repository Tooling and Product checks passed"
