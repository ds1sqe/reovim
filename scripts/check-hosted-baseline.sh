#!/usr/bin/env bash
# Fail-closed active-tree guard for the Plan 03 hosted-tooling checkpoint.

set -euo pipefail

ROOT="$(cd "${1:-$(dirname "${BASH_SOURCE[0]}")/..}" && pwd)"
failures=()

fail() {
    failures+=("$1")
}

relative_files() {
    find "$ROOT" \
        \( -path "$ROOT/.git" -o -path "$ROOT/target" -o -path "$ROOT/tmp" \) -prune \
        -o "$@" -print \
        | sed "s#^$ROOT/##" \
        | LC_ALL=C sort
}

for removed_root in \
    .cargo apps arch client editor kabi platform system uapi \
    lib/ds lib/testrt tools
do
    if [[ -e "$ROOT/$removed_root" ]]; then
        fail "removed product/runtime path exists: $removed_root"
    fi
done

mapfile -t cargo_configs < <(
    relative_files -type f \( -path '*/.cargo/config' -o -path '*/.cargo/config.toml' \)
)
if ((${#cargo_configs[@]} != 0)); then
    fail "custom Cargo configuration is not admitted at this checkpoint: ${cargo_configs[*]}"
fi

mapfile -t manifests < <(relative_files -type f -name Cargo.toml)
expected_manifests=(Cargo.toml lib/depgraph/Cargo.toml)
if [[ "${manifests[*]}" != "${expected_manifests[*]}" ]]; then
    fail "Cargo manifest set is not the two-file hosted baseline: ${manifests[*]:-(none)}"
fi

mapfile -t lockfiles < <(relative_files -type f -name Cargo.lock)
if [[ "${lockfiles[*]}" != "Cargo.lock" ]]; then
    fail "Cargo lockfile set is not the root-only hosted baseline: ${lockfiles[*]:-(none)}"
fi

mapfile -t build_scripts < <(relative_files -type f -name build.rs)
if ((${#build_scripts[@]} != 0)); then
    fail "build scripts are not admitted at this checkpoint: ${build_scripts[*]}"
fi

mapfile -t linker_assets < <(
    relative_files -type f \( -name 'link-*.ld' -o -name '*.lds' -o -name '*.x' \)
)
if ((${#linker_assets[@]} != 0)); then
    fail "custom linker assets are not admitted at this checkpoint: ${linker_assets[*]}"
fi

mapfile -t rust_sources < <(relative_files -type f -name '*.rs')
for source in "${rust_sources[@]}"; do
    case "$source" in
        lib/depgraph/src/*.rs) ;;
        *) fail "Rust source is outside the retained repository tool: $source" ;;
    esac
done

if ((${#rust_sources[@]} == 0)); then
    fail "retained depgraph Rust source is missing"
elif rg -n '#!\[(no_std|no_main)\]' "${rust_sources[@]/#/$ROOT/}" >/dev/null; then
    fail "freestanding crate attributes remain in active Rust source"
fi

if rg -n 'panic[[:space:]]*=[[:space:]]*"abort"' \
    "$ROOT/Cargo.toml" "$ROOT/lib/depgraph/Cargo.toml" >/dev/null; then
    fail 'panic = "abort" remains in an active Cargo manifest'
fi

if rg -n '(reovim-(arch|uapi|kabi|system-kernel|platform)|path[[:space:]]*=[[:space:]]*"[^\"]*(apps|arch|client|editor|kabi|platform|system|uapi|lib/(ds|testrt)|tools)(/|\"))' \
    "$ROOT/Cargo.toml" "$ROOT/lib/depgraph/Cargo.toml" >/dev/null; then
    fail "an active Cargo manifest names a removed product/runtime dependency"
fi

if ((${#failures[@]} != 0)); then
    printf 'check-hosted-baseline: %d failure(s)\n' "${#failures[@]}" >&2
    printf '  - %s\n' "${failures[@]}" >&2
    exit 1
fi

printf 'check-hosted-baseline: two manifests, one lockfile, depgraph-only Rust source\n'
