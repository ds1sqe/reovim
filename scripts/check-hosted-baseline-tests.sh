#!/usr/bin/env bash
# Negative fixtures for the Plan 03 hosted-baseline guard.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK="$ROOT/scripts/check-hosted-baseline.sh"
FIXTURE="$(mktemp -d)"
trap 'rm -rf "$FIXTURE"' EXIT

mkdir -p "$FIXTURE/lib/depgraph/src"
printf '%s\n' '[workspace]' 'members = ["lib/depgraph"]' > "$FIXTURE/Cargo.toml"
printf '%s\n' 'version = 4' > "$FIXTURE/Cargo.lock"
printf '%s\n' '[package]' 'name = "reovim-depgraph"' 'version = "0.0.0"' > \
    "$FIXTURE/lib/depgraph/Cargo.toml"
printf '%s\n' '#![forbid(unsafe_code)]' > "$FIXTURE/lib/depgraph/src/lib.rs"

expect_failure() {
    local label="$1"
    if "$CHECK" "$FIXTURE" >/dev/null 2>&1; then
        printf 'check-hosted-baseline-tests: expected failure for %s\n' "$label" >&2
        exit 1
    fi
}

"$CHECK" "$FIXTURE" >/dev/null

mkdir -p "$FIXTURE/system/lib/kernel"
printf '%s\n' '// forbidden donor path' > "$FIXTURE/system/lib/kernel/ghost.rs"
expect_failure 'removed product/runtime path'
rm -rf "$FIXTURE/system"

mkdir -p "$FIXTURE/extra"
printf '%s\n' '[package]' 'name = "unexpected"' 'version = "0.0.0"' > \
    "$FIXTURE/extra/Cargo.toml"
expect_failure 'unexpected Cargo manifest'
rm -rf "$FIXTURE/extra"

printf '%s\n' '#![no_std]' > "$FIXTURE/lib/depgraph/src/freestanding.rs"
expect_failure 'freestanding crate attribute'
rm "$FIXTURE/lib/depgraph/src/freestanding.rs"

printf '%s\n' '[dependencies]' 'legacy = { path = "../../system/lib/kernel" }' >> \
    "$FIXTURE/lib/depgraph/Cargo.toml"
expect_failure 'removed path dependency'
printf '%s\n' '[package]' 'name = "reovim-depgraph"' 'version = "0.0.0"' > \
    "$FIXTURE/lib/depgraph/Cargo.toml"

"$CHECK" "$FIXTURE" >/dev/null
printf 'check-hosted-baseline-tests: positive control and 4 negative fixtures passed\n'
