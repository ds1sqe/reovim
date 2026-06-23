#!/usr/bin/env bash
# Smoke-test the Raspberry Pi 4 physical evidence validator.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TARGET_DIR="$ROOT/apps/os/targets/raspi4b-aarch64"
VALIDATOR="$TARGET_DIR/validate-evidence.sh"
TEMPLATE="$TARGET_DIR/evidence-template.md"

tmp_dir="$(mktemp -d)"
cleanup() {
    rm -rf "$tmp_dir"
}
trap cleanup EXIT

pass_evidence="$tmp_dir/pass.md"
display_only_evidence="$tmp_dir/display-only.md"
missing_setup_evidence="$tmp_dir/missing-setup.md"
missing_audit_evidence="$tmp_dir/missing-audit.md"
missing_media_evidence="$tmp_dir/missing-media.md"
media_byte_mismatch_evidence="$tmp_dir/media-byte-mismatch.md"
media_sha_mismatch_evidence="$tmp_dir/media-sha-mismatch.md"
validator_output="$tmp_dir/validator.out"

write_passing_evidence() {
    local output="$1"

    {
        printf '# Raspberry Pi 4 USB Keyboard Evidence\n\n'
        printf '## Session\n\n'
        printf -- '- Image bytes: 433788\n'
        printf -- '- Image SHA-256: abeccca617486102d57d9e25b93f8c59c463003f2f7584b69a3faae5d6ba15b2\n'
        printf -- '- Preflight result: preflight=ok qemu_smoke=passed\n'
        printf -- '- [x] HDMI display attached before boot.\n'
        printf -- '- [x] Physical USB keyboard attached before boot.\n'
        printf -- '- [x] `REOVIM_OS_BOOTLINE` unset on booted image.\n'
        printf -- '- Evidence label:\n'
        printf '  - [ ] display-only\n'
        printf '  - [ ] UART input\n'
        printf '  - [ ] bootline-script\n'
        printf '  - [x] physical USB keyboard\n\n'
        printf '## Media Preparation\n\n'
        printf 'Required media facts:\n\n'
        printf -- '- [x] `media_prepare=ok`\n'
        printf -- '- [x] installed `kernel8.img` SHA-256 matches image SHA-256.\n'
        printf -- '- [x] installed `kernel8.img` byte count matches image byte count.\n\n'
        printf '```text\n'
        printf 'media_prepare=ok\n'
        printf 'bootfs=/media/pi-boot\n'
        printf 'installed=/media/pi-boot/kernel8.img\n'
        printf 'bytes=433788\n'
        printf 'sha256=abeccca617486102d57d9e25b93f8c59c463003f2f7584b69a3faae5d6ba15b2\n'
        printf '```\n\n'
        printf '## Boot Evidence\n\n'
        printf '```text\n'
        printf 'reovim-os> cat /boot/image\n'
        printf 'package=reovim-os\n'
        printf 'target=aarch64-unknown-none\n'
        printf 'bootline=absent\n'
        printf '```\n\n'
        printf 'Required facts:\n\n'
        printf -- '- [x] `target=aarch64-unknown-none`\n'
        printf -- '- [x] `bootline=absent`\n'
        printf -- '- [x] shell prompt reached: `reovim-os>`\n'
        printf -- '- [x] input source is recorded honestly\n'
        printf -- '- [x] QEMU/VNC/HDMI display was not counted as keyboard input\n\n'
        printf '## Command Transcript\n\n'
        printf '```text\n'
        printf 'reovim-os> cat /boot/image\n'
        printf 'target=aarch64-unknown-none\n'
        printf 'bootline=absent\n'
        printf 'reovim-os> status\n'
        printf 'input=usb-keyboard+uart-fallback\n'
        printf 'usb_keyboard=ready\n'
        printf 'reovim-os> input\n'
        printf 'source=usb-keyboard+uart-fallback\n'
        printf 'usb_keyboard=ready\n'
        printf 'reovim-os> probe help\n'
        printf 'probe targets:\n'
        printf '  usb-keyboard (alias: keyboard)\n'
        printf 'reovim-os> probe usb-keyboard\n'
        printf 'probe usb-keyboard:\n'
        printf 'state=report-pending\n'
        printf 'slot_id=1\n'
        printf 'endpoint_id=3\n'
        printf 'reovim-os> cat /boot/profile\n'
        printf 'input=usb-keyboard+uart-fallback\n'
        printf 'usb_keyboard=ready\n'
        printf 'reovim-os> dmesg\n'
        printf 'dmesg:\n'
        printf 'shell: cat /boot/image\n'
        printf 'shell: status\n'
        printf 'shell: input\n'
        printf 'shell: probe help\n'
        printf 'shell: probe usb-keyboard\n'
        printf 'shell: cat /boot/profile\n'
        printf 'shell: dmesg\n'
        printf '```\n\n'
        printf '## USB Keyboard Readiness\n\n'
        printf 'Required success facts:\n\n'
        printf -- '- [x] A physical USB keypress reached the root shell.\n'
        printf -- '- [x] `input=usb-keyboard+uart-fallback`\n'
        printf -- '- [x] `usb_keyboard=ready`\n'
        printf -- '- [x] `dmesg` contains `shell: <command>` audit lines for the typed commands.\n'
        printf -- '- [x] `dmesg` contains the `probe usb-keyboard` output or blocker.\n\n'
        printf '## Result\n\n'
        printf -- '- [x] PASS: physical USB keyboard proof complete.\n'
        printf -- '- [ ] FAIL: display/UART/scripted evidence only.\n'
    } >"$output"
}

expect_failure() {
    local file="$1"
    local label="$2"

    if "$VALIDATOR" "$file" >"$tmp_dir/$label.out" 2>"$tmp_dir/$label.err"; then
        printf 'error: validator accepted %s evidence\n' "$label" >&2
        cat "$tmp_dir/$label.out" >&2
        exit 1
    fi
}

write_passing_evidence "$pass_evidence"
"$VALIDATOR" "$pass_evidence" >"$validator_output"
grep -q '^evidence=pass$' "$validator_output"
grep -q "^file=$pass_evidence$" "$validator_output"

expect_failure "$TEMPLATE" "template"

cp "$pass_evidence" "$display_only_evidence"
sed -i 's/^  - \[ \] display-only$/  - [x] display-only/' "$display_only_evidence"
expect_failure "$display_only_evidence" "display-only"

cp "$pass_evidence" "$missing_setup_evidence"
sed -i '/^- \[x\] HDMI display attached before boot\.$/d' "$missing_setup_evidence"
expect_failure "$missing_setup_evidence" "missing-setup"

cp "$pass_evidence" "$missing_audit_evidence"
sed -i '/^shell: dmesg$/d' "$missing_audit_evidence"
expect_failure "$missing_audit_evidence" "missing-audit"

cp "$pass_evidence" "$missing_media_evidence"
sed -i '/^media_prepare=ok$/d' "$missing_media_evidence"
expect_failure "$missing_media_evidence" "missing-media"

cp "$pass_evidence" "$media_byte_mismatch_evidence"
sed -i 's/^- Image bytes: 433788$/- Image bytes: 433787/' "$media_byte_mismatch_evidence"
expect_failure "$media_byte_mismatch_evidence" "media-byte-mismatch"

cp "$pass_evidence" "$media_sha_mismatch_evidence"
sed -i 's/^sha256=abeccca617486102d57d9e25b93f8c59c463003f2f7584b69a3faae5d6ba15b2$/sha256=0000000000000000000000000000000000000000000000000000000000000000/' "$media_sha_mismatch_evidence"
expect_failure "$media_sha_mismatch_evidence" "media-sha-mismatch"

printf 'evidence validator smoke ok\n'
