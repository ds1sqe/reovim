#!/usr/bin/env bash
# Locate mounted Raspberry Pi 4 boot partitions for the #800 proof.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
CHECK_SCRIPT="$ROOT/apps/os/targets/raspi4b-aarch64/check-bootfs.sh"

usage() {
    cat <<'USAGE'
Usage: apps/os/targets/raspi4b-aarch64/find-bootfs.sh [DIR...]

With no DIR arguments, scans mounted FAT-like filesystems and prints paths that
look like Raspberry Pi 4 boot partitions. With DIR arguments, checks only those
paths. This helper is read-only; it does not format, mount, or write media.
USAGE
}

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 2
}

is_fat_like() {
    case "$1" in
        vfat | fat | msdos | exfat) return 0 ;;
        *) return 1 ;;
    esac
}

check_candidate() {
    local path="$1"
    local source="${2:-explicit}"
    local fstype="${3:-unknown}"
    local output

    if ! output="$("$CHECK_SCRIPT" "$path" 2>/dev/null)"; then
        return 1
    fi

    printf 'bootfs_candidate=%s\n' "$path"
    printf 'source=%s\n' "$source"
    printf 'fstype=%s\n' "$fstype"
    printf '%s\n' "$output" | sed 's/^/  /'
    return 0
}

scan_mounted_fat() {
    local found=1
    local target source fstype

    if ! command -v findmnt >/dev/null 2>&1; then
        fail "findmnt not found; pass candidate DIR paths explicitly"
    fi

    while read -r target source fstype _rest; do
        if [ -z "${target:-}" ] || [ -z "${source:-}" ] || [ -z "${fstype:-}" ]; then
            continue
        fi
        if ! is_fat_like "$fstype"; then
            continue
        fi
        if check_candidate "$target" "$source" "$fstype"; then
            found=0
        fi
    done < <(findmnt -rn -o TARGET,SOURCE,FSTYPE)

    if [ "$found" -ne 0 ]; then
        printf 'no mounted Raspberry Pi 4 bootfs candidates found\n' >&2
    fi

    return "$found"
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
    usage
    exit 0
fi

if [ "$#" -gt 0 ]; then
    found=1
    for candidate in "$@"; do
        if check_candidate "$candidate"; then
            found=0
        fi
    done
    if [ "$found" -ne 0 ]; then
        printf 'no Raspberry Pi 4 bootfs candidates found\n' >&2
    fi
    exit "$found"
fi

scan_mounted_fat
