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

Current expected size is `435900` bytes.

Copy `apps/target/aarch64-unknown-none/debug/reovim-os.kernel8.img` to the Pi
4 boot partition as `kernel8.img`, using the normal Raspberry Pi firmware
files and DTB for the board. Keep `REOVIM_OS_BOOTLINE` unset for real-machine
input proof; a bootline script is only a deterministic test harness.

To run the full local media-prep path against a mounted Pi 4 boot partition:

```sh
apps/os/targets/raspi4b-aarch64/prepare-boot-media.sh \
  --bootfs /path/to/bootfs \
  --evidence tmp/raspi4b-usb-keyboard-evidence.md
```

The media-prep helper runs preflight, installs the exact built `kernel8.img`,
verifies the installed image SHA-256 against the built image, and only then
publishes the evidence seed for the physical session. The published evidence
seed includes `media_prepare=ok`, the installed `kernel8.img` path, byte count,
and SHA-256, and the validator compares those installed-media values with the
seeded image identity. Use this path for the real HDMI plus USB-keyboard proof.

To build and install the image manually onto a mounted Pi 4 boot partition:

```sh
apps/os/targets/raspi4b-aarch64/install-image.sh --build /path/to/bootfs
```

To check the mounted boot partition before installing:

```sh
apps/os/targets/raspi4b-aarch64/check-bootfs.sh /path/to/bootfs
```

The checker requires non-empty Pi 4 boot firmware files:
`start4.elf`, `fixup4.dat`, and `bcm2711-rpi-4-b.dtb`. It prints the
bootfs path, firmware byte counts, and the existing `kernel8.img` identity
when present.

The install helper only copies `kernel8.img`; it does not format media, mount
media, or install Raspberry Pi firmware files. If `kernel8.img` already exists
on the boot partition, the helper saves a timestamped backup before replacing
it, then prints the installed image byte count and SHA-256 for the evidence
template.

To smoke-test the installer against a temporary directory instead of real
media:

```sh
apps/os/targets/raspi4b-aarch64/test-check-bootfs.sh
apps/os/targets/raspi4b-aarch64/test-install-image.sh
apps/os/targets/raspi4b-aarch64/test-prepare-boot-media.sh
```

The smoke tests verify bootfs readiness checks, dry-run identity output,
installed image hash, distinct backups after repeated installs, and the
full media-prep wrapper.

## Local Preflight

Before writing real boot media, run the target preflight:

```sh
apps/os/targets/raspi4b-aarch64/preflight-real-board.sh
```

The preflight builds the no-bootline image, runs the installer smoke, boots the
aarch64 QEMU display/UART/proof smoke, and prints the image byte count,
SHA-256, install command, and evidence-template path. The QEMU smoke pipes the
`proof` command over UART to the exact no-bootline image and requires the
root-shell checklist output. If the boot partition is already mounted, pass it
for bootfs validation plus an installer dry-run:

```sh
apps/os/targets/raspi4b-aarch64/preflight-real-board.sh --bootfs /path/to/bootfs
```

QEMU preflight output remains display/UART/proof-command evidence only; it is
not physical USB keyboard proof.

To write a prefilled evidence seed after preflight passes without installing:

```sh
apps/os/targets/raspi4b-aarch64/preflight-real-board.sh \
  --bootfs /path/to/bootfs \
  --evidence tmp/raspi4b-usb-keyboard-evidence.md
```

The generated evidence file records the current image path, byte count,
SHA-256, build/install commands, boot partition path, and preflight result.

To smoke-test the evidence-seed path without QEMU or real media:

```sh
apps/os/targets/raspi4b-aarch64/test-preflight-real-board.sh
```

The smoke test verifies the preflight output and generated evidence fields.

After the real-board session, validate the completed physical proof file:

```sh
apps/os/targets/raspi4b-aarch64/validate-evidence.sh \
  tmp/raspi4b-usb-keyboard-evidence.md
```

The validator rejects checked display-only, UART, bootline-script, or FAIL
labels. It also requires checked HDMI-display and physical-keyboard setup,
physical-keyboard PASS labels, no-bootline image facts, installed-media
identity from `prepare-boot-media.sh`, USB-keyboard readiness, live
source diagnostics from `status` / `input`, VFS-backed live diagnostics from
`cat /boot/status` / `cat /boot/input`, the in-OS `proof` checklist,
shell help/renderer evidence from `help` / `clear` / `screentest`, VFS
namespace evidence from `pwd` / `ls` / `mount` / `cat /boot/mounts`, boot
inventory from `ls /dev` / `device` / `cat /boot/memory` /
`cat /boot/devices`, relative device-file access from `cd /dev` /
`cat uart0`, `probe help` hardware-target catalog output, read-only PCIe/xHCI
state from `probe pcie`, `probe usb-keyboard` output, shell-only
`launch`/`reovim` disabled output, and `dmesg` command/status audit lines
including the expected disabled-payload `shell.status=error` entries. It is a
textual guard for completed evidence; it does not replace the physical HDMI
plus USB-keyboard session.

To smoke-test the validator:

```sh
apps/os/targets/raspi4b-aarch64/test-validate-evidence.sh
```

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
proof
cat /boot/proof
help
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

Record the full visible output or serial transcript. The key evidence is:

- `proof` prints the physical-input proof checklist from the running OS.
- `cat /boot/proof` prints the same checklist through the kernel VFS.
- `help`, `clear`, and `screentest` prove the shell help path and framebuffer
  renderer diagnostics are reachable from the physical keyboard session.
- `pwd`, `ls /`, `ls /boot`, `mount`, and `cat /boot/mounts` show the root
  VFS namespace and pseudo-filesystem mount table.
- `ls /dev`, `device`, `cat /boot/memory`, and `cat /boot/devices` show the
  device namespace plus boot memory and device inventory facts.
- `cd /dev`, `pwd`, `cat uart0`, and `cd /` prove session cwd and relative
  VFS pseudo-device access.
- `cat /boot/image` shows `target=aarch64-unknown-none` and `bootline=absent`.
- `status` / `input` show live ready source diagnostics:
  `source=usb-keyboard+uart-fallback`, `source_state=ready`, `mode=live`,
  `usb_keyboard_probe=enabled`, and a nonzero
  `usb_keyboard_poll_interval_ms`.
- `cat /boot/status` / `cat /boot/input` show the same live diagnostics
  through the kernel VFS.
- `probe help` lists `pcie`, `usb-keyboard`, and
  `xhci-read-keyboard-report`, proving the hardware probe paths were
  discoverable from the shell.
- `probe pcie` reports the read-only PCIe/root-complex and xHCI discovery
  state without running active xHCI start/enumeration transitions.
- `probe usb-keyboard` either records a precise lower-provider blocker or queues
  decoded bytes without consuming shell input.
- `launch` and `reovim` report disabled launch in the shell-only image; this
  proves boot does not depend on editor/server payload startup.
- `dmesg` includes the command audit lines and probe output.
- The final `cat /log/dmesg` includes `shell.status=error` for the intentional
  shell-only `launch` and `reovim` disabled paths.
- The final `cat /log/dmesg` shows the `shell.status=ok` record for the prior
  `dmesg` command, because `dmesg` records its own status after printing.
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
