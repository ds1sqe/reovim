#!/usr/bin/env bash
# Install the Raspberry Pi 4 RTOS image onto a mounted boot partition.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET="aarch64-unknown-none"
PACKAGE="reovim-os"
IMAGE="$ROOT/apps/target/$TARGET/debug/$PACKAGE.kernel8.img"
BUILD_SCRIPT="$ROOT/apps/os/targets/raspi4b-aarch64/build-image.sh"

BUILD=0
BACKUP=1
DRY_RUN=0
BOOT_DIR_ARG=""

usage() {
    cat <<'USAGE'
Usage: apps/os/targets/raspi4b-aarch64/install-image.sh [options] <boot-partition-dir>

Options:
  --build       Build the no-bootline image before installing it.
  --no-backup   Replace kernel8.img without first saving a timestamped backup.
  --dry-run     Print the planned copy operation without changing files.
  -h, --help    Show this help text.

The argument must be the mounted Raspberry Pi 4 FAT boot partition directory.
This script only installs kernel8.img; it does not format media, mount media,
or install Raspberry Pi firmware files.
USAGE
}

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 2
}

warn() {
    printf 'warning: %s\n' "$*" >&2
}

backup_path_for() {
    local dest="$1"
    local base
    local candidate
    local index

    base="$dest.bak-$(date -u +%Y%m%dT%H%M%SZ)"
    candidate="$base"
    index=1
    while [ -e "$candidate" ]; do
        candidate="$base.$index"
        index=$((index + 1))
    done
    printf '%s\n' "$candidate"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --build)
            BUILD=1
            ;;
        --no-backup)
            BACKUP=0
            ;;
        --dry-run)
            DRY_RUN=1
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        -*)
            fail "unknown argument: $1"
            ;;
        *)
            if [ -n "$BOOT_DIR_ARG" ]; then
                fail "expected one boot partition directory"
            fi
            BOOT_DIR_ARG="$1"
            ;;
    esac
    shift
done

if [ -z "$BOOT_DIR_ARG" ]; then
    usage >&2
    exit 2
fi

if [ "$BUILD" -eq 1 ]; then
    "$BUILD_SCRIPT"
fi

if [ ! -s "$IMAGE" ]; then
    fail "missing image: ${IMAGE#$ROOT/}; run build-image.sh or pass --build"
fi

if [ ! -d "$BOOT_DIR_ARG" ]; then
    fail "boot partition directory does not exist: $BOOT_DIR_ARG"
fi

BOOT_DIR="$(cd "$BOOT_DIR_ARG" && pwd -P)"
if [ "$BOOT_DIR" = "/" ]; then
    fail "refusing to install to /"
fi

if command -v mountpoint >/dev/null 2>&1 && ! mountpoint -q "$BOOT_DIR"; then
    warn "$BOOT_DIR is not reported as a mount point"
fi

for firmware in start4.elf fixup4.dat bcm2711-rpi-4-b.dtb; do
    if [ ! -e "$BOOT_DIR/$firmware" ]; then
        warn "missing expected Raspberry Pi 4 firmware file: $firmware"
    fi
done

DEST="$BOOT_DIR/kernel8.img"
BACKUP_PATH=""
if [ -e "$DEST" ] && [ "$BACKUP" -eq 1 ]; then
    BACKUP_PATH="$(backup_path_for "$DEST")"
fi

bytes="$(wc -c <"$IMAGE" | tr -d ' ')"
sha256="$(sha256sum "$IMAGE" | awk '{ print $1 }')"

if [ "$DRY_RUN" -eq 1 ]; then
    printf 'source=%s\n' "${IMAGE#$ROOT/}"
    printf 'destination=%s\n' "$DEST"
    printf 'bytes=%s\n' "$bytes"
    printf 'sha256=%s\n' "$sha256"
    if [ -n "$BACKUP_PATH" ]; then
        printf 'backup=%s\n' "$BACKUP_PATH"
    fi
    printf 'dry_run=true\n'
    exit 0
fi

tmp_dest=""
cleanup() {
    if [ -n "$tmp_dest" ] && [ -e "$tmp_dest" ]; then
        rm -f "$tmp_dest"
    fi
}
trap cleanup EXIT

if [ -n "$BACKUP_PATH" ]; then
    cp "$DEST" "$BACKUP_PATH"
fi

tmp_dest="$DEST.tmp.$$"
cp "$IMAGE" "$tmp_dest"
mv "$tmp_dest" "$DEST"
sync "$DEST" 2>/dev/null || sync

printf 'installed=%s\n' "$DEST"
printf 'source=%s\n' "${IMAGE#$ROOT/}"
printf 'bytes=%s\n' "$bytes"
printf 'sha256=%s\n' "$sha256"
if [ -n "$BACKUP_PATH" ]; then
    printf 'backup=%s\n' "$BACKUP_PATH"
fi
