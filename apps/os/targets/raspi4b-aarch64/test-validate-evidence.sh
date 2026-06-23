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
missing_status_evidence="$tmp_dir/missing-status.md"
missing_probe_help_evidence="$tmp_dir/missing-probe-help.md"
missing_targeted_screentest_help_evidence="$tmp_dir/missing-targeted-screentest-help.md"
missing_targeted_input_help_evidence="$tmp_dir/missing-targeted-input-help.md"
missing_targeted_proof_help_evidence="$tmp_dir/missing-targeted-proof-help.md"
missing_targeted_vfs_help_evidence="$tmp_dir/missing-targeted-vfs-help.md"
missing_targeted_help_evidence="$tmp_dir/missing-targeted-help.md"
missing_live_source_evidence="$tmp_dir/missing-live-source.md"
missing_last_poll_evidence="$tmp_dir/missing-last-poll.md"
missing_manual_next_evidence="$tmp_dir/missing-manual-next.md"
missing_vfs_live_source_evidence="$tmp_dir/missing-vfs-live-source.md"
missing_image_identity_evidence="$tmp_dir/missing-image-identity.md"
missing_profile_identity_evidence="$tmp_dir/missing-profile-identity.md"
missing_identity_checkbox_evidence="$tmp_dir/missing-identity-checkbox.md"
missing_vfs_namespace_evidence="$tmp_dir/missing-vfs-namespace.md"
missing_boot_inventory_evidence="$tmp_dir/missing-boot-inventory.md"
missing_shell_usability_evidence="$tmp_dir/missing-shell-usability.md"
missing_typed_command_evidence="$tmp_dir/missing-typed-command.md"
missing_profile_prompt_evidence="$tmp_dir/missing-profile-prompt.md"
missing_pcie_probe_evidence="$tmp_dir/missing-pcie-probe.md"
missing_payload_disabled_evidence="$tmp_dir/missing-payload-disabled.md"
missing_halt_evidence="$tmp_dir/missing-halt.md"
prompt_after_halt_evidence="$tmp_dir/prompt-after-halt.md"
missing_proof_evidence="$tmp_dir/missing-proof.md"
missing_proof_command_evidence="$tmp_dir/missing-proof-command.md"
missing_proof_identity_evidence="$tmp_dir/missing-proof-identity.md"
missing_proof_screentest_fact_evidence="$tmp_dir/missing-proof-screentest-fact.md"
missing_vfs_help_evidence="$tmp_dir/missing-vfs-help.md"
missing_vfs_proof_evidence="$tmp_dir/missing-vfs-proof.md"
missing_vfs_probe_catalog_evidence="$tmp_dir/missing-vfs-probe-catalog.md"
missing_final_log_sequence_evidence="$tmp_dir/missing-final-log-sequence.md"
missing_screentest_erase_modes_evidence="$tmp_dir/missing-screentest-erase-modes.md"
missing_log_stats_evidence="$tmp_dir/missing-log-stats.md"
missing_log_discovery_evidence="$tmp_dir/missing-log-discovery.md"
preflight_only_evidence="$tmp_dir/preflight-only.md"
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
        printf 'version=0.16.0-dev\n'
        printf 'target=aarch64-unknown-none\n'
        printf 'selected_profile=shell-only\n'
        printf 'profile_request=shell-only\n'
        printf 'bootline=absent\n'
        printf 'launch_profile_feature=disabled\n'
        printf '```\n\n'
        printf 'Required facts:\n\n'
        printf -- '- [x] `package=reovim-os`\n'
        printf -- '- [x] `/boot/image` reports a `version=` line.\n'
        printf -- '- [x] `target=aarch64-unknown-none`\n'
        printf -- '- [x] `selected_profile=shell-only`\n'
        printf -- '- [x] `profile_request=shell-only`\n'
        printf -- '- [x] `bootline=absent`\n'
        printf -- '- [x] `launch_profile_feature=disabled`\n'
        printf -- '- [x] shell prompt reached: `reovim-os>`\n'
        printf -- '- [x] input source is recorded honestly\n'
        printf -- '- [x] QEMU/VNC/HDMI display was not counted as keyboard input\n\n'
        printf '## Command Transcript\n\n'
        printf '```text\n'
        printf 'reovim-os> proof\n'
        printf 'proof:\n'
        printf 'commands:\n'
        printf '  proof\n'
        printf '  cat /boot/proof\n'
        printf '  help\n'
        printf '  help screentest\n'
        printf '  help input\n'
        printf '  help proof\n'
        printf '  help pwd\n'
        printf '  help ls\n'
        printf '  help cd\n'
        printf '  help cat\n'
        printf '  help mount\n'
        printf '  help dmesg\n'
        printf '  help status\n'
        printf '  help probe\n'
        printf '  cat /boot/help\n'
        printf '  clear\n'
        printf '  screentest\n'
        printf '  pwd\n'
        printf '  ls /\n'
        printf '  ls /boot\n'
        printf '  ls /dev\n'
        printf '  ls /log\n'
        printf '  mount\n'
        printf '  cat /boot/mounts\n'
        printf '  device\n'
        printf '  cat /boot/memory\n'
        printf '  cat /boot/devices\n'
        printf '  cd /dev\n'
        printf '  pwd\n'
        printf '  ls\n'
        printf '  cat uart0\n'
        printf '  cd /\n'
        printf '  cat /boot/image\n'
        printf '  status\n'
        printf '  cat /boot/status\n'
        printf '  input\n'
        printf '  cat /boot/input\n'
        printf '  probe help\n'
        printf '  cat /boot/probes\n'
        printf '  probe pcie\n'
        printf '  probe usb-keyboard\n'
        printf '  cat /boot/profile\n'
        printf '  launch\n'
        printf '  reovim\n'
        printf '  dmesg --stats\n'
        printf '  cat /log/stats\n'
        printf '  dmesg\n'
        printf '  cat /log/dmesg\n'
        printf 'terminal:\n'
        printf '  halt\n'
        printf 'expected:\n'
        printf '  package=reovim-os\n'
        printf '  version=0.16.0-dev\n'
        printf '  target=aarch64-unknown-none\n'
        printf '  selected_profile=shell-only\n'
        printf '  profile_request=shell-only\n'
        printf '  launch_profile_feature=disabled\n'
        printf '  profile=shell-only\n'
        printf '  launch=disabled\n'
        printf '  payloads=0\n'
        printf '  bootline=absent\n'
        printf '  source=usb-keyboard+uart-fallback\n'
        printf '  source_state=ready\n'
        printf '  mode=live\n'
        printf '  input_mode=live\n'
        printf '  usb_keyboard=ready\n'
        printf '  usb_keyboard_probe=enabled\n'
        printf '  usb_keyboard_last_poll=report-ready\n'
        printf '  manual_next=type-shell-command\n'
        printf '  help catalog available through /boot/help\n'
        printf '  screentest includes erase-line mode diagnostics\n'
        printf '  kernel log stats available through /log/stats\n'
        printf '  probe catalog available through /boot/probes\n'
        printf '  probe targets include pcie\n'
        printf '  probe targets include usb-keyboard\n'
        printf '  probe targets include xhci-read-keyboard-report\n'
        printf '  launch/reovim disabled in shell-only profile\n'
        printf '  shell.status=ok\n'
        printf '  shell.status=error for disabled payload commands\n'
        printf '  halt typed last prints halt: ok and stops root daemon\n'
        printf 'reovim-os> cat /boot/proof\n'
        printf 'proof:\n'
        printf 'commands:\n'
        printf '  proof\n'
        printf '  cat /boot/proof\n'
        printf '  help\n'
        printf '  help screentest\n'
        printf '  help input\n'
        printf '  help proof\n'
        printf '  help pwd\n'
        printf '  help ls\n'
        printf '  help cd\n'
        printf '  help cat\n'
        printf '  help mount\n'
        printf '  help dmesg\n'
        printf '  help status\n'
        printf '  help probe\n'
        printf '  cat /boot/help\n'
        printf '  clear\n'
        printf '  screentest\n'
        printf '  pwd\n'
        printf '  ls /\n'
        printf '  ls /boot\n'
        printf '  ls /dev\n'
        printf '  ls /log\n'
        printf '  mount\n'
        printf '  cat /boot/mounts\n'
        printf '  device\n'
        printf '  cat /boot/memory\n'
        printf '  cat /boot/devices\n'
        printf '  cd /dev\n'
        printf '  pwd\n'
        printf '  ls\n'
        printf '  cat uart0\n'
        printf '  cd /\n'
        printf '  cat /boot/image\n'
        printf '  status\n'
        printf '  cat /boot/status\n'
        printf '  input\n'
        printf '  cat /boot/input\n'
        printf '  probe help\n'
        printf '  cat /boot/probes\n'
        printf '  probe pcie\n'
        printf '  probe usb-keyboard\n'
        printf '  cat /boot/profile\n'
        printf '  launch\n'
        printf '  reovim\n'
        printf '  dmesg --stats\n'
        printf '  cat /log/stats\n'
        printf '  dmesg\n'
        printf '  cat /log/dmesg\n'
        printf 'terminal:\n'
        printf '  halt\n'
        printf 'expected:\n'
        printf '  package=reovim-os\n'
        printf '  version=0.16.0-dev\n'
        printf '  target=aarch64-unknown-none\n'
        printf '  selected_profile=shell-only\n'
        printf '  profile_request=shell-only\n'
        printf '  launch_profile_feature=disabled\n'
        printf '  profile=shell-only\n'
        printf '  launch=disabled\n'
        printf '  payloads=0\n'
        printf '  bootline=absent\n'
        printf '  source=usb-keyboard+uart-fallback\n'
        printf '  source_state=ready\n'
        printf '  mode=live\n'
        printf '  input_mode=live\n'
        printf '  usb_keyboard=ready\n'
        printf '  usb_keyboard_probe=enabled\n'
        printf '  usb_keyboard_last_poll=report-ready\n'
        printf '  manual_next=type-shell-command\n'
        printf '  help catalog available through /boot/help\n'
        printf '  screentest includes erase-line mode diagnostics\n'
        printf '  kernel log stats available through /log/stats\n'
        printf '  probe catalog available through /boot/probes\n'
        printf '  probe targets include pcie\n'
        printf '  probe targets include usb-keyboard\n'
        printf '  probe targets include xhci-read-keyboard-report\n'
        printf '  launch/reovim disabled in shell-only profile\n'
        printf '  shell.status=ok\n'
        printf '  shell.status=error for disabled payload commands\n'
        printf '  halt typed last prints halt: ok and stops root daemon\n'
        printf 'reovim-os> help\n'
        printf 'reovim root shell\n'
        printf 'commands: help, clear, screentest, pwd, ls, cd, cat, mount, input, status, proof, device, dmesg, probe, launch, reovim, halt\n'
        printf 'usage: help [command]\n'
        printf 'reovim-os> help screentest\n'
        printf 'screentest - print renderer diagnostics\n'
        printf '  required rows: el: clean, el1: clean-left, el2: clean-all\n'
        printf 'reovim-os> help input\n'
        printf 'input - print live console input diagnostics\n'
        printf 'reovim-os> help proof\n'
        printf 'proof - print physical input proof checklist\n'
        printf 'reovim-os> help pwd\n'
        printf 'pwd - print current kernel VFS directory\n'
        printf 'reovim-os> help ls\n'
        printf 'ls [path] - list a kernel VFS directory\n'
        printf 'reovim-os> help cd\n'
        printf 'cd [path] - change current kernel VFS directory\n'
        printf 'reovim-os> help cat\n'
        printf 'cat path... - print kernel VFS pseudo files\n'
        printf 'reovim-os> help mount\n'
        printf 'mount - print kernel VFS mount table\n'
        printf 'reovim-os> help dmesg\n'
        printf 'dmesg [--stats] - print retained kernel log or ring stats\n'
        printf 'reovim-os> help status\n'
        printf 'status - print boot, input, and manual_next summary\n'
        printf 'reovim-os> help probe\n'
        printf 'probe target - run lower hardware probe; try `probe help` or `cat /boot/probes`\n'
        printf 'reovim-os> cat /boot/help\n'
        printf 'reovim root shell\n'
        printf 'commands: help, clear, screentest, pwd, ls, cd, cat, mount, input, status, proof, device, dmesg, probe, launch, reovim, halt\n'
        printf 'usage: help [command]\n'
        printf 'reovim-os> clear\n'
        printf '\033[2J\033[H'
        printf 'reovim-os> screentest\n'
        printf 'screen test:\n'
        printf '  target: framebuffer/serial tty renderer subset\n'
        printf '  idx-fg: idx21 idx46 idx51 idx93 idx160 idx196 idx201 idx226\n'
        printf '  rgb-bg: warm green sky violet\n'
        printf '  attrs: bold dim italic underline reverse bold+underline normal\n'
        printf '  el: clean\n'
        printf '  el1: clean-left\n'
        printf '  el2: clean-all\n'
        printf '  done\n'
        printf 'reovim-os> pwd\n'
        printf '/\n'
        printf 'reovim-os> ls /\n'
        printf 'boot\n'
        printf 'dev\n'
        printf 'log\n'
        printf 'reovim-os> ls /boot\n'
        printf 'devices\n'
        printf 'help\n'
        printf 'image\n'
        printf 'input\n'
        printf 'memory\n'
        printf 'mounts\n'
        printf 'proof\n'
        printf 'profile\n'
        printf 'status\n'
        printf 'reovim-os> ls /dev\n'
        printf 'uart0\n'
        printf 'reovim-os> ls /log\n'
        printf 'dmesg\n'
        printf 'stats\n'
        printf 'reovim-os> mount\n'
        printf 'kernel on / type rootfs (ro,pseudo)\n'
        printf 'boot on /boot type bootfs (ro,pseudo)\n'
        printf 'devices on /dev type devfs (ro,pseudo)\n'
        printf 'klog on /log type logfs (ro,pseudo)\n'
        printf 'reovim-os> cat /boot/mounts\n'
        printf 'kernel on / type rootfs (ro,pseudo)\n'
        printf 'boot on /boot type bootfs (ro,pseudo)\n'
        printf 'devices on /dev type devfs (ro,pseudo)\n'
        printf 'klog on /log type logfs (ro,pseudo)\n'
        printf 'reovim-os> device\n'
        printf 'boot_info:\n'
        printf '  ranges=1\n'
        printf '  usable_bytes=4096\n'
        printf '  cpu_count=1\n'
        printf '  heap_total_bytes=0\n'
        printf '  cpu_freq_hz=0\n'
        printf '  mem_freq_hz=0\n'
        printf '  cache_line_bytes=64\n'
        printf 'devices:\n'
        printf -- '- [0] uart compat=arm,pl011 mmio=0x1000/0x100 irq=12\n'
        printf 'reovim-os> cat /boot/memory\n'
        printf 'ranges=1\n'
        printf 'usable_bytes=4096\n'
        printf 'cpu_count=1\n'
        printf 'heap_total_bytes=0\n'
        printf 'cache_line_bytes=64\n'
        printf 'reovim-os> cat /boot/devices\n'
        printf -- '- [0] uart compat=arm,pl011 mmio=0x1000/0x100 irq=12\n'
        printf 'reovim-os> cd /dev\n'
        printf 'reovim-os> pwd\n'
        printf '/dev\n'
        printf 'reovim-os> ls\n'
        printf 'uart0\n'
        printf 'reovim-os> cat uart0\n'
        printf -- '- [0] uart compat=arm,pl011 mmio=0x1000/0x100 irq=12\n'
        printf 'reovim-os> cd /\n'
        printf 'reovim-os> cat /boot/image\n'
        printf 'package=reovim-os\n'
        printf 'version=0.16.0-dev\n'
        printf 'target=aarch64-unknown-none\n'
        printf 'selected_profile=shell-only\n'
        printf 'profile_request=shell-only\n'
        printf 'bootline=absent\n'
        printf 'launch_profile_feature=disabled\n'
        printf 'reovim-os> status\n'
        printf 'package=reovim-os\n'
        printf 'version=0.16.0-dev\n'
        printf 'target=aarch64-unknown-none\n'
        printf 'selected_profile=shell-only\n'
        printf 'profile_request=shell-only\n'
        printf 'bootline=absent\n'
        printf 'launch_profile_feature=disabled\n'
        printf 'profile=shell-only\n'
        printf 'launch=disabled\n'
        printf 'payloads=0\n'
        printf 'input=usb-keyboard+uart-fallback\n'
        printf 'source_state=ready\n'
        printf 'input_mode=live\n'
        printf 'usb_keyboard=ready\n'
        printf 'usb_keyboard_probe=enabled\n'
        printf 'usb_keyboard_poll_interval_ms=5\n'
        printf 'usb_keyboard_last_poll=report-ready\n'
        printf 'manual_next=type-shell-command\n'
        printf 'reovim-os> cat /boot/status\n'
        printf 'package=reovim-os\n'
        printf 'version=0.16.0-dev\n'
        printf 'target=aarch64-unknown-none\n'
        printf 'selected_profile=shell-only\n'
        printf 'profile_request=shell-only\n'
        printf 'bootline=absent\n'
        printf 'launch_profile_feature=disabled\n'
        printf 'profile=shell-only\n'
        printf 'launch=disabled\n'
        printf 'payloads=0\n'
        printf 'input=usb-keyboard+uart-fallback\n'
        printf 'source_state=ready\n'
        printf 'input_mode=live\n'
        printf 'usb_keyboard=ready\n'
        printf 'usb_keyboard_probe=enabled\n'
        printf 'usb_keyboard_poll_interval_ms=5\n'
        printf 'usb_keyboard_last_poll=report-ready\n'
        printf 'manual_next=type-shell-command\n'
        printf 'reovim-os> input\n'
        printf 'source=usb-keyboard+uart-fallback\n'
        printf 'source_state=ready\n'
        printf 'mode=live\n'
        printf 'usb_keyboard=ready\n'
        printf 'usb_keyboard_probe=enabled\n'
        printf 'usb_keyboard_poll_interval_ms=5\n'
        printf 'usb_keyboard_last_poll=report-ready\n'
        printf 'reovim-os> cat /boot/input\n'
        printf 'source=usb-keyboard+uart-fallback\n'
        printf 'source_state=ready\n'
        printf 'mode=live\n'
        printf 'usb_keyboard=ready\n'
        printf 'usb_keyboard_probe=enabled\n'
        printf 'usb_keyboard_poll_interval_ms=5\n'
        printf 'usb_keyboard_last_poll=report-ready\n'
        printf 'reovim-os> probe help\n'
        printf 'probe targets:\n'
        printf '  pcie\n'
        printf '  usb-keyboard (alias: keyboard)\n'
        printf '  xhci-read-keyboard-report (alias: usb-keyboard-read-report)\n'
        printf 'reovim-os> cat /boot/probes\n'
        printf 'probe targets:\n'
        printf '  pcie\n'
        printf '  usb-keyboard (alias: keyboard)\n'
        printf '  xhci-read-keyboard-report (alias: usb-keyboard-read-report)\n'
        printf 'reovim-os> probe pcie\n'
        printf 'probe pcie:\n'
        printf 'state=present\n'
        printf 'raw_status=0x00000001\n'
        printf 'root_complex=true\n'
        printf 'link_up=true\n'
        printf 'xhci=present\n'
        printf 'xhci.vendor=0x00001106\n'
        printf 'xhci.device_id=0x00003483\n'
        printf 'reovim-os> probe usb-keyboard\n'
        printf 'probe usb-keyboard:\n'
        printf 'state=report-pending\n'
        printf 'slot_id=1\n'
        printf 'endpoint_id=3\n'
        printf 'reovim-os> cat /boot/profile\n'
        printf 'profile=shell-only\n'
        printf 'launch=disabled\n'
        printf 'payloads=0\n'
        printf 'prompt=reovim-os> \n'
        printf 'input=usb-keyboard+uart-fallback\n'
        printf 'input_mode=live\n'
        printf 'usb_keyboard=ready\n'
        printf 'reovim-os> launch\n'
        printf 'launch disabled for this profile\n'
        printf 'reovim-os> reovim\n'
        printf 'reovim disabled for this profile\n'
        printf 'reovim-os> dmesg --stats\n'
        printf 'capacity_bytes=8192\n'
        printf 'retained_bytes=2048\n'
        printf 'dropped_bytes=0\n'
        printf 'reovim-os> cat /log/stats\n'
        printf 'capacity_bytes=8192\n'
        printf 'retained_bytes=2100\n'
        printf 'dropped_bytes=0\n'
        printf 'reovim-os> dmesg\n'
        printf 'dmesg:\n'
        printf 'shell: proof\n'
        printf 'shell.status=ok\n'
        printf 'shell: cat /boot/proof\n'
        printf 'shell.status=ok\n'
        printf 'shell: help\n'
        printf 'shell.status=ok\n'
        printf 'shell: help screentest\n'
        printf 'shell.status=ok\n'
        printf 'shell: help input\n'
        printf 'shell.status=ok\n'
        printf 'shell: help proof\n'
        printf 'shell.status=ok\n'
        printf 'shell: help pwd\n'
        printf 'shell.status=ok\n'
        printf 'shell: help ls\n'
        printf 'shell.status=ok\n'
        printf 'shell: help cd\n'
        printf 'shell.status=ok\n'
        printf 'shell: help cat\n'
        printf 'shell.status=ok\n'
        printf 'shell: help mount\n'
        printf 'shell.status=ok\n'
        printf 'shell: help dmesg\n'
        printf 'shell.status=ok\n'
        printf 'shell: help status\n'
        printf 'shell.status=ok\n'
        printf 'shell: help probe\n'
        printf 'shell.status=ok\n'
        printf 'shell: cat /boot/help\n'
        printf 'shell.status=ok\n'
        printf 'shell: clear\n'
        printf 'shell.status=ok\n'
        printf 'shell: screentest\n'
        printf 'shell.status=ok\n'
        printf 'shell: pwd\n'
        printf 'shell.status=ok\n'
        printf 'shell: ls /\n'
        printf 'shell.status=ok\n'
        printf 'shell: ls /boot\n'
        printf 'shell.status=ok\n'
        printf 'shell: ls /dev\n'
        printf 'shell.status=ok\n'
        printf 'shell: ls /log\n'
        printf 'shell.status=ok\n'
        printf 'shell: mount\n'
        printf 'shell.status=ok\n'
        printf 'shell: cat /boot/mounts\n'
        printf 'shell.status=ok\n'
        printf 'shell: device\n'
        printf 'shell.status=ok\n'
        printf 'shell: cat /boot/memory\n'
        printf 'shell.status=ok\n'
        printf 'shell: cat /boot/devices\n'
        printf 'shell.status=ok\n'
        printf 'shell: cd /dev\n'
        printf 'shell.status=ok\n'
        printf 'shell: pwd\n'
        printf 'shell.status=ok\n'
        printf 'shell: ls\n'
        printf 'shell.status=ok\n'
        printf 'shell: cat uart0\n'
        printf 'shell.status=ok\n'
        printf 'shell: cd /\n'
        printf 'shell.status=ok\n'
        printf 'shell: cat /boot/image\n'
        printf 'shell.status=ok\n'
        printf 'shell: status\n'
        printf 'shell.status=ok\n'
        printf 'shell: cat /boot/status\n'
        printf 'shell.status=ok\n'
        printf 'shell: input\n'
        printf 'shell.status=ok\n'
        printf 'shell: cat /boot/input\n'
        printf 'shell.status=ok\n'
        printf 'shell: probe help\n'
        printf 'shell.status=ok\n'
        printf 'shell: cat /boot/probes\n'
        printf 'shell.status=ok\n'
        printf 'shell: probe pcie\n'
        printf 'shell.status=ok\n'
        printf 'shell: probe usb-keyboard\n'
        printf 'shell.status=ok\n'
        printf 'shell: cat /boot/profile\n'
        printf 'shell.status=ok\n'
        printf 'shell: launch\n'
        printf 'shell.status=error\n'
        printf 'shell: reovim\n'
        printf 'shell.status=error\n'
        printf 'shell: dmesg --stats\n'
        printf 'shell.status=ok\n'
        printf 'shell: cat /log/stats\n'
        printf 'shell.status=ok\n'
        printf 'shell: dmesg\n'
        printf 'reovim-os> cat /log/dmesg\n'
        printf 'shell: dmesg\n'
        printf 'shell.status=ok\n'
        printf 'shell: cat /log/dmesg\n'
        printf '```\n\n'
        printf '## Shutdown Check\n\n'
        printf '```text\n'
        printf 'reovim-os> halt\n'
        printf 'halt: ok\n'
        printf '```\n\n'
        printf '## USB Keyboard Readiness\n\n'
        printf 'Required success facts:\n\n'
        printf -- '- [x] A physical USB keypress reached the root shell.\n'
        printf -- '- [x] `proof` prints the physical input proof checklist.\n'
        printf -- '- [x] `cat /boot/proof` prints the VFS-backed proof pseudo-file.\n'
        printf -- '- [x] `cat /boot/help` prints the VFS-backed help catalog.\n'
        printf -- '- [x] `help dmesg` / `ls /log` make kernel log stats discoverable.\n'
        printf -- '- [x] `help input` / `help proof` make live input diagnostics and the proof checklist self-describing.\n'
        printf -- '- [x] `help status` / `help probe` make `manual_next` and probe catalog discovery self-describing.\n'
        printf -- '- [x] `help pwd` / `help ls` / `help cd` / `help cat` / `help mount` make VFS navigation self-describing.\n'
        printf -- '- [x] `help` / `help screentest` / `clear` / `screentest` prove shell help and erase-line mode diagnostics.\n'
        printf -- '- [x] `pwd` / `ls` / `mount` report the kernel VFS namespace and mounts.\n'
        printf -- '- [x] `device` / `cat /boot/memory` / `cat /boot/devices` report boot inventory.\n'
        printf -- '- [x] `cd /dev` / `ls` / `cat uart0` prove relative VFS device-file access.\n'
        printf -- '- [x] `input=usb-keyboard+uart-fallback`\n'
        printf -- '- [x] `usb_keyboard=ready`\n'
        printf -- '- [x] `usb_keyboard_last_poll=report-ready`\n'
        printf -- '- [x] `manual_next=type-shell-command`\n'
        printf -- '- [x] `status` / `cat /boot/status` report image/profile identity and live ready source diagnostics.\n'
        printf -- '- [x] `input` / `cat /boot/input` report live input diagnostics.\n'
        printf -- '- [x] `cat /boot/probes` prints the VFS-backed probe catalog.\n'
        printf -- '- [x] `probe help` lists `pcie`, `usb-keyboard`, and `xhci-read-keyboard-report`.\n'
        printf -- '- [x] `probe pcie` reports the read-only PCIe/xHCI state.\n'
        printf -- '- [x] `cat /boot/profile` reports `profile=shell-only`, `launch=disabled`, `payloads=0`, and `input_mode=live`.\n'
        printf -- '- [x] `launch` / `reovim` report shell-only payload launch disabled.\n'
        printf -- '- [x] `dmesg --stats` / `cat /log/stats` report kernel log ring stats.\n'
        printf -- '- [x] `dmesg` contains `shell: <command>` and `shell.status=ok` audit lines for the typed commands.\n'
        printf -- '- [x] `dmesg` contains `shell.status=error` audit lines for the disabled payload launch commands.\n'
        printf -- '- [x] `dmesg` contains the `probe usb-keyboard` output or blocker.\n'
        printf -- '- [x] `halt` was typed last and printed `halt: ok`.\n'
        printf -- '- [x] No new `reovim-os>` prompt appeared after `halt: ok`.\n\n'
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

cp "$pass_evidence" "$missing_status_evidence"
sed -i '/^shell\.status=ok$/d' "$missing_status_evidence"
expect_failure "$missing_status_evidence" "missing-status"

cp "$pass_evidence" "$missing_probe_help_evidence"
sed -i '/^  usb-keyboard (alias: keyboard)$/d' "$missing_probe_help_evidence"
expect_failure "$missing_probe_help_evidence" "missing-probe-help"

cp "$pass_evidence" "$missing_targeted_screentest_help_evidence"
sed -i '\#^reovim-os> help screentest$#d' "$missing_targeted_screentest_help_evidence"
expect_failure "$missing_targeted_screentest_help_evidence" "missing-targeted-screentest-help"

cp "$pass_evidence" "$missing_targeted_input_help_evidence"
sed -i '\#^reovim-os> help input$#d' "$missing_targeted_input_help_evidence"
expect_failure "$missing_targeted_input_help_evidence" "missing-targeted-input-help"

cp "$pass_evidence" "$missing_targeted_proof_help_evidence"
sed -i '\#^reovim-os> help proof$#d' "$missing_targeted_proof_help_evidence"
expect_failure "$missing_targeted_proof_help_evidence" "missing-targeted-proof-help"

cp "$pass_evidence" "$missing_targeted_vfs_help_evidence"
sed -i '\#^reovim-os> help ls$#d' "$missing_targeted_vfs_help_evidence"
expect_failure "$missing_targeted_vfs_help_evidence" "missing-targeted-vfs-help"

cp "$pass_evidence" "$missing_targeted_help_evidence"
sed -i '\#^reovim-os> help status$#d' "$missing_targeted_help_evidence"
expect_failure "$missing_targeted_help_evidence" "missing-targeted-help"

cp "$pass_evidence" "$missing_live_source_evidence"
sed -i '/^source=usb-keyboard+uart-fallback$/d' "$missing_live_source_evidence"
expect_failure "$missing_live_source_evidence" "missing-live-source"

cp "$pass_evidence" "$missing_last_poll_evidence"
sed -i '/^usb_keyboard_last_poll=report-ready$/d' "$missing_last_poll_evidence"
expect_failure "$missing_last_poll_evidence" "missing-last-poll"

cp "$pass_evidence" "$missing_manual_next_evidence"
sed -i '/^manual_next=type-shell-command$/d' "$missing_manual_next_evidence"
expect_failure "$missing_manual_next_evidence" "missing-manual-next"

cp "$pass_evidence" "$missing_vfs_live_source_evidence"
sed -i '\#^reovim-os> cat /boot/status$#d' "$missing_vfs_live_source_evidence"
expect_failure "$missing_vfs_live_source_evidence" "missing-vfs-live-source"

cp "$pass_evidence" "$missing_image_identity_evidence"
sed -i '/^selected_profile=shell-only$/d' "$missing_image_identity_evidence"
expect_failure "$missing_image_identity_evidence" "missing-image-identity"

cp "$pass_evidence" "$missing_profile_identity_evidence"
sed -i '/^profile=shell-only$/d' "$missing_profile_identity_evidence"
expect_failure "$missing_profile_identity_evidence" "missing-profile-identity"

cp "$pass_evidence" "$missing_identity_checkbox_evidence"
sed -i '/^- \[x\] `selected_profile=shell-only`$/d' "$missing_identity_checkbox_evidence"
expect_failure "$missing_identity_checkbox_evidence" "missing-identity-checkbox"

cp "$pass_evidence" "$missing_vfs_namespace_evidence"
sed -i '\#^reovim-os> ls /boot$#d' "$missing_vfs_namespace_evidence"
expect_failure "$missing_vfs_namespace_evidence" "missing-vfs-namespace"

cp "$pass_evidence" "$missing_boot_inventory_evidence"
sed -i '\#^reovim-os> cat /boot/devices$#d' "$missing_boot_inventory_evidence"
expect_failure "$missing_boot_inventory_evidence" "missing-boot-inventory"

cp "$pass_evidence" "$missing_shell_usability_evidence"
sed -i '/^screen test:$/d' "$missing_shell_usability_evidence"
expect_failure "$missing_shell_usability_evidence" "missing-shell-usability"

cp "$pass_evidence" "$missing_screentest_erase_modes_evidence"
sed -i '/^  el1: clean-left$/d; /^  el2: clean-all$/d' "$missing_screentest_erase_modes_evidence"
expect_failure "$missing_screentest_erase_modes_evidence" "missing-screentest-erase-modes"

cp "$pass_evidence" "$missing_log_stats_evidence"
sed -i '/^capacity_bytes=8192$/d' "$missing_log_stats_evidence"
expect_failure "$missing_log_stats_evidence" "missing-log-stats"

cp "$pass_evidence" "$missing_log_discovery_evidence"
sed -i '\#^reovim-os> ls /log$#d' "$missing_log_discovery_evidence"
expect_failure "$missing_log_discovery_evidence" "missing-log-discovery"

cp "$pass_evidence" "$missing_typed_command_evidence"
sed -i '\#^reovim-os> status$#d' "$missing_typed_command_evidence"
expect_failure "$missing_typed_command_evidence" "missing-typed-command"

cp "$pass_evidence" "$missing_profile_prompt_evidence"
sed -i '\#^reovim-os> cat /boot/profile$#d' "$missing_profile_prompt_evidence"
expect_failure "$missing_profile_prompt_evidence" "missing-profile-prompt"

cp "$pass_evidence" "$missing_pcie_probe_evidence"
sed -i '\#^reovim-os> probe pcie$#d' "$missing_pcie_probe_evidence"
expect_failure "$missing_pcie_probe_evidence" "missing-pcie-probe"

cp "$pass_evidence" "$missing_payload_disabled_evidence"
sed -i '/^reovim disabled for this profile$/d' "$missing_payload_disabled_evidence"
expect_failure "$missing_payload_disabled_evidence" "missing-payload-disabled"

cp "$pass_evidence" "$missing_halt_evidence"
sed -i '/^halt: ok$/d' "$missing_halt_evidence"
expect_failure "$missing_halt_evidence" "missing-halt"

cp "$pass_evidence" "$prompt_after_halt_evidence"
sed -i '/^halt: ok$/a reovim-os> ' "$prompt_after_halt_evidence"
expect_failure "$prompt_after_halt_evidence" "prompt-after-halt"

cp "$pass_evidence" "$missing_proof_evidence"
sed -i '/^proof:$/d' "$missing_proof_evidence"
expect_failure "$missing_proof_evidence" "missing-proof"

cp "$pass_evidence" "$missing_proof_command_evidence"
sed -i '\#^  cat /boot/proof$#d' "$missing_proof_command_evidence"
expect_failure "$missing_proof_command_evidence" "missing-proof-command"

cp "$pass_evidence" "$missing_proof_identity_evidence"
sed -i '/^  selected_profile=shell-only$/d' "$missing_proof_identity_evidence"
expect_failure "$missing_proof_identity_evidence" "missing-proof-identity"

cp "$pass_evidence" "$missing_proof_screentest_fact_evidence"
sed -i '/^  screentest includes erase-line mode diagnostics$/d' "$missing_proof_screentest_fact_evidence"
expect_failure "$missing_proof_screentest_fact_evidence" "missing-proof-screentest-fact"

cp "$pass_evidence" "$missing_vfs_help_evidence"
sed -i '\#^reovim-os> cat /boot/help$#d' "$missing_vfs_help_evidence"
expect_failure "$missing_vfs_help_evidence" "missing-vfs-help"

cp "$pass_evidence" "$missing_vfs_proof_evidence"
sed -i '\#^reovim-os> cat /boot/proof$#d' "$missing_vfs_proof_evidence"
expect_failure "$missing_vfs_proof_evidence" "missing-vfs-proof"

cp "$pass_evidence" "$missing_vfs_probe_catalog_evidence"
sed -i '\#^reovim-os> cat /boot/probes$#d' "$missing_vfs_probe_catalog_evidence"
expect_failure "$missing_vfs_probe_catalog_evidence" "missing-vfs-probe-catalog"

awk '
    /^shell: dmesg$/ {
        print
        next_line = ""
        if (getline next_line > 0 && next_line == "shell.status=ok") {
            next
        }
        if (next_line != "") {
            print next_line
        }
        next
    }
    { print }
' "$pass_evidence" >"$missing_final_log_sequence_evidence"
expect_failure "$missing_final_log_sequence_evidence" "missing-final-log-sequence"

awk '
    /^## Media Preparation$/ {
        skip = 1
        next
    }
    /^## Boot Evidence$/ {
        skip = 0
    }
    !skip { print }
' "$pass_evidence" >"$preflight_only_evidence"
expect_failure "$preflight_only_evidence" "preflight-only"

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
