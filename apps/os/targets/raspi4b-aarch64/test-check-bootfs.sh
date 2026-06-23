#!/usr/bin/env bash
# Smoke-test the Raspberry Pi 4 boot-partition readiness checker.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
CHECK_SCRIPT="$ROOT/apps/os/targets/raspi4b-aarch64/check-bootfs.sh"

tmp_bootfs="$(mktemp -d)"
cleanup() {
    rm -rf "$tmp_bootfs"
}
trap cleanup EXIT

write_firmware() {
    local dir="$1"

    printf 'firmware-start\n' >"$dir/start4.elf"
    printf 'firmware-fixup\n' >"$dir/fixup4.dat"
    printf 'firmware-dtb\n' >"$dir/bcm2711-rpi-4-b.dtb"
}

expect_contains() {
    local haystack="$1"
    local needle="$2"
    local label="$3"

    case "$haystack" in
        *"$needle"*) ;;
        *)
            printf 'error: missing %s: %s\n' "$label" "$needle" >&2
            printf '%s\n' "$haystack" >&2
            exit 1
            ;;
    esac
}

expect_failure() {
    local dir="$1"
    local label="$2"

    if "$CHECK_SCRIPT" "$dir" >"$tmp_bootfs/$label.out" 2>"$tmp_bootfs/$label.err"; then
        printf 'error: checker accepted %s bootfs\n' "$label" >&2
        cat "$tmp_bootfs/$label.out" >&2
        exit 1
    fi
}

write_firmware "$tmp_bootfs"
output="$("$CHECK_SCRIPT" "$tmp_bootfs")"
expect_contains "$output" "bootfs=ok" "bootfs ok status"
expect_contains "$output" "firmware=start4.elf bytes=" "start4.elf row"
expect_contains "$output" "firmware=fixup4.dat bytes=" "fixup4.dat row"
expect_contains "$output" "firmware=bcm2711-rpi-4-b.dtb bytes=" "dtb row"
expect_contains "$output" "existing_kernel8=absent" "absent kernel report"

printf 'old-kernel\n' >"$tmp_bootfs/kernel8.img"
output="$("$CHECK_SCRIPT" "$tmp_bootfs")"
expect_contains "$output" "existing_kernel8=present bytes=11 sha256=" "existing kernel report"

missing_bootfs="$tmp_bootfs/missing"
mkdir "$missing_bootfs"
printf 'firmware-start\n' >"$missing_bootfs/start4.elf"
printf 'firmware-dtb\n' >"$missing_bootfs/bcm2711-rpi-4-b.dtb"
expect_failure "$missing_bootfs" "missing-firmware"

empty_bootfs="$tmp_bootfs/empty"
mkdir "$empty_bootfs"
write_firmware "$empty_bootfs"
: >"$empty_bootfs/fixup4.dat"
expect_failure "$empty_bootfs" "empty-firmware"

printf 'bootfs checker smoke ok\n'
