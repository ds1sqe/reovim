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

Current expected size is `813988` bytes.

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

If exactly one mounted Pi 4 boot partition candidate is present, the wrapper
can discover it through the same read-only bootfs helper:

```sh
apps/os/targets/raspi4b-aarch64/prepare-boot-media.sh \
  --bootfs auto \
  --evidence tmp/raspi4b-usb-keyboard-evidence.md
```

Auto-discovery stops before preflight/install when no candidate or multiple
candidates are found; pass an explicit `--bootfs DIR` in that case.

The media-prep helper runs preflight, installs the exact built `kernel8.img`,
verifies the installed image SHA-256 against the built image, and only then
publishes the evidence seed for the physical session. The published evidence
seed includes the actual `prepare-boot-media.sh` command, `media_prepare=ok`,
the installed `kernel8.img` path, byte count, and SHA-256, and the validator
compares those installed-media values with the seeded image identity. It also
requires the recorded media-prep command, boot partition path, and installed
`kernel8.img` path to describe the same bootfs. Use this path for the real
HDMI plus USB-keyboard proof.
Do not use `--skip-qemu` for final proof preparation: it is a helper-smoke mode
only, and completed evidence with `qemu_smoke=skipped`, a recorded
`--skip-qemu` command argument, or a remaining `Preflight warning:` row is
rejected.

To find mounted Pi 4 boot partition candidates before preparing media:

```sh
apps/os/targets/raspi4b-aarch64/find-bootfs.sh
```

The discovery helper is read-only. It scans mounted FAT-like filesystems and
prints only candidates that pass the same Pi 4 firmware checks as
`check-bootfs.sh`. If it prints
`next=mount the Pi 4 boot partition or pass candidate DIR paths explicitly`,
mount the Pi 4 boot partition or rerun the helper with explicit candidate
paths before running media preparation.

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
media, or install Raspberry Pi firmware files. It refuses to install when the
required Pi 4 firmware files are missing or empty, matching the bootfs checker.
If `kernel8.img` already exists on the boot partition, the helper saves a
timestamped backup before replacing it, then prints the installed image byte
count and SHA-256 for the evidence template.

To run every no-real-media Pi 4 target smoke before touching real media:

```sh
apps/os/targets/raspi4b-aarch64/test-target-smokes.sh
```

For narrower debugging, the aggregate smoke runs these individual checks:

```sh
cargo test -p reovim-arch --test fixtures_exec testrt_pilot_all_pass_exits_zero -- --exact
apps/os/targets/raspi4b-aarch64/test-check-bootfs.sh
apps/os/targets/raspi4b-aarch64/test-analyze-dump.sh
apps/os/targets/raspi4b-aarch64/test-find-bootfs.sh
apps/os/targets/raspi4b-aarch64/test-install-image.sh
apps/os/targets/raspi4b-aarch64/test-preflight-real-board.sh
apps/os/targets/raspi4b-aarch64/test-proof-command-list.sh
apps/os/targets/raspi4b-aarch64/test-prepare-boot-media.sh
apps/os/targets/raspi4b-aarch64/test-validate-evidence.sh
```

The smoke tests run the no_std system-kernel selftest fixture, build a fresh
no-bootline image, verify this README's current expected image size matches
the built image, then verify bootfs readiness checks, read-only bootfs
discovery, dump-analyzer positive and negative fixtures, dry-run identity
output, installed image hash, distinct backups after repeated installs,
preflight evidence generation, proof command and required-fact consistency
across the runbook/template/generated seed, the full media-prep wrapper, and
completed evidence validation.

## Local Preflight

Before writing real boot media, run the target preflight:

```sh
apps/os/targets/raspi4b-aarch64/preflight-real-board.sh
```

The preflight builds the no-bootline image, runs the installer smoke, boots the
aarch64 QEMU display/UART/proof smoke, and prints the image byte count,
SHA-256, low-level install command, preferred `prepare-boot-media.sh` command,
next action, and evidence-template path. The QEMU smoke pipes the `proof`
command over UART to the exact no-bootline image and requires the root-shell
checklist output. If the boot partition is already mounted, pass it for bootfs
validation plus an installer dry-run:

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
SHA-256, build/install commands, the preferred media-preparation command, boot
partition path, and preflight result.
This is a preflight-only seed; final physical proof still needs
`prepare-boot-media.sh` to rewrite that media-preparation command with the
actual final evidence path and add installed-media identity before
`validate-evidence.sh` can pass.

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
identity from `prepare-boot-media.sh`, shell-only image identity from
`cat /boot/image`, shell-only profile identity from `cat /boot/profile`,
USB-keyboard readiness, the lower `usb_keyboard_last_poll=report-ready`
diagnostic, the retained
`input.usb_keyboard=ready source=usb-keyboard+uart-fallback last_poll=report-ready`
kernel-log transition, retained `input.line_source=usb-keyboard` audit lines
with `fallback_bytes=0` immediately before typed command audit rows, identity
plus live source diagnostics from `status` / `cat /boot/status`, live input
diagnostics from `input` / `cat /boot/input`, the in-OS `proof` checklist, the
VFS-backed detailed help catalog from
`cat /boot/help`, log-stat discoverability from `help dmesg` / `ls /log`,
targeted input/proof/status/probe guidance from `help input` / `help proof` /
`help status` / `help probe`, remaining operator `/bin` program guidance from
`help clear` / `help device` / `help launch` / `help reovim` / `help halt`,
shell help and erase-line mode diagnostics from `help` / `help clear` /
`help screentest` / `clear` / `screentest`, VFS namespace evidence from
`help pwd` / `help ls` / `help cd` / `help cat` / `help read` / `help mount` /
`pwd` / `ls` / `mount` /
`cat /boot/mounts`, boot inventory from `ls /dev` / `device` /
`cat /boot/memory` / `cat /boot/devices`, relative device-file access from
`cd /dev` / `ls` / `cat uart0`, hardware-target catalog output from
`probe help` / `cat /boot/probes`, read-only PCIe/xHCI state from
`probe pcie`, live and retained `probe usb-keyboard` output including
`manual_next=type-shell-command`, shell-only `launch`/`reovim` disabled output,
kernel log ring stats from `dmesg --stats` /
`cat /log/stats`, the `/bin/proof` and `/bin/proc` process rows with
`loader=source-image entry_fn=bin_proof` and
`loader=source-image entry_fn=bin_proc`, and `dmesg` command/status audit
lines including the expected disabled-payload `shell.status=error` entries. It also
requires the terminal `halt` check to print `halt: ok` with no later root-shell
prompt. It is a textual guard for completed evidence; it does not replace the
physical HDMI plus USB-keyboard session, and it rejects preflight-only seeds
that have not gone through media preparation. It also rejects skipped-QEMU
helper seeds that still contain a `Preflight warning:` row, even if the
preflight result line was edited afterward.

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
keyboard for the main proof transcript:

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
help read
help mount
help device
help dmesg
help dump
help sched
help proc
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
dump status
dump snapshot
dump sync
cat /log/stats
cat /log/events
proc
dmesg
cat /log/dmesg
```

After recording the main transcript, type this final terminal command from the
physical USB keyboard:

```text
halt
```

Record the full visible output or serial transcript. The key evidence is:

- `proof` prints the physical-input proof checklist from the running OS.
- `cat /boot/proof` prints the same checklist through the kernel VFS.
- `cat /boot/help` prints the root-shell detailed help catalog through the
  kernel VFS.
- `help dmesg` and `ls /log` show that the log namespace and log-stat command
  path are discoverable from the running shell.
- `help`, `help clear`, `help screentest`, `clear`, and `screentest` prove
  the shell help path and framebuffer erase-line mode diagnostics, including
  erase-to-end, erase-to-start, and whole-line erase redraw checks, are
  reachable from the physical keyboard session.
- `help input` names the live input diagnostics command, and `help proof`
  names the running proof checklist command.
- `help pwd`, `help ls`, `help cd`, `help cat`, `help read`, and
  `help mount` name the VFS navigation, TTY line read, and pseudo-file
  commands before the proof uses them.
- `help device`, `help launch`, `help reovim`, and `help halt` name the boot
  inventory, payload, and terminal shutdown commands before the proof uses
  them.
- `pwd`, `ls /`, `ls /boot`, `mount`, and `cat /boot/mounts` show the root
  VFS namespace and pseudo-filesystem mount table.
- `ls /dev`, `device`, `cat /boot/memory`, and `cat /boot/devices` show the
  device namespace plus boot memory and device inventory facts.
- `cd /dev`, `pwd`, `ls`, `cat uart0`, and `cd /` prove session cwd and
  relative VFS pseudo-device access.
- `cat /boot/image` shows `package=reovim-os`, a `version=` line,
  `target=aarch64-unknown-none`, `selected_profile=shell-only`,
  `profile_request=shell-only`, `bootline=absent`, and
  `launch_profile_feature=disabled`.
- `status` repeats the image/profile identity facts and shows live ready input
  diagnostics: `input=usb-keyboard+uart-fallback`, `source_state=ready`,
  `input_mode=live`, `usb_keyboard_probe=enabled`, and a nonzero
  `usb_keyboard_poll_interval_ms`. It also shows
  `usb_keyboard_last_poll=report-ready` so the proof names the lower
  interrupt-IN report path that made the keyboard ready. If the keyboard is
  not ready yet, `manual_next` points at the next action, such as
  `probe-pcie`, `probe-xhci-start`, `probe-xhci-read-keyboard-report`,
  `probe-usb-keyboard`, or `press-usb-key` when an interrupt-IN report
  transfer is already queued. In the successful physical proof it must end at
  `manual_next=type-shell-command`.
- `help status` names the `manual_next` summary path, and `help probe` points
  at both `probe help` and `cat /boot/probes`.
- `cat /boot/status` shows the same status summary through the kernel VFS, and
  `cat /boot/input` shows the live input diagnostics through the kernel VFS,
  including `source=usb-keyboard+uart-fallback` and `mode=live`.
- `probe help` lists `pcie`, `usb-keyboard`, and
  `xhci-read-keyboard-report`, proving the hardware probe paths were
  discoverable from the shell.
- `cat /boot/probes` prints the same hardware-target catalog through the
  kernel VFS.
- `probe pcie` reports the read-only PCIe/root-complex and xHCI discovery
  state without running active xHCI start/enumeration transitions.
- `probe usb-keyboard` either records a precise lower-provider blocker or queues
  decoded bytes without consuming shell input. It also prints `manual_next=...`
  with the same next-action vocabulary as `status`, so the operator can stay
  in the probe flow while bringing up the physical keyboard. In completed
  evidence, this probe block must report `manual_next=type-shell-command`.
- `cat /boot/profile` shows `profile=shell-only`, `launch=disabled`,
  `payloads=0`, `input_mode=live`, and the `reovim-os>` prompt identity.
- `launch` and `reovim` report disabled launch in the shell-only image; this
  proves boot does not depend on editor/server payload startup.
- `dmesg --stats` and `cat /log/stats` report kernel log ring capacity,
  retained bytes, and `dropped_bytes=0` through command and VFS paths.
- `dump status`, `dump snapshot`, and `dump sync` expose dump state through a
  `/bin` program, include package/version/target/profile identity in the dump
  header, and fail closed while persistence is unavailable.
- `cat /log/events` reports structured kernel events with process/task
  identity.
- `proc` reports the live process table through a `/bin` process-management
  program, including `loader=source-image entry_fn=bin_proc` for the running
  `/bin/proc` process.
- `proof` reports the physical proof checklist through a `/bin` program,
  including `loader=source-image entry_fn=bin_proof` for the running
  `/bin/proof` process.
- `dmesg` and `cat /log/dmesg` do not print a
  `[klog] dropped_bytes=` retained-log wrap marker.
- `dmesg` includes the command audit lines, the retained `probe usb-keyboard`
  output with `manual_next=type-shell-command`, and the
  `input.usb_keyboard=ready source=usb-keyboard+uart-fallback last_poll=report-ready`
  transition.
- `dmesg` pairs `input.line_source=usb-keyboard` rows with
  `fallback_bytes=0` immediately before typed command audit rows, proving UART
  fallback did not carry the shell transcript.
- The final `cat /log/dmesg` includes `shell.status=error` for the intentional
  shell-only `launch` and `reovim` disabled paths.
- The final `cat /log/dmesg` shows the `shell.status=ok` record for the prior
  `dmesg` command, because `dmesg` records its own status after printing.
- `halt` prints `halt: ok` as the final command and no new `reovim-os>` prompt
  appears after that line.
- Final success requires physical USB keyboard input to reach the shell.
  `cat /boot/profile` must show:

```text
input=usb-keyboard+uart-fallback
input_mode=live
usb_keyboard=ready
```

  `status`, `/boot/status`, `input`, or `/boot/input` must also show:

```text
usb_keyboard_last_poll=report-ready
manual_next=type-shell-command
```

  The `probe usb-keyboard` output block must also include
  `manual_next=type-shell-command`; a generic status-only line is not enough
  for completed physical-keyboard evidence.

Do not count HDMI framebuffer output, QEMU/VNC display, bootline scripting, or
UART-typed commands as USB keyboard proof.

## Dump Analysis

The current image can print a parseable `reovim-dump-v1` snapshot, and the host
tool can validate a saved snapshot text or mounted bootfs directory:

```sh
apps/os/targets/raspi4b-aarch64/analyze-dump.sh \
  --expect-package reovim-os \
  --expect-target aarch64-unknown-none \
  --expect-profile shell-only \
  --require-zero-drops \
  <dump-file-or-mounted-bootfs>
```

This analyzer verifies the dump header checksum, boot/session identity, image
package/version/target/profile rows, boot/device/proof/panic/runtime sections,
and dropped-log count. If the input is a directory, it must contain exactly one
supported artifact path: `reovim-dump-v1.txt`, `reovim-dump.txt`,
`reovim/dump-v1.txt`, or `reovim/dump.txt`. Today it is an offline contract for
a copied `dump snapshot` transcript, copied dump artifact, or mounted bootfs
artifact. It is not persistent SD-card proof until `dump sync` reports a
checked write/read-back storage target instead of
`reason=no-persistent-dump-sink`.

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
