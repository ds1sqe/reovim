# Raspberry Pi 4 AArch64 OS Image

This directory owns the official Raspberry Pi 4 distribution target for the
`apps/os` RTOS image. `arch/tests/fixtures` remains a CI/floor harness; manual
board evidence belongs here and in the #800 flight log.

## Artifact

Default shell-only image:

```sh
apps/os/targets/raspi4b-aarch64/build-image.sh
```

The helper unsets `REOVIM_OS_BOOTLINE` and `REOVIM_OS_PROFILE`, builds
`reovim-os` for `aarch64-unknown-none`, converts the ELF to raw
`kernel8.img` bytes using rustup's `llvm-objcopy`, and prints:

```text
image=apps/target/aarch64-unknown-none/debug/reovim-os.kernel8.img
bytes=<image-size>
```

Current expected size at the time this helper was added is `433788` bytes.

Copy `apps/target/aarch64-unknown-none/debug/reovim-os.kernel8.img` to the Pi
4 boot partition as `kernel8.img`, using the normal Raspberry Pi firmware
files and DTB for the board. Keep `REOVIM_OS_BOOTLINE` unset for real-machine
input proof; a bootline script is only a deterministic test harness.

To build and install the image onto a mounted Pi 4 boot partition:

```sh
apps/os/targets/raspi4b-aarch64/install-image.sh --build /path/to/bootfs
```

The install helper only copies `kernel8.img`; it does not format media, mount
media, or install Raspberry Pi firmware files. If `kernel8.img` already exists
on the boot partition, the helper saves a timestamped backup before replacing
it, then prints the installed image byte count and SHA-256 for the evidence
template.

## QEMU Smoke

QEMU raspi4b currently disables the DTB PCIe path used by the Pi 4 VL805 xHCI
controller, so this smoke proves framebuffer/UART boot behavior only. It is not
physical USB keyboard evidence.

```sh
timeout 8s qemu-system-aarch64 \
  -M raspi4b \
  -m 2048 \
  -display none \
  -serial stdio \
  -semihosting \
  -dtb arch/tests/fixtures/dtb/bcm2711-rpi-4-b.dtb \
  -kernel apps/target/aarch64-unknown-none/debug/reovim-os.kernel8.img
```

Expected honest QEMU label:

```text
input=pl011-uart mode=live usb_keyboard=unavailable
```

## Real Board Proof

Hardware setup:

- Raspberry Pi 4.
- HDMI display attached before boot.
- Physical USB keyboard attached before boot.
- Optional UART serial console attached for logs; UART input is fallback only.
- No `REOVIM_OS_BOOTLINE` in the image environment.

At the `reovim-os>` prompt, type these commands from the physical USB
keyboard:

```text
cat /boot/image
status
input
probe help
probe usb-keyboard
cat /boot/profile
dmesg
```

Record the full visible output or serial transcript. The key evidence is:

- `cat /boot/image` shows `target=aarch64-unknown-none` and `bootline=absent`.
- `status` / `input` show whether the USB probe is enabled and whether decoded
  bytes are pending.
- `probe usb-keyboard` either records a precise lower-provider blocker or queues
  decoded bytes without consuming shell input.
- `dmesg` includes the command audit lines and probe output.
- Final success requires physical USB keyboard input to reach the shell and
  `cat /boot/profile` or `status` to show:

```text
input=usb-keyboard+uart-fallback
usb_keyboard=ready
```

Do not count HDMI framebuffer output, QEMU/VNC display, bootline scripting, or
UART-typed commands as USB keyboard proof.

Use `evidence-template.md` to record the session result. The template keeps
display-only, UART input, bootline-script, and physical USB keyboard evidence
separate, and it marks the exact facts needed before #800 can call the
real-machine keyboard proof complete.

## Evidence Labels

Use these labels in the #800 flight log:

| Label | Meaning |
|---|---|
| `display-only` | Framebuffer/HDMI/VNC output was observed; input was not proven. |
| `UART input` | Commands were typed through serial/PL011 fallback. |
| `bootline-script` | Commands came from `REOVIM_OS_BOOTLINE`; deterministic harness only. |
| `physical USB keyboard` | Commands were typed from the attached USB keyboard and readiness reported `ready`. |

Only `physical USB keyboard` satisfies the remaining #800 real-machine input
acceptance criteria.
