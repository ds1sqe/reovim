#!/usr/bin/env bash
# Bounded hosted contributor checks for the v0.16 workspaces.

set -euo pipefail

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

usage() {
    cat <<'EOF'
Usage: ./scripts/check.sh

Checks normative/public documentation and every fast hosted Repository Tooling
and Product workspace target. This command targets a warm runtime of about one
minute. Exact clean-revision Product evidence uses check-conformance.sh and is
not part of this contributor gate.

The exhaustive `product_empty_v2_full_cli_and_mutation_matrix` and
`exact_git_generate_verify_and_raw_identity_mutants` tests are pre-checkpoint
gates, not per-edit checks.
EOF
}

case "${1:-}" in
    "") ;;
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

cd "$ROOT"

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
RUSTDOCFLAGS="${RUSTDOCFLAGS:+$RUSTDOCFLAGS }-D warnings -F unsafe-code" \
    cargo doc --locked --offline --workspace --all-features --no-deps

echo "==> Product workspace rustdoc"
CARGO_TARGET_DIR="$PRODUCT_TARGET_DIR" \
    RUSTDOCFLAGS="${RUSTDOCFLAGS:+$RUSTDOCFLAGS }-D warnings -F unsafe-code" \
    cargo doc --locked --offline \
        --manifest-path "$PRODUCT_MANIFEST" --workspace --all-features \
        --target "$PRODUCT_TARGET" --release --no-deps

echo "check.sh: hosted Repository Tooling and Product checks passed"
