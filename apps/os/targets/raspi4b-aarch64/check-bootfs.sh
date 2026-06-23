#!/usr/bin/env bash
# Check a mounted Raspberry Pi 4 boot partition before installing reovim-os.

set -euo pipefail

usage() {
    cat <<'USAGE'
Usage: apps/os/targets/raspi4b-aarch64/check-bootfs.sh <boot-partition-dir>

Checks that the directory looks like a Raspberry Pi 4 FAT boot partition for
the #800 physical-board proof. This does not format, mount, or mutate media.
USAGE
}

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 2
}

warn() {
    printf 'warning: %s\n' "$*" >&2
}

if [ "$#" -ne 1 ]; then
    usage >&2
    exit 2
fi

BOOT_DIR_ARG="$1"
if [ ! -d "$BOOT_DIR_ARG" ]; then
    fail "boot partition directory does not exist: $BOOT_DIR_ARG"
fi

BOOT_DIR="$(cd "$BOOT_DIR_ARG" && pwd -P)"
if [ "$BOOT_DIR" = "/" ]; then
    fail "refusing to inspect /"
fi

if command -v mountpoint >/dev/null 2>&1 && ! mountpoint -q "$BOOT_DIR"; then
    warn "$BOOT_DIR is not reported as a mount point"
fi

for firmware in start4.elf fixup4.dat bcm2711-rpi-4-b.dtb; do
    path="$BOOT_DIR/$firmware"
    if [ ! -e "$path" ]; then
        fail "missing expected Raspberry Pi 4 firmware file: $firmware"
    fi
    if [ ! -s "$path" ]; then
        fail "expected Raspberry Pi 4 firmware file is empty: $firmware"
    fi
done

printf 'bootfs=ok\n'
printf 'path=%s\n' "$BOOT_DIR"
for firmware in start4.elf fixup4.dat bcm2711-rpi-4-b.dtb; do
    bytes="$(wc -c <"$BOOT_DIR/$firmware" | tr -d ' ')"
    printf 'firmware=%s bytes=%s\n' "$firmware" "$bytes"
done

kernel="$BOOT_DIR/kernel8.img"
if [ -s "$kernel" ]; then
    bytes="$(wc -c <"$kernel" | tr -d ' ')"
    sha256="$(sha256sum "$kernel" | awk '{ print $1 }')"
    printf 'existing_kernel8=present bytes=%s sha256=%s\n' "$bytes" "$sha256"
else
    printf 'existing_kernel8=absent\n'
fi
