# Raspberry Pi 4 USB Keyboard Evidence Template

Use this template for the #800 real-machine proof. The proof is incomplete
unless HDMI output is observed, input source is `physical USB keyboard`, and
the shell reports `usb_keyboard=ready` plus
`usb_keyboard_last_poll=report-ready`.

## Session

- Date:
- Operator:
- Board:
- Image path:
- Image bytes:
- Image SHA-256:
- Image build command:
- Image install command:
- Media preparation command:
- Boot partition path:
- Preflight command:
- Preflight result:
- Evidence label:
  - [ ] display-only
  - [ ] UART input
  - [ ] bootline-script
  - [ ] physical USB keyboard
- [ ] HDMI display attached before boot.
- [ ] Physical USB keyboard attached before boot.
- [ ] `REOVIM_OS_BOOTLINE` unset on booted image.
- UART serial console attached:

## Media Preparation

Required media facts:

- [ ] `media_prepare=ok`
- [ ] installed `kernel8.img` SHA-256 matches image SHA-256.
- [ ] installed `kernel8.img` byte count matches image byte count.

Paste the media-prep result from `prepare-boot-media.sh`:

```text

```

## Boot Evidence

Paste the boot report or serial transcript excerpt:

```text

```

Required facts:

- [ ] `package=reovim-os`
- [ ] `/boot/image` reports a `version=` line.
- [ ] `target=aarch64-unknown-none`
- [ ] `selected_profile=shell-only`
- [ ] `profile_request=shell-only`
- [ ] `bootline=absent`
- [ ] `launch_profile_feature=disabled`
- [ ] shell prompt reached: `reovim-os>`
- [ ] input source is recorded honestly
- [ ] QEMU/VNC/HDMI display was not counted as keyboard input

## Command Transcript

Type these commands from the physical USB keyboard for final acceptance:

```text
proof
cat /boot/proof
help
help clear
help screentest
help input
help proof
help pwd
help ls
help cd
help cat
help mount
help device
help dmesg
help status
help probe
help launch
help reovim
help halt
cat /boot/help
clear
screentest
pwd
ls /
ls /boot
ls /dev
ls /log
mount
cat /boot/mounts
device
cat /boot/memory
cat /boot/devices
cd /dev
pwd
ls
cat uart0
cd /
cat /boot/image
status
cat /boot/status
input
cat /boot/input
probe help
cat /boot/probes
probe pcie
probe usb-keyboard
cat /boot/profile
launch
reovim
dmesg --stats
cat /log/stats
dmesg
cat /log/dmesg
```

Paste the resulting output:

```text

```

## Shutdown Check

After the command transcript is recorded, type this as the final command from
the physical USB keyboard:

```text
halt
```

Paste the resulting output:

```text

```

## USB Keyboard Readiness

Required success facts:

- [ ] A physical USB keypress reached the root shell.
- [ ] `proof` prints the physical input proof checklist.
- [ ] `cat /boot/proof` prints the VFS-backed proof pseudo-file.
- [ ] `cat /boot/help` prints the VFS-backed detailed help catalog.
- [ ] `help dmesg` / `ls /log` make kernel log stats discoverable.
- [ ] `help input` / `help proof` make live input diagnostics and the proof checklist self-describing.
- [ ] `help status` / `help probe` make `manual_next` and probe catalog discovery self-describing.
- [ ] `help pwd` / `help ls` / `help cd` / `help cat` / `help mount` make VFS navigation self-describing.
- [ ] `help` / `help clear` / `help screentest` / `clear` / `screentest` prove shell help and erase-line mode diagnostics.
- [ ] `help device` / `help launch` / `help reovim` / `help halt` make boot inventory, payload, and shutdown commands self-describing.
- [ ] `pwd` / `ls` / `mount` report the kernel VFS namespace and mounts.
- [ ] `device` / `cat /boot/memory` / `cat /boot/devices` report boot inventory.
- [ ] `cd /dev` / `ls` / `cat uart0` prove relative VFS device-file access.
- [ ] `input=usb-keyboard+uart-fallback`
- [ ] `usb_keyboard=ready`
- [ ] `usb_keyboard_last_poll=report-ready`
- [ ] `dmesg` contains `input.usb_keyboard=ready source=usb-keyboard+uart-fallback last_poll=report-ready`.
- [ ] `dmesg` pairs `input.line_source=usb-keyboard` with `fallback_bytes=0` immediately before typed command audit lines.
- [ ] `manual_next=type-shell-command`
- [ ] `probe usb-keyboard` reports `manual_next=type-shell-command`.
- [ ] `status` / `cat /boot/status` report image/profile identity and live ready source diagnostics.
- [ ] `input` / `cat /boot/input` report live input diagnostics.
- [ ] `cat /boot/probes` prints the VFS-backed probe catalog.
- [ ] `probe help` lists `pcie`, `usb-keyboard`, and `xhci-read-keyboard-report`.
- [ ] `probe pcie` reports the read-only PCIe/xHCI state.
- [ ] `cat /boot/profile` reports `profile=shell-only`, `launch=disabled`, `payloads=0`, and `input_mode=live`.
- [ ] `launch` / `reovim` report shell-only payload launch disabled.
- [ ] `dmesg --stats` / `cat /log/stats` report kernel log ring stats with `dropped_bytes=0`.
- [ ] `dmesg` has no `[klog] dropped_bytes=` retained-log wrap marker.
- [ ] `dmesg` contains `shell: <command>` and `shell.status=ok` audit lines for the typed commands.
- [ ] `dmesg` contains `shell.status=error` audit lines for the disabled payload launch commands.
- [ ] `dmesg` contains the retained `probe usb-keyboard` output with `manual_next=type-shell-command`.
- [ ] `halt` was typed last and printed `halt: ok`.
- [ ] No new `reovim-os>` prompt appeared after `halt: ok`.

If unsuccessful, record the exact blocker:

```text

```

## Result

- [ ] PASS: physical USB keyboard proof complete.
- [ ] FAIL: display/UART/scripted evidence only.

Notes:

```text

```
