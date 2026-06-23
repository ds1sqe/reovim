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
cat /boot/help
clear
screentest
pwd
ls /
ls /boot
ls /dev
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
probe pcie
probe usb-keyboard
cat /boot/profile
launch
reovim
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
- [ ] `cat /boot/help` prints the VFS-backed help catalog.
- [ ] `help` / `clear` / `screentest` prove the shell help and renderer commands.
- [ ] `pwd` / `ls` / `mount` report the kernel VFS namespace and mounts.
- [ ] `device` / `cat /boot/memory` / `cat /boot/devices` report boot inventory.
- [ ] `cd /dev` / `ls` / `cat uart0` prove relative VFS device-file access.
- [ ] `input=usb-keyboard+uart-fallback`
- [ ] `usb_keyboard=ready`
- [ ] `usb_keyboard_last_poll=report-ready`
- [ ] `status` / `cat /boot/status` report image/profile identity and live ready source diagnostics.
- [ ] `input` / `cat /boot/input` report live input diagnostics.
- [ ] `probe help` lists `pcie`, `usb-keyboard`, and `xhci-read-keyboard-report`.
- [ ] `probe pcie` reports the read-only PCIe/xHCI state.
- [ ] `cat /boot/profile` reports `profile=shell-only`, `launch=disabled`, `payloads=0`, and `input_mode=live`.
- [ ] `launch` / `reovim` report shell-only payload launch disabled.
- [ ] `dmesg` contains `shell: <command>` and `shell.status=ok` audit lines for the typed commands.
- [ ] `dmesg` contains `shell.status=error` audit lines for the disabled payload launch commands.
- [ ] `dmesg` contains the `probe usb-keyboard` output or blocker.
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
