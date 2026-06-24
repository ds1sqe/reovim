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

require_usb_source_for_shell_command() {
    local command="$1"
    local label="$2"
    if ! awk -v command="$command" '
        /^input\.line_source=usb-keyboard usb_bytes=[1-9][0-9]* fallback_bytes=0 line_bytes=[1-9][0-9]*$/ {
            usb_line = 1
            next
        }
        $0 == "shell: " command {
            if (usb_line) {
                found = 1
            }
            usb_line = 0
            next
        }
        {
            usb_line = 0
        }
        END { exit found ? 0 : 1 }
    ' "$EVIDENCE"; then
        printf 'missing: USB-keyboard line-source immediately before %s\n' "$label" >&2
        missing=1
    fi
}

require_probe_usb_keyboard_manual_next() {
    if ! awk '
        /^probe usb-keyboard:$/ { in_probe = 1; next }
        in_probe && /^manual_next=type-shell-command$/ { found = 1; in_probe = 0; next }
        in_probe && /^reovim-os>/ { in_probe = 0 }
        END { exit found ? 0 : 1 }
    ' "$EVIDENCE"; then
        printf 'missing: probe usb-keyboard manual_next type-command line\n' >&2
        missing=1
    fi
}

require_retained_probe_usb_keyboard_manual_next() {
    if ! awk '
        /^reovim-os> dmesg$/ { in_log = 1; next }
        /^reovim-os> cat \/log\/dmesg$/ { in_log = 1; next }
        in_log && /^```$/ { in_log = 0; in_probe = 0; next }
        in_log && /^probe usb-keyboard:$/ { in_probe = 1; next }
        in_log && in_probe && /^manual_next=type-shell-command$/ {
            found = 1
            in_probe = 0
            next
        }
        in_log && in_probe && /^shell: / { in_probe = 0 }
        END { exit found ? 0 : 1 }
    ' "$EVIDENCE"; then
        printf 'missing: retained dmesg probe usb-keyboard manual_next output\n' >&2
        missing=1
    fi
}

require_count_exact() {
    local pattern="$1"
    local label="$2"
    local expected="$3"
    local count

    count="$(grep -Ec -- "$pattern" "$EVIDENCE" || true)"
    if [ "$count" -ne "$expected" ]; then
        printf 'invalid: %s (found %s, need %s)\n' "$label" "$count" "$expected" >&2
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
forbid_re '^- Preflight warning:' 'preflight warning row remains in completed evidence'
forbid_re '<final-evidence-file>' 'placeholder final evidence path remains'
forbid_re '--skip-qemu' 'skipped-QEMU command argument remains'

require_re '^- Image bytes: [1-9][0-9]*$' 'seeded image byte count'
require_re '^- Image SHA-256: [0-9a-f]{64}$' 'seeded image SHA-256'
require_re '^- Media preparation command: apps/os/targets/raspi4b-aarch64/prepare-boot-media\.sh --bootfs .+ --evidence .+$' 'actual media-preparation command'
require_re '^- Boot partition path: .+$' 'boot partition path'
require_re '^- Preflight result: preflight=ok qemu_smoke=passed$' 'preflight QEMU proof smoke passed'
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
require_re '^- \[[xX]\] `usb_keyboard_last_poll=report-ready`$' 'USB keyboard lower report-ready checkbox'
require_re '^- \[[xX]\] `dmesg` contains `input\.usb_keyboard=ready source=usb-keyboard\+uart-fallback last_poll=report-ready`\.$' 'USB keyboard readiness dmesg checkbox'
require_re '^- \[[xX]\] `dmesg` pairs `input\.line_source=usb-keyboard` with `fallback_bytes=0` immediately before typed command audit lines\.$' 'USB keyboard line-source dmesg checkbox'
require_re '^- \[[xX]\] `manual_next=type-shell-command`$' 'manual next type-command checkbox'
require_re '^- \[[xX]\] `probe usb-keyboard` reports `manual_next=type-shell-command`\.$' 'probe manual next type-command checkbox'
require_re '^- \[[xX]\] `status` / `cat /boot/status` report image/profile identity and live ready source diagnostics\.$' 'status identity/live diagnostics checkbox'
require_re '^- \[[xX]\] `input` / `cat /boot/input` report live input diagnostics\.$' 'input live diagnostics checkbox'
require_re '^- \[[xX]\] `cat /boot/probes` prints the VFS-backed probe catalog\.$' 'VFS probe catalog checkbox'
require_re '^- \[[xX]\] `probe help` lists `pcie`, `usb-keyboard`, and `xhci-read-keyboard-report`\.$' 'probe help catalog checkbox'
require_re '^- \[[xX]\] `probe pcie` reports the read-only PCIe/xHCI state\.$' 'PCIe probe checkbox'
require_re '^- \[[xX]\] `cat /boot/profile` reports `profile=shell-only`, `launch=disabled`, `payloads=0`, and `input_mode=live`\.$' 'profile identity checkbox'
require_re '^- \[[xX]\] `launch` / `reovim` report shell-only payload launch disabled\.$' 'shell-only launch-disabled checkbox'
require_re '^- \[[xX]\] `dmesg --stats` / `cat /log/stats` report kernel log ring stats with `dropped_bytes=0`\.$' 'klog stats checkbox'
require_re '^- \[[xX]\] `dmesg` has no `\[klog\] dropped_bytes=` retained-log wrap marker\.$' 'retained dmesg no-wrap-marker checkbox'
require_re '^- \[[xX]\] `dmesg` contains `shell: <command>` and `shell\.status=ok` audit lines for the typed commands\.$' 'dmesg command/status audit checkbox'
require_re '^- \[[xX]\] `dmesg` contains `shell\.status=error` audit lines for the disabled payload launch commands\.$' 'dmesg payload-disabled error audit checkbox'
require_re '^- \[[xX]\] `dmesg` contains the retained `probe usb-keyboard` output with `manual_next=type-shell-command`\.$' 'retained probe-output dmesg checkbox'
require_re '^- \[[xX]\] `halt` was typed last and printed `halt: ok`\.$' 'terminal halt checkbox'
require_re '^- \[[xX]\] No new `reovim-os>` prompt appeared after `halt: ok`\.$' 'terminal halt no-prompt checkbox'
require_re '^- \[[xX]\] `proof` prints the physical input proof checklist\.$' 'proof checklist checkbox'
require_re '^- \[[xX]\] `cat /boot/proof` prints the VFS-backed proof pseudo-file\.$' 'VFS proof pseudo-file checkbox'
require_re '^- \[[xX]\] `cat /boot/help` prints the VFS-backed detailed help catalog\.$' 'VFS help pseudo-file checkbox'
require_re '^- \[[xX]\] `help dmesg` / `ls /log` make kernel log stats discoverable\.$' 'klog stats discovery checkbox'
require_re '^- \[[xX]\] `help input` / `help proof` make live input diagnostics and the proof checklist self-describing\.$' 'targeted input/proof help checkbox'
require_re '^- \[[xX]\] `help status` / `help probe` make `manual_next` and probe catalog discovery self-describing\.$' 'targeted status/probe help checkbox'
require_re '^- \[[xX]\] `help pwd` / `help ls` / `help cd` / `help cat` / `help mount` make VFS navigation self-describing\.$' 'targeted VFS help checkbox'
require_re '^- \[[xX]\] `help` / `help clear` / `help screentest` / `clear` / `screentest` prove shell help and erase-line mode diagnostics\.$' 'shell help and renderer checkbox'
require_re '^- \[[xX]\] `help device` / `help launch` / `help reovim` / `help halt` make boot inventory, payload, and shutdown commands self-describing\.$' 'remaining command help checkbox'
require_re '^- \[[xX]\] `pwd` / `ls` / `mount` report the kernel VFS namespace and mounts\.$' 'VFS namespace and mounts checkbox'
require_re '^- \[[xX]\] `device` / `cat /boot/memory` / `cat /boot/devices` report boot inventory\.$' 'boot inventory checkbox'
require_re '^- \[[xX]\] `cd /dev` / `ls` / `cat uart0` prove relative VFS device-file access\.$' 'relative device-file checkbox'

require_re '^reovim-os>' 'root shell prompt in pasted transcript'
require_re '^media_prepare=ok$' 'media-prep result line'
require_re '^bootfs=.+$' 'media-prep bootfs path line'
require_re '^installed=.+/kernel8\.img$' 'installed kernel8.img path line'
require_re '^bytes=[1-9][0-9]*$' 'installed kernel8.img byte count line'
require_re '^sha256=[0-9a-f]{64}$' 'installed kernel8.img SHA-256 line'
require_count_exact '^- Preflight result:' 'single preflight result line' 1
require_count_exact '^- Media preparation command:' 'single media-preparation command line' 1
require_count_exact '^- Boot partition path:' 'single boot partition path line' 1
require_count_exact '^media_prepare=' 'single media-prep result line' 1
require_count_exact '^bootfs=' 'single media-prep bootfs path line' 1
require_count_exact '^installed=' 'single installed kernel8.img path line' 1
require_count_exact '^- Image bytes:' 'single seeded image byte-count line' 1
require_count_exact '^- Image SHA-256:' 'single seeded image SHA-256 line' 1
require_count_exact '^bytes=' 'single installed image byte-count line' 1
require_count_exact '^sha256=' 'single installed image SHA-256 line' 1
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
require_re '^usb_keyboard_last_poll=report-ready$' 'lower USB keyboard report-ready line'
require_re '^input\.usb_keyboard=ready source=usb-keyboard\+uart-fallback last_poll=report-ready$' 'dmesg USB keyboard readiness transition'
require_re '^manual_next=type-shell-command$' 'manual next type-command line'
require_re '^reovim-os> proof$' 'proof command in pasted transcript'
require_re '^reovim-os> cat /boot/proof$' 'VFS proof command in pasted transcript'
require_re '^reovim-os> help$' 'help command in pasted transcript'
require_re '^reovim-os> help clear$' 'targeted clear help command in pasted transcript'
require_re '^clear - clear framebuffer console and terminal$' 'targeted clear help output'
require_re '^reovim-os> help screentest$' 'targeted screentest help command in pasted transcript'
require_re '^screentest - print renderer diagnostics$' 'targeted screentest help output'
require_re '^  required rows: el: clean, el1: clean-left, el2: clean-all$' 'targeted screentest required rows output'
require_re '^reovim-os> help input$' 'targeted input help command in pasted transcript'
require_re '^input - print live console input diagnostics$' 'targeted input help output'
require_re '^reovim-os> help proof$' 'targeted proof help command in pasted transcript'
require_re '^proof - print physical input proof checklist$' 'targeted proof help output'
require_re '^reovim-os> help pwd$' 'targeted pwd help command in pasted transcript'
require_re '^pwd - print current kernel VFS directory$' 'targeted pwd help output'
require_re '^reovim-os> help ls$' 'targeted ls help command in pasted transcript'
require_re '^ls \[path\] - list a kernel VFS directory$' 'targeted ls help output'
require_re '^reovim-os> help cd$' 'targeted cd help command in pasted transcript'
require_re '^cd \[path\] - change current kernel VFS directory$' 'targeted cd help output'
require_re '^reovim-os> help cat$' 'targeted cat help command in pasted transcript'
require_re '^cat path\.\.\. - print kernel VFS pseudo files$' 'targeted cat help output'
require_re '^reovim-os> help mount$' 'targeted mount help command in pasted transcript'
require_re '^mount - print kernel VFS mount table$' 'targeted mount help output'
require_re '^reovim-os> help device$' 'targeted device help command in pasted transcript'
require_re '^device - print boot memory and device inventory$' 'targeted device help output'
require_re '^reovim-os> help dmesg$' 'targeted dmesg help command in pasted transcript'
require_re '^dmesg \[--stats\] - print retained kernel log or ring stats$' 'targeted dmesg help output'
require_re '^reovim-os> help status$' 'targeted status help command in pasted transcript'
require_re '^status - print boot, input, and manual_next summary$' 'targeted status help output'
require_re '^reovim-os> help probe$' 'targeted probe help command in pasted transcript'
require_re '^probe target - run lower hardware probe; try `probe help` or `cat /boot/probes`$' 'targeted probe help output'
require_re '^reovim-os> help launch$' 'targeted launch help command in pasted transcript'
require_re '^launch \[payload\] - list or run registered payloads$' 'targeted launch help output'
require_re '^reovim-os> help reovim$' 'targeted reovim help command in pasted transcript'
require_re '^reovim - run the default reovim payload alias$' 'targeted reovim help output'
require_re '^reovim-os> help halt$' 'targeted halt help command in pasted transcript'
require_re '^halt - request root daemon shutdown$' 'targeted halt help output'
require_re '^reovim-os> cat /boot/help$' 'VFS help command in pasted transcript'
require_count_at_least '^reovim root shell$' 'help root-shell banner through command and VFS' 2
require_re '^commands: help, clear, screentest, pwd, ls, cd, cat, mount, input, status, proof, device, dmesg, probe, launch, reovim, halt$' 'help command catalog'
require_count_at_least '^commands: help, clear, screentest, pwd, ls, cd, cat, mount, input, status, proof, device, dmesg, probe, launch, reovim, halt$' 'help command catalog through command and VFS' 2
require_count_at_least '^usage: help \[command\]$' 'help usage through command and VFS' 2
require_re '^details:$' 'VFS help detailed catalog header'
require_re '^  help \[command\] - show command help$' 'VFS help detailed help row'
require_re '^  clear - clear framebuffer console and terminal$' 'VFS help detailed clear row'
require_re '^  screentest - print renderer diagnostics$' 'VFS help detailed screentest row'
require_re '^    required rows: el: clean, el1: clean-left, el2: clean-all$' 'VFS help detailed screentest rows'
require_re '^  pwd - print current kernel VFS directory$' 'VFS help detailed pwd row'
require_re '^  ls \[path\] - list a kernel VFS directory$' 'VFS help detailed ls row'
require_re '^  cd \[path\] - change current kernel VFS directory$' 'VFS help detailed cd row'
require_re '^  cat path\.\.\. - print kernel VFS pseudo files$' 'VFS help detailed cat row'
require_re '^  mount - print kernel VFS mount table$' 'VFS help detailed mount row'
require_re '^  input - print live console input diagnostics$' 'VFS help detailed input row'
require_re '^  status - print boot, input, and manual_next summary$' 'VFS help detailed status row'
require_re '^  proof - print physical input proof checklist$' 'VFS help detailed proof row'
require_re '^  device - print boot memory and device inventory$' 'VFS help detailed device row'
require_re '^  dmesg \[--stats\] - print retained kernel log or ring stats$' 'VFS help detailed dmesg row'
require_re '^  probe target - run lower hardware probe; try `probe help` or `cat /boot/probes`$' 'VFS help detailed probe row'
require_re '^  launch \[payload\] - list or run registered payloads$' 'VFS help detailed launch row'
require_re '^  reovim - run the default reovim payload alias$' 'VFS help detailed reovim row'
require_re '^  halt - request root daemon shutdown$' 'VFS help detailed halt row'
require_re '^reovim-os> clear$' 'clear command in pasted transcript'
require_re 'reovim-os> screentest$' 'screentest command in pasted transcript'
require_re '^screen test:$' 'screentest header'
require_re 'idx-fg:' 'screentest indexed-color row'
require_re 'rgb-bg:' 'screentest truecolor-background row'
require_re 'attrs:' 'screentest attribute row'
require_re '^  el: clean$' 'screentest erase-line row'
require_re '^  el1: clean-left$' 'screentest erase-to-start row'
require_re '^  el2: clean-all$' 'screentest erase-all row'
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
require_re '^memory$' 'boot ls memory entry'
require_re '^mounts$' 'boot ls mounts entry'
require_re '^probes$' 'boot ls probes entry'
require_re '^proof$' 'boot ls proof entry'
require_re '^profile$' 'boot ls profile entry'
require_re '^status$' 'boot ls status entry'
require_re '^reovim-os> ls /dev$' 'dev ls command in pasted transcript'
require_re '^uart0$' 'dev ls uart0 entry'
require_re '^reovim-os> ls /log$' 'log ls command in pasted transcript'
require_re '^dmesg$' 'log ls dmesg entry'
require_re '^stats$' 'log ls stats entry'
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
require_count_at_least '^  proof$' 'proof checklist self command' 2
require_count_at_least '^  cat /boot/proof$' 'proof checklist VFS proof command' 2
require_re '^  help$' 'proof checklist help command'
require_re '^  help clear$' 'proof checklist targeted clear help command'
require_re '^  help screentest$' 'proof checklist targeted screentest help command'
require_re '^  help input$' 'proof checklist targeted input help command'
require_re '^  help proof$' 'proof checklist targeted proof help command'
require_re '^  help pwd$' 'proof checklist targeted pwd help command'
require_re '^  help ls$' 'proof checklist targeted ls help command'
require_re '^  help cd$' 'proof checklist targeted cd help command'
require_re '^  help cat$' 'proof checklist targeted cat help command'
require_re '^  help mount$' 'proof checklist targeted mount help command'
require_re '^  help device$' 'proof checklist targeted device help command'
require_re '^  help dmesg$' 'proof checklist targeted dmesg help command'
require_re '^  help status$' 'proof checklist targeted status help command'
require_re '^  help probe$' 'proof checklist targeted probe help command'
require_re '^  help launch$' 'proof checklist targeted launch help command'
require_re '^  help reovim$' 'proof checklist targeted reovim help command'
require_re '^  help halt$' 'proof checklist targeted halt help command'
require_re '^  cat /boot/help$' 'proof checklist VFS help command'
require_re '^  clear$' 'proof checklist clear command'
require_re '^  screentest$' 'proof checklist screentest command'
require_count_at_least '^  pwd$' 'proof checklist pwd commands' 2
require_re '^  ls /$' 'proof checklist root ls command'
require_re '^  ls /boot$' 'proof checklist boot ls command'
require_re '^  ls /dev$' 'proof checklist dev ls command'
require_re '^  ls /log$' 'proof checklist log ls command'
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
require_re '^  cat /boot/probes$' 'proof checklist VFS probe catalog command'
require_re '^  probe pcie$' 'proof checklist PCIe probe command'
require_re '^  probe usb-keyboard$' 'proof checklist USB keyboard command'
require_re '^  cat /boot/profile$' 'proof checklist profile command'
require_re '^  launch$' 'proof checklist launch command'
require_re '^  reovim$' 'proof checklist reovim command'
require_re '^  dmesg --stats$' 'proof checklist dmesg stats command'
require_re '^  cat /log/stats$' 'proof checklist VFS log stats command'
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
require_re '^  usb_keyboard_last_poll=report-ready$' 'proof expected report-ready fact'
require_re '^  dmesg contains input\.usb_keyboard=ready source=usb-keyboard\+uart-fallback last_poll=report-ready$' 'proof expected readiness dmesg fact'
require_re '^  dmesg pairs input\.line_source=usb-keyboard usb_bytes>0 fallback_bytes=0 line_bytes>0 before shell:<command>$' 'proof expected line-source dmesg fact'
require_re '^  manual_next=type-shell-command$' 'proof expected manual next fact'
require_re '^  probe usb-keyboard reports manual_next=type-shell-command$' 'proof expected probe manual next fact'
require_re '^  shell\.status=ok$' 'proof expected shell status fact'
require_re '^  detailed help catalog available through /boot/help$' 'proof expected VFS help fact'
require_re '^  screentest includes erase-line mode diagnostics$' 'proof expected screentest erase-mode fact'
require_re '^  kernel log stats available through /log/stats$' 'proof expected log stats fact'
require_re '^  kernel log dropped_bytes=0$' 'proof expected zero dropped log bytes fact'
require_re '^  retained dmesg has no \[klog\] dropped_bytes marker$' 'proof expected retained dmesg no-wrap-marker fact'
require_re '^  retained dmesg includes probe usb-keyboard manual_next output$' 'proof expected retained probe output fact'
require_re '^  probe catalog available through /boot/probes$' 'proof expected VFS probe catalog fact'
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
require_re '^reovim-os> cat /boot/probes$' 'VFS probe catalog command in pasted transcript'
require_re '^reovim-os> probe pcie$' 'PCIe probe command in pasted transcript'
require_re '^probe pcie:$' 'PCIe probe output header'
require_re '^state=present$' 'PCIe probe present state'
require_re '^xhci=present$' 'PCIe xHCI present state'
require_re '^reovim-os> probe usb-keyboard$' 'USB keyboard probe command in pasted transcript'
require_re '^probe usb-keyboard:$' 'probe usb-keyboard output header'
require_re '^state=(report-ready|decoded-pending|report-pending)$' 'USB keyboard probe ready/pending state'
require_probe_usb_keyboard_manual_next
require_re '^reovim-os> cat /boot/profile$' 'VFS profile command in pasted transcript'
require_re '^reovim-os> launch$' 'launch command in pasted transcript'
require_re '^launch disabled for this profile$' 'shell-only launch disabled output'
require_re '^reovim-os> reovim$' 'reovim command in pasted transcript'
require_re '^reovim disabled for this profile$' 'shell-only reovim disabled output'
require_re '^reovim-os> dmesg --stats$' 'dmesg stats command in pasted transcript'
require_re '^reovim-os> cat /log/stats$' 'VFS log stats command in pasted transcript'
require_re '^capacity_bytes=[1-9][0-9]*$' 'klog stats capacity line'
require_re '^retained_bytes=[1-9][0-9]*$' 'klog stats retained-bytes line'
require_re '^dropped_bytes=0$' 'klog stats zero dropped-bytes line'
forbid_re '^dropped_bytes=[1-9][0-9]*$' 'nonzero klog dropped bytes'
forbid_re '^\[klog\] dropped_bytes=[1-9][0-9]*$' 'wrapped dmesg dropped bytes marker'
require_re '^reovim-os> dmesg$' 'dmesg command in pasted transcript'
require_re '^reovim-os> cat /log/dmesg$' 'VFS dmesg command in pasted transcript'
require_retained_probe_usb_keyboard_manual_next
require_re '^reovim-os> halt$' 'terminal halt command in pasted transcript'
require_re '^halt: ok$' 'terminal halt output'

require_count_at_least '^input\.line_source=usb-keyboard usb_bytes=[1-9][0-9]* fallback_bytes=0 line_bytes=[1-9][0-9]*$' 'USB-keyboard line-source audit lines' 30

require_usb_source_for_shell_command 'cat /boot/image' 'shell: cat /boot/image'
require_usb_source_for_shell_command 'proof' 'shell: proof'
require_usb_source_for_shell_command 'cat /boot/proof' 'shell: cat /boot/proof'
require_usb_source_for_shell_command 'help' 'shell: help'
require_usb_source_for_shell_command 'help clear' 'shell: help clear'
require_usb_source_for_shell_command 'help screentest' 'shell: help screentest'
require_usb_source_for_shell_command 'help input' 'shell: help input'
require_usb_source_for_shell_command 'help proof' 'shell: help proof'
require_usb_source_for_shell_command 'help pwd' 'shell: help pwd'
require_usb_source_for_shell_command 'help ls' 'shell: help ls'
require_usb_source_for_shell_command 'help cd' 'shell: help cd'
require_usb_source_for_shell_command 'help cat' 'shell: help cat'
require_usb_source_for_shell_command 'help mount' 'shell: help mount'
require_usb_source_for_shell_command 'help device' 'shell: help device'
require_usb_source_for_shell_command 'help dmesg' 'shell: help dmesg'
require_usb_source_for_shell_command 'cat /boot/help' 'shell: cat /boot/help'
require_usb_source_for_shell_command 'clear' 'shell: clear'
require_usb_source_for_shell_command 'screentest' 'shell: screentest'
require_usb_source_for_shell_command 'pwd' 'shell: pwd'
require_usb_source_for_shell_command 'ls /' 'shell: ls /'
require_usb_source_for_shell_command 'ls /boot' 'shell: ls /boot'
require_usb_source_for_shell_command 'ls /dev' 'shell: ls /dev'
require_usb_source_for_shell_command 'ls /log' 'shell: ls /log'
require_usb_source_for_shell_command 'mount' 'shell: mount'
require_usb_source_for_shell_command 'cat /boot/mounts' 'shell: cat /boot/mounts'
require_usb_source_for_shell_command 'device' 'shell: device'
require_usb_source_for_shell_command 'cat /boot/memory' 'shell: cat /boot/memory'
require_usb_source_for_shell_command 'cat /boot/devices' 'shell: cat /boot/devices'
require_usb_source_for_shell_command 'cd /dev' 'shell: cd /dev'
require_usb_source_for_shell_command 'ls' 'shell: ls'
require_usb_source_for_shell_command 'cat uart0' 'shell: cat uart0'
require_usb_source_for_shell_command 'cd /' 'shell: cd /'
require_usb_source_for_shell_command 'help status' 'shell: help status'
require_usb_source_for_shell_command 'help probe' 'shell: help probe'
require_usb_source_for_shell_command 'help launch' 'shell: help launch'
require_usb_source_for_shell_command 'help reovim' 'shell: help reovim'
require_usb_source_for_shell_command 'help halt' 'shell: help halt'
require_usb_source_for_shell_command 'status' 'shell: status'
require_usb_source_for_shell_command 'cat /boot/status' 'shell: cat /boot/status'
require_usb_source_for_shell_command 'input' 'shell: input'
require_usb_source_for_shell_command 'cat /boot/input' 'shell: cat /boot/input'
require_usb_source_for_shell_command 'probe help' 'shell: probe help'
require_usb_source_for_shell_command 'cat /boot/probes' 'shell: cat /boot/probes'
require_usb_source_for_shell_command 'probe pcie' 'shell: probe pcie'
require_usb_source_for_shell_command 'probe usb-keyboard' 'shell: probe usb-keyboard'
require_usb_source_for_shell_command 'cat /boot/profile' 'shell: cat /boot/profile'
require_usb_source_for_shell_command 'launch' 'shell: launch'
require_usb_source_for_shell_command 'reovim' 'shell: reovim'
require_usb_source_for_shell_command 'dmesg --stats' 'shell: dmesg --stats'
require_usb_source_for_shell_command 'cat /log/stats' 'shell: cat /log/stats'
require_usb_source_for_shell_command 'dmesg' 'shell: dmesg'
require_usb_source_for_shell_command 'cat /log/dmesg' 'shell: cat /log/dmesg'

require_re '^shell: cat /boot/image$' 'dmesg audit for cat /boot/image'
require_re '^shell: proof$' 'dmesg audit for proof'
require_re '^shell: cat /boot/proof$' 'dmesg audit for VFS proof'
require_re '^shell: help$' 'dmesg audit for help'
require_re '^shell: help clear$' 'dmesg audit for targeted clear help'
require_re '^shell: help screentest$' 'dmesg audit for targeted screentest help'
require_re '^shell: help input$' 'dmesg audit for targeted input help'
require_re '^shell: help proof$' 'dmesg audit for targeted proof help'
require_re '^shell: help pwd$' 'dmesg audit for targeted pwd help'
require_re '^shell: help ls$' 'dmesg audit for targeted ls help'
require_re '^shell: help cd$' 'dmesg audit for targeted cd help'
require_re '^shell: help cat$' 'dmesg audit for targeted cat help'
require_re '^shell: help mount$' 'dmesg audit for targeted mount help'
require_re '^shell: help device$' 'dmesg audit for targeted device help'
require_re '^shell: help dmesg$' 'dmesg audit for targeted dmesg help'
require_re '^shell: cat /boot/help$' 'dmesg audit for VFS help'
require_re '^shell: clear$' 'dmesg audit for clear'
require_re '^shell: screentest$' 'dmesg audit for screentest'
require_re '^shell: pwd$' 'dmesg audit for pwd'
require_re '^shell: ls /$' 'dmesg audit for root ls'
require_re '^shell: ls /boot$' 'dmesg audit for boot ls'
require_re '^shell: ls /dev$' 'dmesg audit for dev ls'
require_re '^shell: ls /log$' 'dmesg audit for log ls'
require_re '^shell: mount$' 'dmesg audit for mount'
require_re '^shell: cat /boot/mounts$' 'dmesg audit for VFS mounts'
require_re '^shell: device$' 'dmesg audit for device inventory'
require_re '^shell: cat /boot/memory$' 'dmesg audit for VFS memory'
require_re '^shell: cat /boot/devices$' 'dmesg audit for VFS devices'
require_re '^shell: cd /dev$' 'dmesg audit for cd dev'
require_re '^shell: ls$' 'dmesg audit for relative ls'
require_re '^shell: cat uart0$' 'dmesg audit for relative UART device'
require_re '^shell: cd /$' 'dmesg audit for cd root'
require_re '^shell: help status$' 'dmesg audit for targeted status help'
require_re '^shell: help probe$' 'dmesg audit for targeted probe help'
require_re '^shell: help launch$' 'dmesg audit for targeted launch help'
require_re '^shell: help reovim$' 'dmesg audit for targeted reovim help'
require_re '^shell: help halt$' 'dmesg audit for targeted halt help'
require_re '^shell: status$' 'dmesg audit for status'
require_re '^shell: cat /boot/status$' 'dmesg audit for VFS status'
require_re '^shell: input$' 'dmesg audit for input'
require_re '^shell: cat /boot/input$' 'dmesg audit for VFS input'
require_re '^shell: probe help$' 'dmesg audit for probe help'
require_re '^shell: cat /boot/probes$' 'dmesg audit for VFS probe catalog'
require_re '^shell: probe pcie$' 'dmesg audit for probe pcie'
require_re '^shell: probe usb-keyboard$' 'dmesg audit for probe usb-keyboard'
require_re '^shell: cat /boot/profile$' 'dmesg audit for cat /boot/profile'
require_re '^shell: launch$' 'dmesg audit for launch'
require_re '^shell: reovim$' 'dmesg audit for reovim'
require_re '^shell: dmesg --stats$' 'dmesg audit for dmesg stats'
require_re '^shell: cat /log/stats$' 'dmesg audit for VFS log stats'
require_re '^shell: dmesg$' 'dmesg audit for dmesg'
require_re '^shell: cat /log/dmesg$' 'dmesg audit for cat /log/dmesg'
if ! awk '
    /^shell: dmesg$/ { state = 1; next }
    state == 1 && /^shell\.status=ok$/ { state = 2; next }
    state == 1 { state = 0 }
    state == 2 && /^input\.line_source=usb-keyboard usb_bytes=[1-9][0-9]* fallback_bytes=0 line_bytes=[1-9][0-9]*$/ { state = 3; next }
    state == 2 && /^shell: cat \/log\/dmesg$/ { found = 1; next }
    state == 2 { state = 0 }
    state == 3 && /^shell: cat \/log\/dmesg$/ { found = 1; next }
    state == 3 { state = 0 }
    END { exit found ? 0 : 1 }
' "$EVIDENCE"; then
    printf 'missing: final log read preserves dmesg status before its own audit\n' >&2
    missing=1
fi
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

media_command_bootfs="$(
    awk '
        /^- Media preparation command: apps\/os\/targets\/raspi4b-aarch64\/prepare-boot-media\.sh / {
            for (i = 1; i <= NF; i++) {
                if ($i == "--bootfs") {
                    print $(i + 1)
                    exit
                }
            }
        }
    ' "$EVIDENCE"
)"
session_bootfs="$(awk -F': ' '/^- Boot partition path: .+$/ { print $2; exit }' "$EVIDENCE")"
media_bootfs="$(awk -F= '/^bootfs=.+$/ { print $2; exit }' "$EVIDENCE")"
installed_path="$(awk -F= '/^installed=.+\/kernel8\.img$/ { print $2; exit }' "$EVIDENCE")"
if [ -n "$media_command_bootfs" ] && [ -n "$media_bootfs" ] && [ "$media_command_bootfs" != "$media_bootfs" ]; then
    printf 'mismatch: media command bootfs %s != media result bootfs %s\n' "$media_command_bootfs" "$media_bootfs" >&2
    missing=1
fi
if [ -n "$session_bootfs" ] && [ -n "$media_bootfs" ] && [ "$session_bootfs" != "$media_bootfs" ]; then
    printf 'mismatch: session boot partition path %s != media result bootfs %s\n' "$session_bootfs" "$media_bootfs" >&2
    missing=1
fi
if [ -n "$media_bootfs" ] && [ -n "$installed_path" ]; then
    case "$media_bootfs" in
        */) expected_installed="${media_bootfs}kernel8.img" ;;
        *) expected_installed="$media_bootfs/kernel8.img" ;;
    esac
    if [ "$installed_path" != "$expected_installed" ]; then
        printf 'mismatch: installed path %s != expected %s\n' "$installed_path" "$expected_installed" >&2
        missing=1
    fi
fi

if [ "$missing" -ne 0 ]; then
    exit 1
fi

printf 'evidence=pass\n'
printf 'file=%s\n' "$EVIDENCE"
