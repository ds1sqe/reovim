#!/usr/bin/env bash
# Local preflight before preparing real Raspberry Pi 4 media.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET_DIR="$ROOT/apps/os/targets/raspi4b-aarch64"
TARGET="aarch64-unknown-none"
PACKAGE="reovim-os"
IMAGE="$ROOT/apps/target/$TARGET/debug/$PACKAGE.kernel8.img"
BUILD_SCRIPT="$TARGET_DIR/build-image.sh"
INSTALL_SCRIPT="$TARGET_DIR/install-image.sh"
INSTALL_SMOKE_SCRIPT="$TARGET_DIR/test-install-image.sh"
BOOTFS_CHECK_SCRIPT="$TARGET_DIR/check-bootfs.sh"
EVIDENCE_TEMPLATE="$TARGET_DIR/evidence-template.md"
DTB="$ROOT/arch/tests/fixtures/dtb/bcm2711-rpi-4-b.dtb"

BOOTFS=""
SKIP_QEMU=0
EVIDENCE_OUT=""

usage() {
    cat <<'USAGE'
Usage: apps/os/targets/raspi4b-aarch64/preflight-real-board.sh [options]

Options:
  --bootfs DIR   Also dry-run the installer against a mounted Pi boot partition.
  --evidence FILE
                 Write a prefilled evidence seed after preflight passes.
  --skip-qemu    Skip the local aarch64 QEMU smoke.
  -h, --help     Show this help text.

Runs the local checks that should pass before writing real Raspberry Pi 4 boot
media: no-bootline image build, installer smoke, optional installer dry-run,
and a QEMU display/UART smoke. QEMU output is not physical USB keyboard proof.
USAGE
}

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 2
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --bootfs)
            shift
            if [ "$#" -eq 0 ]; then
                fail "--bootfs needs a directory"
            fi
            BOOTFS="$1"
            ;;
        --evidence)
            shift
            if [ "$#" -eq 0 ]; then
                fail "--evidence needs an output file"
            fi
            EVIDENCE_OUT="$1"
            ;;
        --skip-qemu)
            SKIP_QEMU=1
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        -*)
            fail "unknown argument: $1"
            ;;
        *)
            fail "unexpected positional argument: $1"
            ;;
    esac
    shift
done

run_step() {
    printf '==> %s\n' "$*"
}

require_file() {
    local path="$1"
    if [ ! -e "$path" ]; then
        fail "missing required file: ${path#$ROOT/}"
    fi
}

write_evidence_seed() {
    local output="$1"
    local bytes_value="$2"
    local sha256_value="$3"
    local qemu_value="$4"
    local bootfs_value
    local date_utc
    local tmp_output
    local parent

    if [ -e "$output" ]; then
        fail "evidence output already exists: $output"
    fi
    parent="$(dirname "$output")"
    if [ ! -d "$parent" ]; then
        fail "evidence output directory does not exist: $parent"
    fi

    if [ -n "$BOOTFS" ]; then
        bootfs_value="$BOOTFS"
    else
        bootfs_value="<mounted-bootfs>"
    fi
    date_utc="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    tmp_output="$(mktemp)"

    {
        printf '# Raspberry Pi 4 USB Keyboard Evidence\n\n'
        printf 'Use this file for the #800 real-machine proof. The proof is incomplete\n'
        printf 'unless HDMI output is observed, input source is `physical USB keyboard`,\n'
        printf 'and the shell reports `usb_keyboard=ready`.\n\n'
        printf '## Session\n\n'
        printf -- '- Date: %s\n' "$date_utc"
        printf -- '- Operator:\n'
        printf -- '- Board: Raspberry Pi 4\n'
        printf -- '- Image path: %s\n' "${IMAGE#$ROOT/}"
        printf -- '- Image bytes: %s\n' "$bytes_value"
        printf -- '- Image SHA-256: %s\n' "$sha256_value"
        printf -- '- Image build command: %s\n' "${BUILD_SCRIPT#$ROOT/}"
        printf -- '- Image install command: %s --build %s\n' "${INSTALL_SCRIPT#$ROOT/}" "$bootfs_value"
        printf -- '- Boot partition path: %s\n' "$bootfs_value"
        printf -- '- Preflight command: %s\n' "${TARGET_DIR#$ROOT/}/preflight-real-board.sh"
        printf -- '- Preflight result: preflight=ok qemu_smoke=%s\n' "$qemu_value"
        printf -- '- Evidence label:\n'
        printf '  - [ ] display-only\n'
        printf '  - [ ] UART input\n'
        printf '  - [ ] bootline-script\n'
        printf '  - [ ] physical USB keyboard\n'
        printf -- '- [ ] HDMI display attached before boot.\n'
        printf -- '- [ ] Physical USB keyboard attached before boot.\n'
        printf -- '- [ ] `REOVIM_OS_BOOTLINE` unset on booted image.\n'
        printf -- '- UART serial console attached:\n'
        printf '\n'
        printf '## Boot Evidence\n\n'
        printf 'Paste the boot report or serial transcript excerpt:\n\n'
        printf '```text\n\n```\n\n'
        printf 'Required facts:\n\n'
        printf -- '- [ ] `target=aarch64-unknown-none`\n'
        printf -- '- [ ] `bootline=absent`\n'
        printf -- '- [ ] shell prompt reached: `reovim-os>`\n'
        printf -- '- [ ] input source is recorded honestly\n'
        printf -- '- [ ] QEMU/VNC/HDMI display was not counted as keyboard input\n\n'
        printf '## Command Transcript\n\n'
        printf 'Type these commands from the physical USB keyboard for final acceptance:\n\n'
        printf '```text\n'
        printf 'cat /boot/image\n'
        printf 'status\n'
        printf 'input\n'
        printf 'probe help\n'
        printf 'probe usb-keyboard\n'
        printf 'cat /boot/profile\n'
        printf 'dmesg\n'
        printf 'cat /log/dmesg\n'
        printf '```\n\n'
        printf 'Paste the resulting output:\n\n'
        printf '```text\n\n```\n\n'
        printf '## USB Keyboard Readiness\n\n'
        printf 'Required success facts:\n\n'
        printf -- '- [ ] A physical USB keypress reached the root shell.\n'
        printf -- '- [ ] `input=usb-keyboard+uart-fallback`\n'
        printf -- '- [ ] `usb_keyboard=ready`\n'
        printf -- '- [ ] `probe help` lists `usb-keyboard` and `xhci-read-keyboard-report`.\n'
        printf -- '- [ ] `dmesg` contains `shell: <command>` and `shell.status=ok` audit lines for the typed commands.\n'
        printf -- '- [ ] `dmesg` contains the `probe usb-keyboard` output or blocker.\n\n'
        printf 'If unsuccessful, record the exact blocker:\n\n'
        printf '```text\n\n```\n\n'
        printf '## Result\n\n'
        printf -- '- [ ] PASS: physical USB keyboard proof complete.\n'
        printf -- '- [ ] FAIL: display/UART/scripted evidence only.\n\n'
        printf 'Notes:\n\n'
        printf '```text\n\n```\n'
    } >"$tmp_output"

    mv "$tmp_output" "$output"
    printf 'evidence_seed=%s\n' "$output"
}

cd "$ROOT"

require_file "$BUILD_SCRIPT"
require_file "$INSTALL_SCRIPT"
require_file "$INSTALL_SMOKE_SCRIPT"
require_file "$BOOTFS_CHECK_SCRIPT"
require_file "$EVIDENCE_TEMPLATE"
require_file "$DTB"

run_step "syntax check target helper scripts"
for script in "$BUILD_SCRIPT" "$INSTALL_SCRIPT" "$INSTALL_SMOKE_SCRIPT" "$BOOTFS_CHECK_SCRIPT"; do
    bash -n "$script"
done

run_step "build no-bootline Raspberry Pi 4 image"
"$BUILD_SCRIPT"

run_step "smoke-test boot-partition installer without real media"
"$INSTALL_SMOKE_SCRIPT"

if [ -n "$BOOTFS" ]; then
    run_step "check mounted Raspberry Pi 4 boot partition"
    "$BOOTFS_CHECK_SCRIPT" "$BOOTFS"

    run_step "dry-run installer against mounted boot partition"
    "$INSTALL_SCRIPT" --dry-run "$BOOTFS"
fi

qemu_status="skipped"
qemu_log=""
if [ "$SKIP_QEMU" -eq 0 ]; then
    if ! command -v qemu-system-aarch64 >/dev/null 2>&1; then
        fail "qemu-system-aarch64 not found; install it or pass --skip-qemu"
    fi
    if ! command -v timeout >/dev/null 2>&1; then
        fail "timeout not found; cannot bound QEMU smoke"
    fi

    run_step "boot QEMU raspi4b display/UART smoke"
    qemu_log="$(mktemp)"
    set +e
    timeout 8s qemu-system-aarch64 \
        -M raspi4b \
        -m 2048 \
        -display none \
        -serial stdio \
        -semihosting \
        -dtb "$DTB" \
        -kernel "$IMAGE" \
        >"$qemu_log" 2>&1
    status=$?
    set -e

    if [ "$status" -ne 0 ] && [ "$status" -ne 124 ]; then
        cat "$qemu_log" >&2
        rm -f "$qemu_log"
        fail "QEMU smoke failed with exit status $status"
    fi
    if ! grep -q 'reovim-os>' "$qemu_log"; then
        cat "$qemu_log" >&2
        rm -f "$qemu_log"
        fail "QEMU smoke did not reach root shell prompt"
    fi
    if ! grep -q 'input=pl011-uart' "$qemu_log"; then
        cat "$qemu_log" >&2
        rm -f "$qemu_log"
        fail "QEMU smoke did not report PL011 UART input"
    fi
    qemu_status="passed"
    rm -f "$qemu_log"
fi

bytes="$(wc -c <"$IMAGE" | tr -d ' ')"
sha256="$(sha256sum "$IMAGE" | awk '{ print $1 }')"
if [ -n "$BOOTFS" ]; then
    install_bootfs="$BOOTFS"
else
    install_bootfs="<mounted-bootfs>"
fi

printf 'preflight=ok\n'
printf 'image=%s\n' "${IMAGE#$ROOT/}"
printf 'bytes=%s\n' "$bytes"
printf 'sha256=%s\n' "$sha256"
printf 'qemu_smoke=%s\n' "$qemu_status"
printf 'install_command=%s --build %s\n' "${INSTALL_SCRIPT#$ROOT/}" "$install_bootfs"
printf 'evidence_template=%s\n' "${EVIDENCE_TEMPLATE#$ROOT/}"

if [ -n "$EVIDENCE_OUT" ]; then
    write_evidence_seed "$EVIDENCE_OUT" "$bytes" "$sha256" "$qemu_status"
fi
