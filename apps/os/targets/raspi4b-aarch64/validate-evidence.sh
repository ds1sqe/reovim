#!/usr/bin/env bash
# Validate completed Raspberry Pi 4 physical USB keyboard evidence.

set -euo pipefail

usage() {
    printf 'Usage: apps/os/targets/raspi4b-aarch64/validate-evidence.sh <evidence-file>\n'
}

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 2
}

if [ "$#" -ne 1 ]; then
    usage >&2
    exit 2
fi

EVIDENCE="$1"
if [ ! -f "$EVIDENCE" ]; then
    fail "evidence file does not exist: $EVIDENCE"
fi
if [ ! -s "$EVIDENCE" ]; then
    fail "evidence file is empty: $EVIDENCE"
fi

missing=0

require_re() {
    local pattern="$1"
    local label="$2"

    if ! grep -Eq -- "$pattern" "$EVIDENCE"; then
        printf 'missing: %s\n' "$label" >&2
        missing=1
    fi
}

require_count_at_least() {
    local pattern="$1"
    local label="$2"
    local minimum="$3"
    local count

    count="$(grep -Ec -- "$pattern" "$EVIDENCE" || true)"
    if [ "$count" -lt "$minimum" ]; then
        printf 'missing: %s (found %s, need %s)\n' "$label" "$count" "$minimum" >&2
        missing=1
    fi
}

forbid_re() {
    local pattern="$1"
    local label="$2"

    if grep -Eq -- "$pattern" "$EVIDENCE"; then
        printf 'forbidden: %s\n' "$label" >&2
        missing=1
    fi
}

forbid_re '^  - \[[xX]\] display-only$' 'display-only evidence label is checked'
forbid_re '^  - \[[xX]\] UART input$' 'UART input evidence label is checked'
forbid_re '^  - \[[xX]\] bootline-script$' 'bootline-script evidence label is checked'
forbid_re '^- \[[xX]\] FAIL: display/UART/scripted evidence only\.$' 'FAIL result is checked'

require_re '^- Image bytes: [1-9][0-9]*$' 'seeded image byte count'
require_re '^- Image SHA-256: [0-9a-f]{64}$' 'seeded image SHA-256'
require_re '^- Preflight result: preflight=ok' 'preflight result passed'
require_re '^- \[[xX]\] HDMI display attached before boot\.$' 'HDMI display attached checkbox'
require_re '^- \[[xX]\] Physical USB keyboard attached before boot\.$' 'physical USB keyboard attached checkbox'
require_re '^- \[[xX]\] `REOVIM_OS_BOOTLINE` unset on booted image\.$' 'bootline unset setup checkbox'
require_re '^- \[[xX]\] `media_prepare=ok`$' 'media-prep status checkbox'
require_re '^- \[[xX]\] installed `kernel8\.img` SHA-256 matches image SHA-256\.$' 'installed image SHA checkbox'
require_re '^- \[[xX]\] installed `kernel8\.img` byte count matches image byte count\.$' 'installed image byte-count checkbox'
require_re '^  - \[[xX]\] physical USB keyboard$' 'physical USB keyboard evidence label'
require_re '^- \[[xX]\] PASS: physical USB keyboard proof complete\.$' 'physical USB keyboard PASS result'

require_re '^- \[[xX]\] `target=aarch64-unknown-none`$' 'target proof checkbox'
require_re '^- \[[xX]\] `bootline=absent`$' 'bootline-absent proof checkbox'
require_re '^- \[[xX]\] shell prompt reached: `reovim-os>`$' 'root prompt proof checkbox'
require_re '^- \[[xX]\] input source is recorded honestly$' 'honest input-source checkbox'
require_re '^- \[[xX]\] QEMU/VNC/HDMI display was not counted as keyboard input$' 'non-keyboard display evidence rejected'

require_re '^- \[[xX]\] A physical USB keypress reached the root shell\.$' 'physical keypress reached shell checkbox'
require_re '^- \[[xX]\] `input=usb-keyboard\+uart-fallback`$' 'USB keyboard input-source checkbox'
require_re '^- \[[xX]\] `usb_keyboard=ready`$' 'USB keyboard readiness checkbox'
require_re '^- \[[xX]\] `status` / `input` report live ready source diagnostics\.$' 'live source diagnostics checkbox'
require_re '^- \[[xX]\] `probe help` lists `usb-keyboard` and `xhci-read-keyboard-report`\.$' 'probe help catalog checkbox'
require_re '^- \[[xX]\] `dmesg` contains `shell: <command>` and `shell\.status=ok` audit lines for the typed commands\.$' 'dmesg command/status audit checkbox'
require_re '^- \[[xX]\] `dmesg` contains the `probe usb-keyboard` output or blocker\.$' 'probe-output dmesg checkbox'
require_re '^- \[[xX]\] `proof` prints the physical input proof checklist\.$' 'proof checklist checkbox'

require_re '^reovim-os>' 'root shell prompt in pasted transcript'
require_re '^media_prepare=ok$' 'media-prep result line'
require_re '^installed=.+/kernel8\.img$' 'installed kernel8.img path line'
require_re '^bytes=[1-9][0-9]*$' 'installed kernel8.img byte count line'
require_re '^sha256=[0-9a-f]{64}$' 'installed kernel8.img SHA-256 line'
require_re '^target=aarch64-unknown-none$' 'cat /boot/image target line'
require_re '^bootline=absent$' 'cat /boot/image bootline line'
require_re '^input=usb-keyboard\+uart-fallback$' 'live USB keyboard input line'
require_re '^source=usb-keyboard\+uart-fallback$' 'live USB keyboard source line'
require_re '^source_state=ready$' 'live source readiness line'
require_re '^mode=live$' 'input live-mode line'
require_re '^input_mode=live$' 'status/profile live-mode line'
require_re '^usb_keyboard=ready$' 'live USB keyboard readiness line'
require_re '^usb_keyboard_probe=enabled$' 'USB keyboard probe-enabled line'
require_re '^usb_keyboard_poll_interval_ms=[1-9][0-9]*$' 'nonzero USB keyboard poll interval line'
require_re '^reovim-os> proof$' 'proof command in pasted transcript'
require_re '^proof:$' 'proof checklist header'
require_re '^commands:$' 'proof command-list header'
require_re '^  cat /boot/image$' 'proof checklist image command'
require_re '^  probe usb-keyboard$' 'proof checklist USB keyboard command'
require_re '^  cat /log/dmesg$' 'proof checklist final log command'
require_re '^expected:$' 'proof expected-facts header'
require_re '^  source=usb-keyboard\+uart-fallback$' 'proof expected source fact'
require_re '^  usb_keyboard=ready$' 'proof expected readiness fact'
require_re '^  shell\.status=ok$' 'proof expected shell status fact'
require_re '^probe targets:$' 'probe help target catalog header'
require_re '^  usb-keyboard \(alias: keyboard\)$' 'probe help USB keyboard target'
require_re '^  xhci-read-keyboard-report \(alias: usb-keyboard-read-report\)$' 'probe help keyboard report target'
require_re '^probe usb-keyboard:$' 'probe usb-keyboard output header'
require_re '^state=(report-ready|decoded-pending|report-pending)$' 'USB keyboard probe ready/pending state'

require_re '^shell: cat /boot/image$' 'dmesg audit for cat /boot/image'
require_re '^shell: proof$' 'dmesg audit for proof'
require_re '^shell: status$' 'dmesg audit for status'
require_re '^shell: input$' 'dmesg audit for input'
require_re '^shell: probe help$' 'dmesg audit for probe help'
require_re '^shell: probe usb-keyboard$' 'dmesg audit for probe usb-keyboard'
require_re '^shell: cat /boot/profile$' 'dmesg audit for cat /boot/profile'
require_re '^shell: dmesg$' 'dmesg audit for dmesg'
require_re '^shell: cat /log/dmesg$' 'dmesg audit for cat /log/dmesg'
require_count_at_least '^shell\.status=ok$' 'successful shell status audit lines' 8

image_bytes="$(awk '/^- Image bytes: [1-9][0-9]*$/ { print $4; exit }' "$EVIDENCE")"
media_bytes="$(awk -F= '/^bytes=[1-9][0-9]*$/ { print $2; exit }' "$EVIDENCE")"
if [ -n "$image_bytes" ] && [ -n "$media_bytes" ] && [ "$image_bytes" != "$media_bytes" ]; then
    printf 'mismatch: image bytes %s != installed bytes %s\n' "$image_bytes" "$media_bytes" >&2
    missing=1
fi

image_sha="$(awk '/^- Image SHA-256: [0-9a-f]{64}$/ { print $4; exit }' "$EVIDENCE")"
media_sha="$(awk -F= '/^sha256=[0-9a-f]{64}$/ { print $2; exit }' "$EVIDENCE")"
if [ -n "$image_sha" ] && [ -n "$media_sha" ] && [ "$image_sha" != "$media_sha" ]; then
    printf 'mismatch: image SHA-256 %s != installed SHA-256 %s\n' "$image_sha" "$media_sha" >&2
    missing=1
fi

if [ "$missing" -ne 0 ]; then
    exit 1
fi

printf 'evidence=pass\n'
printf 'file=%s\n' "$EVIDENCE"
