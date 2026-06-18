# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [0.16.0-dev] - Unreleased

### Added

- **Runtime-discovered hardware facts + boot-time health banner** (#778,
  platform-info sub-plan 02). The kernel now learns what machine it booted on
  and reports its boot health, the way a real kernel's `dmesg` does:
  - An arch-neutral `BootInfo` (memory map + CPU id/freq/count) lives in
    `kabi-platform` and is pushed at entry through `LauncherArgs`/`Init::new`,
    so both the kernel and any arch backend share one shape without a
    kernel→arch dependency edge. The memory field is a `&'static [MemoryRange]`
    slice (a map, not a single range) so multi-range platforms (x86 E820) and
    single-range ones (the ARM mailbox) fit the same type.
  - An aarch64 discovery backend (`arch/src/sys/none_aarch64/boot_info.rs`)
    fills it from real hardware: usable RAM via mailbox tag `0x00010005`, the
    CPU id from `MIDR_EL1`, and the counter frequency from `CNTFRQ_EL0`.
  - A kernel `diagnostics` module emits, at the boot-handoff tail, a
    dmesg-style hardware banner plus **live boot-time health probes**. Ten
    probes run as two-line ping-pong — a `probing...` line, then a verdict —
    covering CPU id and core count, memory-map presence and usable RAM, a byte
    and a page heap allocation, log-ring capacity and occupancy, and timer
    frequency and monotonic advance. Each is a genuine check with a real FAIL
    branch (zero id register, empty map, refused alloc, frozen counter, …), not
    cosmetic `[ OK ]` theatre. Health lines ride the same
    `render::render_event` pipeline as the boot stages (a `health.<metric>`
    event id mapped to the `health` subsystem), so the ring and any sink render
    them identically (LOG1). Under QEMU raspi4b the banner is visible live over
    both the PL011 serial and the VNC framebuffer; `-icount shift=N` paces the
    boot ladder so the probe checklist is watchable in slow motion.

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
