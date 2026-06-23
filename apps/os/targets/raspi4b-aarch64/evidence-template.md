# Raspberry Pi 4 USB Keyboard Evidence Template

Use this template for the #800 real-machine proof. The proof is incomplete
unless HDMI output is observed, input source is `physical USB keyboard`, and
the shell reports `usb_keyboard=ready`.

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

- [ ] `target=aarch64-unknown-none`
- [ ] `bootline=absent`
- [ ] shell prompt reached: `reovim-os>`
- [ ] input source is recorded honestly
- [ ] QEMU/VNC/HDMI display was not counted as keyboard input

## Command Transcript

Type these commands from the physical USB keyboard for final acceptance:

```text
cat /boot/image
status
input
probe help
probe usb-keyboard
cat /boot/profile
dmesg
cat /log/dmesg
```

Paste the resulting output:

```text

```

## USB Keyboard Readiness

Required success facts:

- [ ] A physical USB keypress reached the root shell.
- [ ] `input=usb-keyboard+uart-fallback`
- [ ] `usb_keyboard=ready`
- [ ] `probe help` lists `usb-keyboard` and `xhci-read-keyboard-report`.
- [ ] `dmesg` contains `shell: <command>` and `shell.status=ok` audit lines for the typed commands.
- [ ] `dmesg` contains the `probe usb-keyboard` output or blocker.

If unsuccessful, record the exact blocker:

```text

```

## Result

- [ ] PASS: physical USB keyboard proof complete.
- [ ] FAIL: display/UART/scripted evidence only.

Notes:

```text

```
