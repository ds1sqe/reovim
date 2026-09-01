#!/usr/bin/env bash
# Hosted tooling verification for the v0.16 excision checkpoint.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
    cat <<'EOF'
Usage: ./scripts/check.sh

Checks normative/public documentation and the retained hosted depgraph tool.
This checkpoint has no shipping editor, nested workspace, custom target,
freestanding runtime, or target-specific test lane.
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
"$ROOT/scripts/check-documentation.sh"

echo "==> hosted-baseline guard"
"$ROOT/scripts/check-hosted-baseline.sh"
"$ROOT/scripts/check-hosted-baseline-tests.sh"

echo "==> hosted workspace metadata"
cargo metadata --locked --format-version 1 --no-deps > /dev/null

echo "==> rustfmt"
cargo fmt --all --check

echo "==> depgraph tests"
cargo test --locked -p reovim-depgraph --all-targets --all-features

echo "==> depgraph doctests"
cargo test --locked -p reovim-depgraph --doc --all-features

echo "==> depgraph clippy"
cargo clippy --locked -p reovim-depgraph --all-targets --all-features -- \
    -D warnings

echo "==> depgraph rustdoc"
RUSTDOCFLAGS="${RUSTDOCFLAGS:+$RUSTDOCFLAGS }-D warnings" \
    cargo doc --locked -p reovim-depgraph --all-features --no-deps

echo "check.sh: hosted tooling checks passed"
