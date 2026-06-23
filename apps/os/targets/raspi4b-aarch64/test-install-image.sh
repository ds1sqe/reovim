#!/usr/bin/env bash
# Smoke-test the Raspberry Pi 4 boot-partition installer without real media.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET="aarch64-unknown-none"
PACKAGE="reovim-os"
IMAGE="$ROOT/apps/target/$TARGET/debug/$PACKAGE.kernel8.img"
BUILD_SCRIPT="$ROOT/apps/os/targets/raspi4b-aarch64/build-image.sh"
INSTALL_SCRIPT="$ROOT/apps/os/targets/raspi4b-aarch64/install-image.sh"

if [ ! -s "$IMAGE" ]; then
    "$BUILD_SCRIPT"
fi

tmp_bootfs="$(mktemp -d)"
cleanup() {
    rm -rf "$tmp_bootfs"
}
trap cleanup EXIT

: >"$tmp_bootfs/start4.elf"
: >"$tmp_bootfs/fixup4.dat"
: >"$tmp_bootfs/bcm2711-rpi-4-b.dtb"

source_sha="$(sha256sum "$IMAGE" | awk '{ print $1 }')"

dry_run="$("$INSTALL_SCRIPT" --dry-run "$tmp_bootfs")"
case "$dry_run" in
    *"dry_run=true"*) ;;
    *)
        printf 'error: dry-run output did not include dry_run=true\n' >&2
        printf '%s\n' "$dry_run" >&2
        exit 1
        ;;
esac
case "$dry_run" in
    *"$source_sha"*) ;;
    *)
        printf 'error: dry-run output did not include expected SHA-256\n' >&2
        printf '%s\n' "$dry_run" >&2
        exit 1
        ;;
esac

"$INSTALL_SCRIPT" "$tmp_bootfs" >/dev/null
first_sha="$(sha256sum "$tmp_bootfs/kernel8.img" | awk '{ print $1 }')"
if [ "$first_sha" != "$source_sha" ]; then
    printf 'error: installed image hash mismatch after first install\n' >&2
    exit 1
fi

"$INSTALL_SCRIPT" "$tmp_bootfs" >/dev/null
"$INSTALL_SCRIPT" "$tmp_bootfs" >/dev/null

second_sha="$(sha256sum "$tmp_bootfs/kernel8.img" | awk '{ print $1 }')"
if [ "$second_sha" != "$source_sha" ]; then
    printf 'error: installed image hash mismatch after repeated install\n' >&2
    exit 1
fi

backup_count="$(find "$tmp_bootfs" -maxdepth 1 -name 'kernel8.img.bak-*' | wc -l | tr -d ' ')"
if [ "$backup_count" != 2 ]; then
    printf 'error: expected 2 backups after repeated install, got %s\n' "$backup_count" >&2
    find "$tmp_bootfs" -maxdepth 1 -type f -printf '%f\n' >&2
    exit 1
fi

printf 'install-image smoke ok\n'
printf 'bytes=%s\n' "$(wc -c <"$IMAGE" | tr -d ' ')"
printf 'sha256=%s\n' "$source_sha"
