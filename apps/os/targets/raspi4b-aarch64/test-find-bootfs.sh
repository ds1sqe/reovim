#!/usr/bin/env bash
# Smoke-test the Raspberry Pi 4 bootfs discovery helper.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
FIND_SCRIPT="$ROOT/apps/os/targets/raspi4b-aarch64/find-bootfs.sh"

tmp_root="$(mktemp -d)"
good="$tmp_root/pi-boot"
bad="$tmp_root/workstation-boot"
cleanup() {
    rm -rf "$tmp_root"
}
trap cleanup EXIT

mkdir "$good" "$bad"
printf 'firmware-start\n' >"$good/start4.elf"
printf 'firmware-fixup\n' >"$good/fixup4.dat"
printf 'firmware-dtb\n' >"$good/bcm2711-rpi-4-b.dtb"
printf 'not-pi\n' >"$bad/kernel.img"

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

output="$("$FIND_SCRIPT" "$bad" "$good")"
expect_contains "$output" "bootfs_candidate=$good" "good candidate"
expect_contains "$output" "bootfs=ok" "checker output"
expect_contains "$output" "firmware=start4.elf bytes=" "firmware row"

case "$output" in
    *"bootfs_candidate=$bad"*)
        printf 'error: discovery accepted non-Pi bootfs\n' >&2
        printf '%s\n' "$output" >&2
        exit 1
        ;;
esac

if "$FIND_SCRIPT" "$bad" >"$tmp_root/bad.out" 2>"$tmp_root/bad.err"; then
    printf 'error: discovery accepted only bad candidates\n' >&2
    cat "$tmp_root/bad.out" >&2
    exit 1
fi

printf 'bootfs discovery smoke ok\n'
