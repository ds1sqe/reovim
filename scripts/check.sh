#!/usr/bin/env bash
# Hosted tooling and Product-empty v2 conformance verification for v0.16.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
    cat <<'EOF'
Usage: ./scripts/check.sh

Checks normative/public documentation, the active Product-empty v2 Source Role
Map boundary, and every hosted workspace target. This command runs candidate
mode; exact clean-revision evidence uses scripts/check-conformance.sh exact.
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

echo "==> documentation"
./scripts/check-documentation.sh

echo "==> workspace tests and conformance fixtures"
cargo test --locked --offline --workspace --all-targets --all-features

echo "==> candidate repository conformance"
./scripts/check-conformance.sh candidate

echo "==> rustfmt"
cargo fmt --all --check

echo "==> workspace doctests"
cargo test --locked --offline --workspace --doc --all-features

echo "==> workspace clippy"
cargo clippy --locked --offline --workspace --all-targets --all-features -- \
    -D warnings -F unsafe-code

echo "==> workspace rustdoc"
RUSTDOCFLAGS="${RUSTDOCFLAGS:+$RUSTDOCFLAGS }-D warnings -F unsafe-code" \
    cargo doc --locked --offline --workspace --all-features --no-deps

echo "check.sh: hosted tooling and candidate conformance checks passed"
