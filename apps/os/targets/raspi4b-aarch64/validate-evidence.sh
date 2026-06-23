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
require_re '^- \[[xX]\] `package=reovim-os`$' 'package proof checkbox'
require_re '^- \[[xX]\] `/boot/image` reports a `version=` line\.$' 'version proof checkbox'
require_re '^- \[[xX]\] `selected_profile=shell-only`$' 'selected-profile proof checkbox'
require_re '^- \[[xX]\] `profile_request=shell-only`$' 'profile-request proof checkbox'
require_re '^- \[[xX]\] `bootline=absent`$' 'bootline-absent proof checkbox'
require_re '^- \[[xX]\] `launch_profile_feature=disabled`$' 'launch-profile-feature proof checkbox'
require_re '^- \[[xX]\] shell prompt reached: `reovim-os>`$' 'root prompt proof checkbox'
require_re '^- \[[xX]\] input source is recorded honestly$' 'honest input-source checkbox'
require_re '^- \[[xX]\] QEMU/VNC/HDMI display was not counted as keyboard input$' 'non-keyboard display evidence rejected'

require_re '^- \[[xX]\] A physical USB keypress reached the root shell\.$' 'physical keypress reached shell checkbox'
require_re '^- \[[xX]\] `input=usb-keyboard\+uart-fallback`$' 'USB keyboard input-source checkbox'
require_re '^- \[[xX]\] `usb_keyboard=ready`$' 'USB keyboard readiness checkbox'
require_re '^- \[[xX]\] `status` / `cat /boot/status` report image/profile identity and live ready source diagnostics\.$' 'status identity/live diagnostics checkbox'
require_re '^- \[[xX]\] `input` / `cat /boot/input` report live input diagnostics\.$' 'input live diagnostics checkbox'
require_re '^- \[[xX]\] `probe help` lists `pcie`, `usb-keyboard`, and `xhci-read-keyboard-report`\.$' 'probe help catalog checkbox'
require_re '^- \[[xX]\] `probe pcie` reports the read-only PCIe/xHCI state\.$' 'PCIe probe checkbox'
require_re '^- \[[xX]\] `cat /boot/profile` reports `profile=shell-only`, `launch=disabled`, `payloads=0`, and `input_mode=live`\.$' 'profile identity checkbox'
require_re '^- \[[xX]\] `launch` / `reovim` report shell-only payload launch disabled\.$' 'shell-only launch-disabled checkbox'
require_re '^- \[[xX]\] `dmesg` contains `shell: <command>` and `shell\.status=ok` audit lines for the typed commands\.$' 'dmesg command/status audit checkbox'
require_re '^- \[[xX]\] `dmesg` contains `shell\.status=error` audit lines for the disabled payload launch commands\.$' 'dmesg payload-disabled error audit checkbox'
require_re '^- \[[xX]\] `dmesg` contains the `probe usb-keyboard` output or blocker\.$' 'probe-output dmesg checkbox'
require_re '^- \[[xX]\] `halt` was typed last and printed `halt: ok`\.$' 'terminal halt checkbox'
require_re '^- \[[xX]\] No new `reovim-os>` prompt appeared after `halt: ok`\.$' 'terminal halt no-prompt checkbox'
require_re '^- \[[xX]\] `proof` prints the physical input proof checklist\.$' 'proof checklist checkbox'
require_re '^- \[[xX]\] `cat /boot/proof` prints the VFS-backed proof pseudo-file\.$' 'VFS proof pseudo-file checkbox'
require_re '^- \[[xX]\] `cat /boot/help` prints the VFS-backed help catalog\.$' 'VFS help pseudo-file checkbox'
require_re '^- \[[xX]\] `help` / `clear` / `screentest` prove the shell help and renderer commands\.$' 'shell help and renderer checkbox'
require_re '^- \[[xX]\] `pwd` / `ls` / `mount` report the kernel VFS namespace and mounts\.$' 'VFS namespace and mounts checkbox'
require_re '^- \[[xX]\] `device` / `cat /boot/memory` / `cat /boot/devices` report boot inventory\.$' 'boot inventory checkbox'
require_re '^- \[[xX]\] `cd /dev` / `ls` / `cat uart0` prove relative VFS device-file access\.$' 'relative device-file checkbox'

require_re '^reovim-os>' 'root shell prompt in pasted transcript'
require_re '^media_prepare=ok$' 'media-prep result line'
require_re '^installed=.+/kernel8\.img$' 'installed kernel8.img path line'
require_re '^bytes=[1-9][0-9]*$' 'installed kernel8.img byte count line'
require_re '^sha256=[0-9a-f]{64}$' 'installed kernel8.img SHA-256 line'
require_re '^package=reovim-os$' 'cat /boot/image package line'
require_re '^version=[0-9A-Za-z.+_-]+$' 'cat /boot/image version line'
require_re '^target=aarch64-unknown-none$' 'cat /boot/image target line'
require_re '^selected_profile=shell-only$' 'cat /boot/image selected profile line'
require_re '^profile_request=shell-only$' 'cat /boot/image requested profile line'
require_re '^bootline=absent$' 'cat /boot/image bootline line'
require_re '^launch_profile_feature=disabled$' 'cat /boot/image launch-profile feature line'
require_re '^profile=shell-only$' 'cat /boot/profile profile line'
require_re '^launch=disabled$' 'cat /boot/profile launch-disabled line'
require_re '^payloads=0$' 'cat /boot/profile payload count line'
require_re '^prompt=reovim-os> ?$' 'cat /boot/profile prompt line'
require_count_at_least '^package=reovim-os$' 'package through image and status surfaces' 3
require_count_at_least '^version=[0-9A-Za-z.+_-]+$' 'version through image and status surfaces' 3
require_count_at_least '^target=aarch64-unknown-none$' 'target through image and status surfaces' 3
require_count_at_least '^selected_profile=shell-only$' 'selected profile through image and status surfaces' 3
require_count_at_least '^profile_request=shell-only$' 'profile request through image and status surfaces' 3
require_count_at_least '^launch_profile_feature=disabled$' 'launch-profile feature through image and status surfaces' 3
require_count_at_least '^profile=shell-only$' 'profile through status and profile surfaces' 3
require_count_at_least '^launch=disabled$' 'launch state through status and profile surfaces' 3
require_count_at_least '^payloads=0$' 'payload count through status and profile surfaces' 3
require_re '^input=usb-keyboard\+uart-fallback$' 'live USB keyboard input line'
require_re '^source=usb-keyboard\+uart-fallback$' 'live USB keyboard source line'
require_re '^source_state=ready$' 'live source readiness line'
require_re '^mode=live$' 'input live-mode line'
require_re '^input_mode=live$' 'status/profile live-mode line'
require_re '^usb_keyboard=ready$' 'live USB keyboard readiness line'
require_re '^usb_keyboard_probe=enabled$' 'USB keyboard probe-enabled line'
require_re '^usb_keyboard_poll_interval_ms=[1-9][0-9]*$' 'nonzero USB keyboard poll interval line'
require_re '^reovim-os> proof$' 'proof command in pasted transcript'
require_re '^reovim-os> cat /boot/proof$' 'VFS proof command in pasted transcript'
require_re '^reovim-os> help$' 'help command in pasted transcript'
require_re '^reovim-os> cat /boot/help$' 'VFS help command in pasted transcript'
require_count_at_least '^reovim root shell$' 'help root-shell banner through command and VFS' 2
require_re '^commands: help, clear, screentest, pwd, ls, cd, cat, mount, input, status, proof, device, dmesg, probe, launch, reovim, halt$' 'help command catalog'
require_count_at_least '^commands: help, clear, screentest, pwd, ls, cd, cat, mount, input, status, proof, device, dmesg, probe, launch, reovim, halt$' 'help command catalog through command and VFS' 2
require_count_at_least '^usage: help \[command\]$' 'help usage through command and VFS' 2
require_re '^reovim-os> clear$' 'clear command in pasted transcript'
require_re 'reovim-os> screentest$' 'screentest command in pasted transcript'
require_re '^screen test:$' 'screentest header'
require_re 'idx-fg:' 'screentest indexed-color row'
require_re 'rgb-bg:' 'screentest truecolor-background row'
require_re 'attrs:' 'screentest attribute row'
require_count_at_least '^reovim-os> pwd$' 'pwd commands in pasted transcript' 2
require_re '^/$' 'pwd root output'
require_re '^reovim-os> ls /$' 'root ls command in pasted transcript'
require_re '^boot$' 'root ls boot entry'
require_re '^dev$' 'root ls dev entry'
require_re '^log$' 'root ls log entry'
require_re '^reovim-os> ls /boot$' 'boot ls command in pasted transcript'
require_re '^devices$' 'boot ls devices entry'
require_re '^help$' 'boot ls help entry'
require_re '^image$' 'boot ls image entry'
require_re '^input$' 'boot ls input entry'
require_re '^mounts$' 'boot ls mounts entry'
require_re '^proof$' 'boot ls proof entry'
require_re '^profile$' 'boot ls profile entry'
require_re '^status$' 'boot ls status entry'
require_re '^reovim-os> ls /dev$' 'dev ls command in pasted transcript'
require_re '^uart0$' 'dev ls uart0 entry'
require_re '^reovim-os> mount$' 'mount command in pasted transcript'
require_re '^kernel on / type rootfs \(ro,pseudo\)$' 'mount rootfs row'
require_re '^boot on /boot type bootfs \(ro,pseudo\)$' 'mount bootfs row'
require_re '^devices on /dev type devfs \(ro,pseudo\)$' 'mount devfs row'
require_re '^klog on /log type logfs \(ro,pseudo\)$' 'mount logfs row'
require_re '^reovim-os> cat /boot/mounts$' 'VFS mounts command in pasted transcript'
require_re '^reovim-os> device$' 'device command in pasted transcript'
require_re '^boot_info:$' 'device boot-info header'
require_re '^devices:$' 'device inventory header'
require_re '^reovim-os> cat /boot/memory$' 'VFS memory command in pasted transcript'
require_re '^ranges=[1-9][0-9]*$' 'VFS memory ranges line'
require_re '^usable_bytes=[1-9][0-9]*$' 'VFS memory usable bytes line'
require_re '^cpu_count=[1-9][0-9]*$' 'VFS memory CPU count line'
require_re '^heap_total_bytes=[0-9]+$' 'VFS memory heap line'
require_re '^cache_line_bytes=[0-9]+$' 'VFS memory cache-line line'
require_re '^reovim-os> cat /boot/devices$' 'VFS devices command in pasted transcript'
require_re '^- \[[0-9]+\] uart compat=arm,pl011 mmio=0x[0-9a-f]+/0x[0-9a-f]+ irq=[0-9]+$' 'PL011 UART device inventory row'
require_re '^reovim-os> cd /dev$' 'cd dev command in pasted transcript'
require_re '^/dev$' 'pwd dev output'
require_re '^reovim-os> ls$' 'relative ls command in pasted transcript'
require_count_at_least '^uart0$' 'dev ls uart0 entry through absolute and relative ls' 2
require_re '^reovim-os> cat uart0$' 'relative UART device cat command'
require_re '^reovim-os> cd /$' 'cd root command in pasted transcript'
require_re '^reovim-os> cat /boot/image$' 'VFS image command in pasted transcript'
require_re '^reovim-os> status$' 'status command in pasted transcript'
require_re '^reovim-os> cat /boot/status$' 'VFS status command in pasted transcript'
require_re '^reovim-os> input$' 'input command in pasted transcript'
require_re '^reovim-os> cat /boot/input$' 'VFS input command in pasted transcript'
require_count_at_least '^proof:$' 'proof checklist header' 2
require_re '^commands:$' 'proof command-list header'
require_re '^  help$' 'proof checklist help command'
require_re '^  cat /boot/help$' 'proof checklist VFS help command'
require_re '^  clear$' 'proof checklist clear command'
require_re '^  screentest$' 'proof checklist screentest command'
require_count_at_least '^  pwd$' 'proof checklist pwd commands' 2
require_re '^  ls /$' 'proof checklist root ls command'
require_re '^  ls /boot$' 'proof checklist boot ls command'
require_re '^  ls /dev$' 'proof checklist dev ls command'
require_re '^  mount$' 'proof checklist mount command'
require_re '^  cat /boot/mounts$' 'proof checklist VFS mounts command'
require_re '^  device$' 'proof checklist device command'
require_re '^  cat /boot/memory$' 'proof checklist VFS memory command'
require_re '^  cat /boot/devices$' 'proof checklist VFS devices command'
require_re '^  cd /dev$' 'proof checklist cd dev command'
require_re '^  ls$' 'proof checklist relative ls command'
require_re '^  cat uart0$' 'proof checklist relative UART device command'
require_re '^  cd /$' 'proof checklist cd root command'
require_re '^  cat /boot/image$' 'proof checklist image command'
require_re '^  status$' 'proof checklist status command'
require_re '^  cat /boot/status$' 'proof checklist VFS status command'
require_re '^  input$' 'proof checklist input command'
require_re '^  cat /boot/input$' 'proof checklist VFS input command'
require_re '^  probe help$' 'proof checklist probe help command'
require_re '^  probe pcie$' 'proof checklist PCIe probe command'
require_re '^  probe usb-keyboard$' 'proof checklist USB keyboard command'
require_re '^  cat /boot/profile$' 'proof checklist profile command'
require_re '^  launch$' 'proof checklist launch command'
require_re '^  reovim$' 'proof checklist reovim command'
require_re '^  dmesg$' 'proof checklist dmesg command'
require_re '^  cat /log/dmesg$' 'proof checklist final log command'
require_re '^terminal:$' 'proof terminal-command header'
require_re '^  halt$' 'proof terminal halt command'
require_re '^expected:$' 'proof expected-facts header'
require_re '^  package=reovim-os$' 'proof expected package fact'
require_re '^  version=[0-9A-Za-z.+_-]+$' 'proof expected version fact'
require_re '^  target=aarch64-unknown-none$' 'proof expected target fact'
require_re '^  selected_profile=shell-only$' 'proof expected selected-profile fact'
require_re '^  profile_request=shell-only$' 'proof expected profile-request fact'
require_re '^  launch_profile_feature=disabled$' 'proof expected launch-profile-feature fact'
require_re '^  profile=shell-only$' 'proof expected profile fact'
require_re '^  launch=disabled$' 'proof expected launch fact'
require_re '^  payloads=0$' 'proof expected payload-count fact'
require_re '^  bootline=absent$' 'proof expected bootline fact'
require_re '^  source=usb-keyboard\+uart-fallback$' 'proof expected source fact'
require_re '^  source_state=ready$' 'proof expected source-state fact'
require_re '^  mode=live$' 'proof expected live-mode fact'
require_re '^  input_mode=live$' 'proof expected profile live-mode fact'
require_re '^  usb_keyboard=ready$' 'proof expected readiness fact'
require_re '^  usb_keyboard_probe=enabled$' 'proof expected probe-enabled fact'
require_re '^  shell\.status=ok$' 'proof expected shell status fact'
require_re '^  help catalog available through /boot/help$' 'proof expected VFS help fact'
require_re '^  probe targets include pcie$' 'proof expected PCIe probe fact'
require_re '^  probe targets include usb-keyboard$' 'proof expected USB keyboard probe fact'
require_re '^  probe targets include xhci-read-keyboard-report$' 'proof expected report probe fact'
require_re '^  launch/reovim disabled in shell-only profile$' 'proof expected shell-only launch-disabled fact'
require_re '^  shell\.status=error for disabled payload commands$' 'proof expected payload-disabled status fact'
require_re '^  halt typed last prints halt: ok and stops root daemon$' 'proof expected terminal halt fact'
require_re '^probe targets:$' 'probe help target catalog header'
require_re '^  pcie$' 'probe help PCIe target'
require_re '^  usb-keyboard \(alias: keyboard\)$' 'probe help USB keyboard target'
require_re '^  xhci-read-keyboard-report \(alias: usb-keyboard-read-report\)$' 'probe help keyboard report target'
require_re '^reovim-os> probe help$' 'probe help command in pasted transcript'
require_re '^reovim-os> probe pcie$' 'PCIe probe command in pasted transcript'
require_re '^probe pcie:$' 'PCIe probe output header'
require_re '^state=present$' 'PCIe probe present state'
require_re '^xhci=present$' 'PCIe xHCI present state'
require_re '^reovim-os> probe usb-keyboard$' 'USB keyboard probe command in pasted transcript'
require_re '^probe usb-keyboard:$' 'probe usb-keyboard output header'
require_re '^state=(report-ready|decoded-pending|report-pending)$' 'USB keyboard probe ready/pending state'
require_re '^reovim-os> cat /boot/profile$' 'VFS profile command in pasted transcript'
require_re '^reovim-os> launch$' 'launch command in pasted transcript'
require_re '^launch disabled for this profile$' 'shell-only launch disabled output'
require_re '^reovim-os> reovim$' 'reovim command in pasted transcript'
require_re '^reovim disabled for this profile$' 'shell-only reovim disabled output'
require_re '^reovim-os> dmesg$' 'dmesg command in pasted transcript'
require_re '^reovim-os> cat /log/dmesg$' 'VFS dmesg command in pasted transcript'
require_re '^reovim-os> halt$' 'terminal halt command in pasted transcript'
require_re '^halt: ok$' 'terminal halt output'

require_re '^shell: cat /boot/image$' 'dmesg audit for cat /boot/image'
require_re '^shell: proof$' 'dmesg audit for proof'
require_re '^shell: cat /boot/proof$' 'dmesg audit for VFS proof'
require_re '^shell: help$' 'dmesg audit for help'
require_re '^shell: cat /boot/help$' 'dmesg audit for VFS help'
require_re '^shell: clear$' 'dmesg audit for clear'
require_re '^shell: screentest$' 'dmesg audit for screentest'
require_re '^shell: pwd$' 'dmesg audit for pwd'
require_re '^shell: ls /$' 'dmesg audit for root ls'
require_re '^shell: ls /boot$' 'dmesg audit for boot ls'
require_re '^shell: ls /dev$' 'dmesg audit for dev ls'
require_re '^shell: mount$' 'dmesg audit for mount'
require_re '^shell: cat /boot/mounts$' 'dmesg audit for VFS mounts'
require_re '^shell: device$' 'dmesg audit for device inventory'
require_re '^shell: cat /boot/memory$' 'dmesg audit for VFS memory'
require_re '^shell: cat /boot/devices$' 'dmesg audit for VFS devices'
require_re '^shell: cd /dev$' 'dmesg audit for cd dev'
require_re '^shell: ls$' 'dmesg audit for relative ls'
require_re '^shell: cat uart0$' 'dmesg audit for relative UART device'
require_re '^shell: cd /$' 'dmesg audit for cd root'
require_re '^shell: status$' 'dmesg audit for status'
require_re '^shell: cat /boot/status$' 'dmesg audit for VFS status'
require_re '^shell: input$' 'dmesg audit for input'
require_re '^shell: cat /boot/input$' 'dmesg audit for VFS input'
require_re '^shell: probe help$' 'dmesg audit for probe help'
require_re '^shell: probe pcie$' 'dmesg audit for probe pcie'
require_re '^shell: probe usb-keyboard$' 'dmesg audit for probe usb-keyboard'
require_re '^shell: cat /boot/profile$' 'dmesg audit for cat /boot/profile'
require_re '^shell: launch$' 'dmesg audit for launch'
require_re '^shell: reovim$' 'dmesg audit for reovim'
require_re '^shell: dmesg$' 'dmesg audit for dmesg'
require_re '^shell: cat /log/dmesg$' 'dmesg audit for cat /log/dmesg'
require_count_at_least '^shell\.status=ok$' 'successful shell status audit lines' 30
require_count_at_least '^shell\.status=error$' 'disabled payload error audit lines' 2

if awk '
    /^halt: ok$/ { seen_halt = 1; next }
    seen_halt && /^reovim-os>/ { found_prompt = 1 }
    END { exit found_prompt ? 0 : 1 }
' "$EVIDENCE"; then
    printf 'invalid: terminal halt must not print another root shell prompt\n' >&2
    missing=1
fi

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
