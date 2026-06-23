#!/usr/bin/env bash
# Run all Raspberry Pi 4 target-local smokes that do not touch real media.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET_DIR="$ROOT/apps/os/targets/raspi4b-aarch64"
README="$TARGET_DIR/README.md"

cd "$ROOT"

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 2
}

printf '==> syntax check Raspberry Pi 4 target scripts\n'
bash -n \
    "$TARGET_DIR/build-image.sh" \
    "$TARGET_DIR/check-bootfs.sh" \
    "$TARGET_DIR/find-bootfs.sh" \
    "$TARGET_DIR/install-image.sh" \
    "$TARGET_DIR/preflight-real-board.sh" \
    "$TARGET_DIR/prepare-boot-media.sh" \
    "$TARGET_DIR/test-check-bootfs.sh" \
    "$TARGET_DIR/test-find-bootfs.sh" \
    "$TARGET_DIR/test-install-image.sh" \
    "$TARGET_DIR/test-preflight-real-board.sh" \
    "$TARGET_DIR/test-prepare-boot-media.sh" \
    "$TARGET_DIR/test-proof-command-list.sh" \
    "$TARGET_DIR/test-target-smokes.sh" \
    "$TARGET_DIR/test-validate-evidence.sh" \
    "$TARGET_DIR/validate-evidence.sh"

printf '==> bootfs readiness smoke\n'
"$TARGET_DIR/test-check-bootfs.sh"

printf '==> bootfs discovery smoke\n'
"$TARGET_DIR/test-find-bootfs.sh"

printf '==> build no-bootline Raspberry Pi 4 image\n'
build_output="$("$TARGET_DIR/build-image.sh")"
printf '%s\n' "$build_output"
image_bytes="$(printf '%s\n' "$build_output" | awk -F= '/^bytes=[1-9][0-9]*$/ { print $2; exit }')"
if [ -z "$image_bytes" ]; then
    fail "build-image.sh did not report image bytes"
fi
readme_bytes="$(awk '/^Current expected size is `[1-9][0-9]*` bytes\.$/ {
    gsub(/`/, "", $5);
    print $5;
    exit;
}' "$README")"
if [ -z "$readme_bytes" ]; then
    fail "README current expected size line is missing or malformed"
fi
if [ "$readme_bytes" != "$image_bytes" ]; then
    fail "README current expected size is stale: expected $image_bytes got $readme_bytes"
fi

printf '==> installer smoke\n'
"$TARGET_DIR/test-install-image.sh"

printf '==> preflight evidence smoke\n'
"$TARGET_DIR/test-preflight-real-board.sh"

printf '==> proof contract smoke\n'
"$TARGET_DIR/test-proof-command-list.sh"

printf '==> media preparation smoke\n'
"$TARGET_DIR/test-prepare-boot-media.sh"

printf '==> completed evidence validator smoke\n'
"$TARGET_DIR/test-validate-evidence.sh"

printf 'raspi4b target smokes ok\n'
