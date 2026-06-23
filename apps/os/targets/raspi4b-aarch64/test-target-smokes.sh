#!/usr/bin/env bash
# Run all Raspberry Pi 4 target-local smokes that do not touch real media.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET_DIR="$ROOT/apps/os/targets/raspi4b-aarch64"

cd "$ROOT"

printf '==> syntax check Raspberry Pi 4 target scripts\n'
bash -n \
    "$TARGET_DIR/build-image.sh" \
    "$TARGET_DIR/check-bootfs.sh" \
    "$TARGET_DIR/install-image.sh" \
    "$TARGET_DIR/preflight-real-board.sh" \
    "$TARGET_DIR/prepare-boot-media.sh" \
    "$TARGET_DIR/test-check-bootfs.sh" \
    "$TARGET_DIR/test-install-image.sh" \
    "$TARGET_DIR/test-preflight-real-board.sh" \
    "$TARGET_DIR/test-prepare-boot-media.sh" \
    "$TARGET_DIR/test-proof-command-list.sh" \
    "$TARGET_DIR/test-target-smokes.sh" \
    "$TARGET_DIR/test-validate-evidence.sh" \
    "$TARGET_DIR/validate-evidence.sh"

printf '==> bootfs readiness smoke\n'
"$TARGET_DIR/test-check-bootfs.sh"

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
