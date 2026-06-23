# Raspberry Pi 4 USB Keyboard Evidence Template

Use this template for the #800 real-machine proof. The proof is incomplete
unless the input source is `physical USB keyboard` and the shell reports
`usb_keyboard=ready`.

## Session

- Date:
- Operator:
- Board:
- Image path:
- Image bytes:
- Image build command:
- Evidence label:
  - [ ] display-only
  - [ ] UART input
  - [ ] bootline-script
  - [ ] physical USB keyboard
- HDMI display attached before boot:
- Physical USB keyboard attached before boot:
- UART serial console attached:
- `REOVIM_OS_BOOTLINE` unset:

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
```

Paste the resulting output:

```text

```

## USB Keyboard Readiness

Required success facts:

- [ ] A physical USB keypress reached the root shell.
- [ ] `input=usb-keyboard+uart-fallback`
- [ ] `usb_keyboard=ready`
- [ ] `dmesg` contains `shell: <command>` audit lines for the typed commands.
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
