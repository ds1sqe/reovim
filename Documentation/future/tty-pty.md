# TTY / PTY — Beyond-0.16

**Status:** Non-normative. Future terminal-device direction, not a 0.16
goal.

The current #800 root-daemon shell should not be named `tty`. It is a
kernel-owned console shell: a prompt, line parser, and built-ins running over
the available console/input surfaces. Calling it `tty` would imply more
contract than exists today: terminal devices, line discipline, raw/cooked
mode, carrier state, PTY pairs, and possibly POSIX-shaped expectations.

Keep the 0.16 names narrow:

| Name | Meaning |
|---|---|
| `console` | Concrete boot/display surface: UART text, framebuffer text, and later keyboard input. |
| `terminal` | Product-facing logical terminal/raw-mode vocabulary and its bridge to hosted provider slots. |
| `root_shell` | Root-daemon command surface: prompt, parser, built-ins, payload launch commands. |
| `console_io` | Line/input/output adapter between the root shell and the available console surfaces. |
| `input` | Common translation for physical input reports into console bytes/events; raw device polling stays below. |
| `tty` | Future system-kernel terminal-device subsystem. Not the first root shell. |
| `pty` | Future virtual terminal pair used when a process/client/harness needs a terminal endpoint without physical hardware. |

## Real-Machine Console Cut

The real-machine target is HDMI display plus USB keyboard. The current display
side already flows through the framebuffer console: the lower aarch64 provider
installs a framebuffer surface, and `system/lib/kernel::console` renders text
to it while UART remains the serial log path.

Keyboard input is separate. Do not call the HDMI/VNC framebuffer proof
"interactive" until a physical input provider exists. The intended split is:

- lower USB/board code owns the host controller, device enumeration,
  interrupt-IN polling, and any board-specific reset/power sequencing;
- `system/lib/kernel::input` owns common report translation, starting with USB
  HID boot-keyboard 8-byte reports into root-console bytes;
- `system/lib/kernel::console_io` remains the line discipline over the byte
  source and writer callbacks;
- `apps/os` selects and wires the available byte source. Serial UART may remain
  the fallback, but USB keyboard must be named separately in manual evidence.

This keeps raw USB/DWC/HID transport below the bridge while letting the system
kernel own reusable input policy.

## Future TTY Shape

A real Reovim TTY subsystem would live in `system/lib/kernel` and represent a
terminal endpoint, not a framebuffer and not the hosted TUI crate. It would
own the generic World policy:

- endpoint lifecycle and attachment;
- byte/event carrier state;
- line discipline, including canonical/raw-style modes if needed;
- window or cell-size metadata;
- input editing policy only where it is terminal-wide, not shell-specific;
- delivery of terminal events to root daemon, local console clients, or later
  process-like tasks.

The face rule still controls the design:

- upper/product code sees domain `uapi` vocabulary;
- provider/floor/device code sees `kabi` contracts and local mechanism;
- only `system/lib/kernel` bridges the two;
- no public/product POSIX face is introduced just because the subsystem is
  called TTY.

## Future PTY Shape

A PTY is useful when one component needs to behave like a terminal endpoint
without being physical hardware. Possible future consumers:

- a deterministic CLI test harness that drives the root shell without VNC
  keyboard injection;
- a local console client that wants a terminal-like byte/event carrier;
- a hosted compatibility path where an existing terminal client can be
  exercised against an RTOS-side endpoint;
- later task/process supervision if Reovim grows a real execution model.

The PTY contract should be named in Reovim terms, even if it resembles the
Unix idea. Avoid importing `/dev/pts`, file descriptors, `ioctl`, `termios`,
sessions, job control, signals, or `/bin/sh` semantics into product-facing
APIs. If lower providers use POSIX-shaped scalar values internally, those
belong in `kabi/platform`; product semantics remain in domain `uapi` leaves.

## Graduation Triggers

Move this design into a numbered chapter only when at least one concrete
consumer forces the abstraction:

- the RTOS local UI needs a reusable terminal carrier rather than direct
  console access;
- the test harness needs scripted terminal input beyond simple UART lines;
- the root daemon must host more than one interactive endpoint;
- a launch profile needs to run an editor/server/client payload behind a
  terminal-like endpoint;
- the hosted TUI can be reused honestly because a real RTOS terminal/carrier
  contract exists.

Until one of those lands, #800 should use `root_shell` and `console_io` for
the kernel-only boot proof and reserve `tty` / `pty` for the later subsystem.
