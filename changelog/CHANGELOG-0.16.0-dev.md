# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [0.16.0-dev] - Unreleased

### Added

- **Live, useful bare-metal boot console** (#778, boot-console sub-plan 01).
  On the freestanding aarch64 floor (QEMU raspi4b / BCM2711) the kernel's
  boot log is now rendered live to the screen as it happens, not replayed
  after boot. Three pieces land together:
  - The freestanding `file_write` platform slot (`arch/src/platform.rs`) is
    implemented to route the standard fds (1/2) to the floor's byte sink,
    mirroring the Linux twin. The kernel's own `stderr_echo` therefore reaches
    the console during `Init::boot()` instead of being dropped at an `-ENOSYS`
    stub.
  - A **fan-out console** in the platform floor
    (`arch/src/sys/none_aarch64/console.rs`): `sys::write` for fd 1/2 now
    writes to the PL011 UART **and** to an installed framebuffer console. A
    payload opts in by installing a console (`console::install`); display-less
    payloads (the exit-code selftests) stay UART-only. The bare-metal boot
    payload no longer drains the log ring by hand — the framebuffer is fed
    live through the kernel's own machinery.
  - Boot-stage log lines are **human-readable**: `stage N/7 <verb>: <name>`
    (e.g. `stage 2/7 ok: shell config`) rather than the raw OBS1 event id.
    The enrichment lives in a single `render::render_event` that both the log
    ring and the file sink call, so the ring and any sink can never disagree
    on a line's bytes (LOG1: one renderer, one mechanism).
