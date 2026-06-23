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
expect_contains "$evidence" '- [ ] `package=reovim-os`' "evidence package fact"
expect_contains "$evidence" '- [ ] `/boot/image` reports a `version=` line.' "evidence version fact"
expect_contains "$evidence" '- [ ] `selected_profile=shell-only`' "evidence selected profile fact"
expect_contains "$evidence" '- [ ] `profile_request=shell-only`' "evidence profile request fact"
expect_contains "$evidence" '- [ ] `launch_profile_feature=disabled`' "evidence launch-profile feature fact"
expect_contains "$evidence" 'proof' "evidence proof command"
expect_contains "$evidence" 'cat /boot/proof' "evidence VFS proof command"
expect_contains "$evidence" 'help' "evidence help command"
expect_contains "$evidence" 'help clear' "evidence targeted clear help command"
expect_contains "$evidence" 'help screentest' "evidence targeted screentest help command"
expect_contains "$evidence" 'help input' "evidence targeted input help command"
expect_contains "$evidence" 'help proof' "evidence targeted proof help command"
expect_contains "$evidence" 'help pwd' "evidence targeted pwd help command"
expect_contains "$evidence" 'help ls' "evidence targeted ls help command"
expect_contains "$evidence" 'help cd' "evidence targeted cd help command"
expect_contains "$evidence" 'help cat' "evidence targeted cat help command"
expect_contains "$evidence" 'help mount' "evidence targeted mount help command"
expect_contains "$evidence" 'help device' "evidence targeted device help command"
expect_contains "$evidence" 'help dmesg' "evidence targeted dmesg help command"
expect_contains "$evidence" 'help status' "evidence targeted status help command"
expect_contains "$evidence" 'help probe' "evidence targeted probe help command"
expect_contains "$evidence" 'help launch' "evidence targeted launch help command"
expect_contains "$evidence" 'help reovim' "evidence targeted reovim help command"
expect_contains "$evidence" 'help halt' "evidence targeted halt help command"
expect_contains "$evidence" 'cat /boot/help' "evidence VFS help command"
expect_contains "$evidence" 'clear' "evidence clear command"
expect_contains "$evidence" 'screentest' "evidence screentest command"
expect_contains "$evidence" 'pwd' "evidence pwd command"
expect_contains "$evidence" 'ls /boot' "evidence boot ls command"
expect_contains "$evidence" 'ls /dev' "evidence dev ls command"
expect_contains "$evidence" 'ls /log' "evidence log ls command"
expect_contains "$evidence" 'cat /boot/mounts' "evidence VFS mounts command"
expect_contains "$evidence" $'device\ncat /boot/memory' "evidence device command"
expect_contains "$evidence" 'cat /boot/memory' "evidence VFS memory command"
expect_contains "$evidence" 'cat /boot/devices' "evidence VFS devices command"
expect_contains "$evidence" 'cd /dev' "evidence cd dev command"
expect_contains "$evidence" $'cd /dev\npwd\nls\ncat uart0' "evidence relative ls command"
expect_contains "$evidence" 'cat uart0' "evidence relative UART device command"
expect_contains "$evidence" 'cd /' "evidence cd root command"
expect_contains "$evidence" 'cat /boot/status' "evidence VFS status command"
expect_contains "$evidence" 'cat /boot/input' "evidence VFS input command"
expect_contains "$evidence" 'cat /boot/probes' "evidence VFS probe catalog command"
expect_contains "$evidence" 'cat /boot/image' "evidence command transcript"
expect_contains "$evidence" 'launch' "evidence launch command"
expect_contains "$evidence" 'reovim' "evidence reovim command"
expect_contains "$evidence" 'dmesg --stats' "evidence dmesg stats command"
expect_contains "$evidence" 'cat /log/stats' "evidence VFS log stats command"
expect_contains "$evidence" 'cat /log/dmesg' "evidence kernel log transcript command"
expect_contains "$evidence" '## Shutdown Check' "evidence shutdown section"
expect_contains "$evidence" $'halt\n```' "evidence halt command"
expect_contains "$evidence" 'probe pcie' "evidence PCIe probe command"
expect_contains "$evidence" '`usb_keyboard_last_poll=report-ready`' "evidence USB report-ready fact"
expect_contains "$evidence" '`dmesg` contains `input.usb_keyboard=ready source=usb-keyboard+uart-fallback last_poll=report-ready`.' "evidence USB readiness dmesg fact"
expect_contains "$evidence" '`manual_next=type-shell-command`' "evidence manual next fact"
expect_contains "$evidence" '`status` / `cat /boot/status` report image/profile identity and live ready source diagnostics.' "evidence status identity/live source fact"
expect_contains "$evidence" '`input` / `cat /boot/input` report live input diagnostics.' "evidence input live diagnostics fact"
expect_contains "$evidence" '`cat /boot/probes` prints the VFS-backed probe catalog.' "evidence VFS probe catalog fact"
expect_contains "$evidence" '`probe help` lists `pcie`, `usb-keyboard`, and `xhci-read-keyboard-report`.' "evidence probe help fact"
expect_contains "$evidence" '`probe pcie` reports the read-only PCIe/xHCI state.' "evidence PCIe probe fact"
expect_contains "$evidence" '`cat /boot/profile` reports `profile=shell-only`, `launch=disabled`, `payloads=0`, and `input_mode=live`.' "evidence profile identity fact"
expect_contains "$evidence" '`launch` / `reovim` report shell-only payload launch disabled.' "evidence shell-only launch-disabled fact"
expect_contains "$evidence" '`proof` prints the physical input proof checklist.' "evidence proof fact"
expect_contains "$evidence" '`cat /boot/proof` prints the VFS-backed proof pseudo-file.' "evidence VFS proof fact"
expect_contains "$evidence" '`cat /boot/help` prints the VFS-backed detailed help catalog.' "evidence VFS help fact"
expect_contains "$evidence" '`help dmesg` / `ls /log` make kernel log stats discoverable.' "evidence log stats discovery fact"
expect_contains "$evidence" '`help input` / `help proof` make live input diagnostics and the proof checklist self-describing.' "evidence input/proof targeted help fact"
expect_contains "$evidence" '`help status` / `help probe` make `manual_next` and probe catalog discovery self-describing.' "evidence targeted help fact"
expect_contains "$evidence" '`help pwd` / `help ls` / `help cd` / `help cat` / `help mount` make VFS navigation self-describing.' "evidence VFS targeted help fact"
expect_contains "$evidence" '`help` / `help clear` / `help screentest` / `clear` / `screentest` prove shell help and erase-line mode diagnostics.' "evidence shell usability fact"
expect_contains "$evidence" '`help device` / `help launch` / `help reovim` / `help halt` make boot inventory, payload, and shutdown commands self-describing.' "evidence remaining command help fact"
expect_contains "$evidence" '`pwd` / `ls` / `mount` report the kernel VFS namespace and mounts.' "evidence VFS namespace fact"
expect_contains "$evidence" '`device` / `cat /boot/memory` / `cat /boot/devices` report boot inventory.' "evidence boot inventory fact"
expect_contains "$evidence" '`cd /dev` / `ls` / `cat uart0` prove relative VFS device-file access.' "evidence relative device fact"
expect_contains "$evidence" '`dmesg --stats` / `cat /log/stats` report kernel log ring stats.' "evidence log stats fact"
expect_contains "$evidence" '`shell.status=ok` audit lines' "evidence status audit fact"
expect_contains "$evidence" '`shell.status=error` audit lines for the disabled payload launch commands.' "evidence disabled status audit fact"
expect_contains "$evidence" '`halt` was typed last and printed `halt: ok`.' "evidence halt fact"
expect_contains "$evidence" 'No new `reovim-os>` prompt appeared after `halt: ok`.' "evidence halt no-prompt fact"
expect_contains "$evidence" '`usb_keyboard=ready`' "evidence readiness fact"

printf 'preflight evidence smoke ok\n'
printf 'sha256=%s\n' "$source_sha"
