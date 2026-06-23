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
expect_contains "$evidence" 'proof' "evidence proof command"
expect_contains "$evidence" 'cat /boot/proof' "evidence VFS proof command"
expect_contains "$evidence" 'help' "evidence help command"
expect_contains "$evidence" 'clear' "evidence clear command"
expect_contains "$evidence" 'screentest' "evidence screentest command"
expect_contains "$evidence" 'pwd' "evidence pwd command"
expect_contains "$evidence" 'ls /boot' "evidence boot ls command"
expect_contains "$evidence" 'ls /dev' "evidence dev ls command"
expect_contains "$evidence" 'cat /boot/mounts' "evidence VFS mounts command"
expect_contains "$evidence" $'device\ncat /boot/memory' "evidence device command"
expect_contains "$evidence" 'cat /boot/memory' "evidence VFS memory command"
expect_contains "$evidence" 'cat /boot/devices' "evidence VFS devices command"
expect_contains "$evidence" 'cd /dev' "evidence cd dev command"
expect_contains "$evidence" 'cat uart0' "evidence relative UART device command"
expect_contains "$evidence" 'cd /' "evidence cd root command"
expect_contains "$evidence" 'cat /boot/status' "evidence VFS status command"
expect_contains "$evidence" 'cat /boot/input' "evidence VFS input command"
expect_contains "$evidence" 'cat /boot/image' "evidence command transcript"
expect_contains "$evidence" 'cat /log/dmesg' "evidence kernel log transcript command"
expect_contains "$evidence" 'probe pcie' "evidence PCIe probe command"
expect_contains "$evidence" '`status` / `input` report live ready source diagnostics.' "evidence live source fact"
expect_contains "$evidence" '`cat /boot/status` / `cat /boot/input` report VFS-backed live diagnostics.' "evidence VFS live source fact"
expect_contains "$evidence" '`probe help` lists `pcie`, `usb-keyboard`, and `xhci-read-keyboard-report`.' "evidence probe help fact"
expect_contains "$evidence" '`probe pcie` reports the read-only PCIe/xHCI state.' "evidence PCIe probe fact"
expect_contains "$evidence" '`proof` prints the physical input proof checklist.' "evidence proof fact"
expect_contains "$evidence" '`cat /boot/proof` prints the VFS-backed proof pseudo-file.' "evidence VFS proof fact"
expect_contains "$evidence" '`help` / `clear` / `screentest` prove the shell help and renderer commands.' "evidence shell usability fact"
expect_contains "$evidence" '`pwd` / `ls` / `mount` report the kernel VFS namespace and mounts.' "evidence VFS namespace fact"
expect_contains "$evidence" '`device` / `cat /boot/memory` / `cat /boot/devices` report boot inventory.' "evidence boot inventory fact"
expect_contains "$evidence" '`cd /dev` / `cat uart0` prove relative VFS device-file access.' "evidence relative device fact"
expect_contains "$evidence" '`shell.status=ok` audit lines' "evidence status audit fact"
expect_contains "$evidence" '`usb_keyboard=ready`' "evidence readiness fact"

printf 'preflight evidence smoke ok\n'
printf 'sha256=%s\n' "$source_sha"
