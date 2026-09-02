#!/usr/bin/env bash
# Bounded hosted contributor and Product-empty candidate checks for v0.16.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
    cat <<'EOF'
Usage: ./scripts/check.sh

Checks normative/public documentation, the active Product-empty v2 Source Role
Map boundary, and every fast hosted workspace target. This command targets a
warm runtime of about one minute. Exact clean-revision evidence uses
scripts/check-conformance.sh exact.

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

echo "==> rustfmt"
cargo fmt --all --check

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

echo "==> candidate repository conformance"
./scripts/check-conformance.sh candidate

echo "==> workspace doctests"
cargo test --locked --offline --workspace --doc --all-features

echo "==> workspace clippy"
CLIPPY_CONF_DIR="$ROOT" \
    cargo clippy --locked --offline --workspace --all-targets \
        --all-features -- -D warnings -F unsafe-code

echo "==> workspace rustdoc"
RUSTDOCFLAGS="${RUSTDOCFLAGS:+$RUSTDOCFLAGS }-D warnings -F unsafe-code" \
    cargo doc --locked --offline --workspace --all-features --no-deps

echo "check.sh: hosted tooling and candidate conformance checks passed"
