#!/usr/bin/env bash
# Smoke-test the Raspberry Pi 4 real-board preflight evidence path.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET="aarch64-unknown-none"
PACKAGE="reovim-os"
IMAGE="$ROOT/apps/target/$TARGET/debug/$PACKAGE.kernel8.img"
PREFLIGHT_SCRIPT="$ROOT/apps/os/targets/raspi4b-aarch64/preflight-real-board.sh"

tmp_bootfs="$(mktemp -d)"
tmp_evidence="$(mktemp -u)"
cleanup() {
    rm -rf "$tmp_bootfs"
    rm -f "$tmp_evidence"
}
trap cleanup EXIT

printf 'firmware-start\n' >"$tmp_bootfs/start4.elf"
printf 'firmware-fixup\n' >"$tmp_bootfs/fixup4.dat"
printf 'firmware-dtb\n' >"$tmp_bootfs/bcm2711-rpi-4-b.dtb"

preflight_output="$("$PREFLIGHT_SCRIPT" --skip-qemu --bootfs "$tmp_bootfs" --evidence "$tmp_evidence")"

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

source_sha="$(sha256sum "$IMAGE" | awk '{ print $1 }')"

expect_contains "$preflight_output" "preflight=ok" "preflight status"
expect_contains "$preflight_output" "bootfs=ok" "bootfs check status"
expect_contains "$preflight_output" "qemu_smoke=skipped" "skipped qemu status"
expect_contains "$preflight_output" "sha256=$source_sha" "image sha"
expect_contains "$preflight_output" "evidence_seed=$tmp_evidence" "evidence path"

if [ ! -s "$tmp_evidence" ]; then
    printf 'error: evidence seed was not written: %s\n' "$tmp_evidence" >&2
    exit 1
fi

evidence="$(cat "$tmp_evidence")"
expect_contains "$evidence" "- Image path: apps/target/aarch64-unknown-none/debug/reovim-os.kernel8.img" "evidence image path"
expect_contains "$evidence" "- Image SHA-256: $source_sha" "evidence sha"
expect_contains "$evidence" "- Image install command: apps/os/targets/raspi4b-aarch64/install-image.sh --build $tmp_bootfs" "evidence install command"
expect_contains "$evidence" "- Boot partition path: $tmp_bootfs" "evidence bootfs"
expect_contains "$evidence" "- Preflight result: preflight=ok qemu_smoke=skipped" "evidence preflight status"
expect_contains "$evidence" "- [ ] HDMI display attached before boot." "evidence HDMI setup checkbox"
expect_contains "$evidence" "- [ ] Physical USB keyboard attached before boot." "evidence physical keyboard setup checkbox"
expect_contains "$evidence" '- [ ] `REOVIM_OS_BOOTLINE` unset on booted image.' "evidence bootline setup checkbox"
expect_contains "$evidence" "- [ ] physical USB keyboard" "evidence physical keyboard checkbox"
expect_contains "$evidence" 'cat /boot/image' "evidence command transcript"
expect_contains "$evidence" '`usb_keyboard=ready`' "evidence readiness fact"

printf 'preflight evidence smoke ok\n'
printf 'sha256=%s\n' "$source_sha"
