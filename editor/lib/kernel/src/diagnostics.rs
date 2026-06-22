//! Boot diagnostics — the hardware banner and live health probes (reovim's
//! `dmesg`-style boot self-test).
//!
//! Run at the boot tail (after the stage-7 handoff, before `Init::boot`
//! returns), these emit the discovered hardware facts plus a suite of **real**
//! health probes, each riding 01's live console echo: every line is a
//! `health.*` DS12 event that flows `event_bus.emit` → `LogRing::push_event` →
//! `render_event` → `sink::stderr_echo` → the floor's fan-out console, exactly
//! like the boot-stage lines (LOG1: one renderer, one mechanism).
//!
//! ## Ping-pong probes: announce, then verdict
//!
//! Each probe emits two lines — a `"<key>: probing..."` line, then its verdict
//! (`ok`, a metric, or `FAIL`) once the check has run. The probing line is
//! emitted *before* the work (the allocation, the second clock read), so under
//! QEMU `-icount` slow-mode the boot log visibly works through the checklist
//! rather than printing a wall of results at once.
//!
//! ## Real checks, not per-stage theater
//!
//! Boot stages 1-6 are structural stubs (`boot.rs`) — they init no subsystem,
//! so a per-stage `[ OK ]` line would assert nothing. Instead each probe checks
//! a subsystem that genuinely exists and reports a genuine metric or a genuine
//! `FAIL`: the CPU id register read back non-zero, the implementer decoded to a
//! known vendor, the cache geometry and affinity registers read back well-formed
//! values, a logical core was counted, the firmware memory map was discovered
//! and sums to real usable RAM, the SDRAM clock was reported (or honestly marked
//! unknown), the heap satisfies both a byte and a page allocation and reports its
//! static capacity, the log ring has capacity and captured the boot lines, the
//! timer frequency read non-zero, and the monotonic counter advanced across the
//! suite. Nothing here hardcodes success; every verdict has a real `FAIL` (or
//! `unknown`) branch, unit-tested in `diagnostics_tests`.
//!
//! ## Why the message is carried in the event id
//!
//! A `DS12Event` carries an `&'static str` event id and a fixed structured
//! field set, not a free-text message — `render_event` builds the boot-stage
//! message from the stage number the same way. A health line's metric is
//! runtime-formatted, so it is written (behind the `health.` family prefix)
//! into a shared static line buffer and handed across as the event id;
//! `render_event` maps the `health.` prefix to the `health` subsystem and
//! strips it for the message. This keeps `DS12Event` unchanged and reuses the
//! one rendering path.

// `unsafe` is confined to one shared static line buffer (`DIAG_LINE`), written
// by a single writer at the boot tail. This mirrors `log/flush.rs`'s panic
// mirror: a fixed static array reached only via `&raw mut`/`&raw const` to
// satisfy `static_mut_refs`, with the single-writer invariant documented at the
// one `unsafe` block. The workspace lint stays `warn`; the allow is scoped here.
#![allow(unsafe_code)]

use core::fmt::Write as _;

use {
    reovim_lib_ds::Bytes,
    reovim_uapi::{abi::error::LogLevel, system::BootInfo},
};

use crate::{
    BootClock,
    event_bus::{BootStageFields, DS12Event, DS12EventBus},
    log::{render::FixedWriter, ring::LogRing},
};

/// Bytes per binary mebibyte, for the memory metric.
const MIB: u64 = 1 << 20;

/// The byte the small-heap probe allocates — proves the alloc seam yields any
/// memory at all.
const HEAP_PROBE_SMALL: &[u8] = b"x";

/// The page the larger-heap probe allocates — a small alloc can succeed while a
/// page-sized one fails (arena near exhaustion), so it is a distinct check.
const HEAP_PROBE_PAGE: [u8; 4096] = [0u8; 4096];

/// Capacity of the shared health-line buffer. A health line is one short metric
/// string ("health." prefix + body); every body here is well under this bound.
/// An over-long line is dropped whole.
const DIAG_LINE_CAP: usize = 128;

/// Shared scratch buffer holding the current health line as a `'static` event
/// id for the duration of its `emit`.
///
/// Single-writer-at-boot: the sole writer is [`emit_health`], reached only from
/// [`run_at_boot_tail`] on the single-threaded `Init::boot` path. The buffer is
/// reused per line — each `emit` copies the line into the ring's owned `Bytes`
/// synchronously before the next call overwrites it — so no concurrent access
/// exists. (Mirrors the `log/flush.rs` panic-mirror static.)
static mut DIAG_LINE: [u8; DIAG_LINE_CAP] = [0u8; DIAG_LINE_CAP];

/// Formats `args` into `buf` with no allocation, returning the written `&str`
/// (or `None` if it would overflow `buf`).
fn fmt<'a>(buf: &'a mut [u8], args: core::fmt::Arguments<'_>) -> Option<&'a str> {
    let mut w = FixedWriter::new(buf);
    w.write_fmt(args).ok()?;
    Some(w.finish())
}

/// Emits one `health.<body>` line through the bus, riding the live console.
///
/// `body` is the already-formatted metric (e.g. `"mem.usable: 1024 MiB"`); the
/// `health.` family prefix is prepended so `render_event` tags the `health`
/// subsystem and strips it back off for the message.
fn emit_health(bus: &DS12EventBus, clock: &BootClock, body: &str) {
    const FAMILY: &str = "health.";
    let total = FAMILY.len() + body.len();
    if total > DIAG_LINE_CAP {
        return; // a line that overflows the buffer is dropped whole, never clipped
    }

    // SAFETY: single writer at the boot tail (see `DIAG_LINE`). The static is
    // reached only through `&raw mut`/`&raw const` raw pointers (no `&mut`
    // reference, per the arch `static_mut_refs` convention): write `total` bytes
    // — `FAMILY` (ASCII) then `body` (a valid `&str`), so the result is valid
    // UTF-8 — then read them back as a `&'static str`. The bytes stay unchanged
    // until this `emit` returns and the next `emit_health` overwrites them.
    let event: &'static str = unsafe {
        let dst: *mut u8 = (&raw mut DIAG_LINE).cast::<u8>();
        core::ptr::copy_nonoverlapping(FAMILY.as_ptr(), dst, FAMILY.len());
        core::ptr::copy_nonoverlapping(body.as_ptr(), dst.add(FAMILY.len()), body.len());
        let src: *const u8 = (&raw const DIAG_LINE).cast::<u8>();
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(src, total))
    };

    bus.emit(&DS12Event {
        ts_nanos: clock.elapsed_nanos(),
        level: LogLevel::Info,
        event,
        fields: BootStageFields {
            stage: 0,
            error_code: None,
        },
    });
}

/// Emits the `"<key>: probing..."` line that opens a ping-pong probe — the
/// announce half, sent before the verdict's work runs.
fn probe_start(bus: &DS12EventBus, clock: &BootClock, buf: &mut [u8], key: &str) {
    if let Some(line) = fmt(buf, format_args!("{key}: probing...")) {
        emit_health(bus, clock, line);
    }
}

// ── Verdict formatters (pure; the FAIL/edge branches are unit-tested) ─────────
//
// Each takes the probe `key` so the verdict line carries the same key as its
// `probing...` line, and embeds its own genuine `FAIL` condition.

/// CPU identification register (`MIDR_EL1` on ARM, CPUID on x86). `FAIL` when it
/// reads zero — a register that never got populated by discovery.
pub(crate) fn cpu_id_msg<'a>(buf: &'a mut [u8], key: &str, cpu_id: u32) -> Option<&'a str> {
    if cpu_id == 0 {
        fmt(buf, format_args!("{key}: FAIL (no id register)"))
    } else {
        fmt(buf, format_args!("{key}: {cpu_id:#010x}"))
    }
}

/// Logical core count. `FAIL` at zero — discovery must find at least the boot
/// core it is running on.
pub(crate) fn cpu_cores_msg<'a>(buf: &'a mut [u8], key: &str, count: u32) -> Option<&'a str> {
    if count == 0 {
        return fmt(buf, format_args!("{key}: FAIL (no cores)"));
    }
    let unit = if count == 1 { "core" } else { "cores" };
    fmt(buf, format_args!("{key}: {count} {unit}"))
}

/// Memory-map presence: the count of discovered ranges. `FAIL` on an empty map
/// (a failed firmware query, or a hosted build with no map).
pub(crate) fn mem_map_msg<'a>(buf: &'a mut [u8], key: &str, ranges: usize) -> Option<&'a str> {
    if ranges == 0 {
        return fmt(buf, format_args!("{key}: FAIL (no map)"));
    }
    let unit = if ranges == 1 { "range" } else { "ranges" };
    fmt(buf, format_args!("{key}: {ranges} {unit}"))
}

/// Usable RAM total in MiB. `unknown` when no map was discovered; `FAIL` when a
/// map exists but sums to zero usable bytes (a map with no `Usable` range).
pub(crate) fn mem_usable_msg<'a>(
    buf: &'a mut [u8],
    key: &str,
    usable_bytes: u64,
    present: bool,
) -> Option<&'a str> {
    if !present {
        fmt(buf, format_args!("{key}: unknown"))
    } else if usable_bytes == 0 {
        fmt(buf, format_args!("{key}: FAIL (0 usable)"))
    } else {
        fmt(buf, format_args!("{key}: {} MiB", usable_bytes / MIB))
    }
}

/// Heap probe verdict: whether a probe allocation through the `lib/ds` alloc
/// seam succeeded. `FAIL` when the seam refused the allocation.
pub(crate) fn heap_msg<'a>(buf: &'a mut [u8], key: &str, ok: bool) -> Option<&'a str> {
    if ok {
        fmt(buf, format_args!("{key}: ok"))
    } else {
        fmt(buf, format_args!("{key}: FAIL (alloc refused)"))
    }
}

/// Log-ring capacity in slots. `FAIL` at zero — a ring that could capture
/// nothing.
pub(crate) fn log_capacity_msg<'a>(buf: &'a mut [u8], key: &str, cap: usize) -> Option<&'a str> {
    if cap == 0 {
        fmt(buf, format_args!("{key}: FAIL (no capacity)"))
    } else {
        fmt(buf, format_args!("{key}: {cap} slots"))
    }
}

/// Log-ring occupancy: live entries captured so far. `FAIL` when empty (the
/// boot lines never reached the ring) or when `len` exceeds capacity (corrupt
/// accounting).
pub(crate) fn log_occupancy_msg<'a>(
    buf: &'a mut [u8],
    key: &str,
    len: usize,
    cap: usize,
) -> Option<&'a str> {
    if len == 0 || len > cap {
        fmt(buf, format_args!("{key}: FAIL ({len}/{cap})"))
    } else {
        fmt(buf, format_args!("{key}: {len} entries"))
    }
}

/// Timer frequency in MHz (`CNTFRQ_EL0` on ARM). `FAIL` at zero — an unreadable
/// or unconfigured counter frequency register.
pub(crate) fn clock_freq_msg<'a>(buf: &'a mut [u8], key: &str, freq_hz: u64) -> Option<&'a str> {
    if freq_hz == 0 {
        fmt(buf, format_args!("{key}: FAIL (no timer freq)"))
    } else {
        fmt(buf, format_args!("{key}: {} MHz", freq_hz / 1_000_000))
    }
}

/// Monotonic-counter verdict: whether the counter advanced across the suite,
/// with the elapsed microseconds. `FAIL` on a frozen counter (`!advanced`).
pub(crate) fn clock_monotonic_msg<'a>(
    buf: &'a mut [u8],
    key: &str,
    advanced: bool,
    delta_ns: u64,
) -> Option<&'a str> {
    if advanced {
        fmt(buf, format_args!("{key}: advanced {} us", delta_ns / 1_000))
    } else {
        fmt(buf, format_args!("{key}: FAIL (frozen)"))
    }
}

/// Maps a `MIDR_EL1`/CPUID implementer byte to a vendor name, or `None` for an
/// implementer this decoder does not recognize.
///
/// The implementer byte is `cpu_id[31:24]`. The list is the registered ARM
/// implementer codes; an unrecognized byte is reported as `unknown (0xNN)`
/// rather than guessed.
pub(crate) const fn implementer_name(byte: u8) -> Option<&'static str> {
    match byte {
        0x41 => Some("ARM"),
        0x42 => Some("Broadcom"),
        0x43 => Some("Cavium"),
        0x44 => Some("DEC"),
        0x46 => Some("Fujitsu"),
        0x48 => Some("HiSilicon"),
        0x4E => Some("NVIDIA"),
        0x50 => Some("Ampere"),
        0x51 => Some("Qualcomm"),
        0x56 => Some("Marvell"),
        0x69 => Some("Intel"),
        _ => None,
    }
}

/// CPU vendor, decoded from the implementer byte of the existing `cpu_id`
/// (`MIDR_EL1[31:24]`). An unrecognized implementer renders `unknown (0xNN)` —
/// informational, not `FAIL` (a zero id is already caught by the `cpu.id` probe).
pub(crate) fn cpu_vendor_msg<'a>(buf: &'a mut [u8], key: &str, cpu_id: u32) -> Option<&'a str> {
    #[allow(clippy::cast_possible_truncation)]
    let implementer = (cpu_id >> 24) as u8;
    match implementer_name(implementer) {
        Some(name) => fmt(buf, format_args!("{key}: {name}")),
        None => fmt(buf, format_args!("{key}: unknown ({implementer:#04x})")),
    }
}

/// CPU cache geometry: minimum line size plus L1-D / L1-I / L2 sizes in KiB.
/// `FAIL` when the line size is zero (`CTR_EL0` unreadable); an absent level
/// reports `0 KiB` (its `CLIDR_EL1` `Ctype` was empty), which is honest.
// `l1d_bytes`/`l1i_bytes` differ by one letter by design (the cache they name
// does), so the similar-names heuristic does not apply.
#[allow(clippy::similar_names)]
pub(crate) fn cpu_cache_msg<'a>(
    buf: &'a mut [u8],
    key: &str,
    line_bytes: u32,
    l1d_bytes: u32,
    l1i_bytes: u32,
    l2_bytes: u32,
) -> Option<&'a str> {
    if line_bytes == 0 {
        return fmt(buf, format_args!("{key}: FAIL (no cache info)"));
    }
    fmt(
        buf,
        format_args!(
            "{key}: line {line_bytes} B, L1d {} KiB, L1i {} KiB, L2 {} KiB",
            l1d_bytes >> 10,
            l1i_bytes >> 10,
            l2_bytes >> 10,
        ),
    )
}

/// CPU affinity from `MPIDR_EL1`. `FAIL` when bit 31 (RES1 on `ARMv8`) is clear —
/// a malformed or unreadable register; otherwise the affinity value in hex.
pub(crate) fn cpu_affinity_msg<'a>(buf: &'a mut [u8], key: &str, affinity: u64) -> Option<&'a str> {
    if affinity & (1 << 31) == 0 {
        fmt(buf, format_args!("{key}: FAIL (bad MPIDR)"))
    } else {
        // The Aff0..Aff3 topology bytes; mask off the RES1/U/MT flag bits above
        // Aff2 so the reported value is the affinity proper.
        let aff = affinity & 0x0000_00FF_00FF_FFFF;
        fmt(buf, format_args!("{key}: {aff:#x}"))
    }
}

/// Memory (SDRAM) clock in MHz. `unknown` when zero — the firmware did not
/// report the clock (QEMU's model may not implement the tag), never fabricated.
pub(crate) fn mem_freq_msg<'a>(buf: &'a mut [u8], key: &str, freq_hz: u64) -> Option<&'a str> {
    if freq_hz == 0 {
        fmt(buf, format_args!("{key}: unknown"))
    } else {
        fmt(buf, format_args!("{key}: {} MHz", freq_hz / 1_000_000))
    }
}

/// Heap (page-arena) total capacity. `FAIL` at zero — capacity was never
/// reported; otherwise the size in MiB (when a whole mebibyte or larger) or KiB.
pub(crate) fn heap_total_msg<'a>(buf: &'a mut [u8], key: &str, bytes: u64) -> Option<&'a str> {
    if bytes == 0 {
        fmt(buf, format_args!("{key}: FAIL (no capacity)"))
    } else if bytes >= MIB {
        fmt(buf, format_args!("{key}: {} MiB", bytes / MIB))
    } else {
        fmt(buf, format_args!("{key}: {} KiB", bytes >> 10))
    }
}

/// Sums the `Usable` ranges of `boot_info`'s memory map. Returns `(bytes,
/// present)`; `present` is `false` for an empty map so the memory line reports
/// `unknown` rather than a misleading `0 MiB`.
pub(crate) fn usable_memory(boot_info: &BootInfo) -> (u64, bool) {
    if boot_info.memory.is_empty() {
        return (0, false);
    }
    (boot_info.memory.usable_bytes(), true)
}

// ── Boot-tail entry point ─────────────────────────────────────────────────────

/// Emits the hardware banner and the health-probe suite at the boot tail.
///
/// Called once from `Init::boot` after the stages run, single-threaded. Each
/// probe is two lines — `probing...` then verdict — emitted through `bus` so it
/// rides the built-in log ring and the live console echo. The probing line
/// precedes its work (alloc, second clock read), giving the slow-mode boot a
/// visible back-and-forth. `t0` is read first so the closing monotonic probe
/// can confirm the counter advanced across the whole suite.
pub(crate) fn run_at_boot_tail(
    bus: &DS12EventBus,
    clock: &BootClock,
    ring: &LogRing,
    boot_info: &BootInfo,
) {
    let t0 = clock.elapsed_nanos();
    let mut buf = [0u8; DIAG_LINE_CAP];

    probe_start(bus, clock, &mut buf, "cpu.id");
    if let Some(line) = cpu_id_msg(&mut buf, "cpu.id", boot_info.cpu_id) {
        emit_health(bus, clock, line);
    }

    probe_start(bus, clock, &mut buf, "cpu.cores");
    if let Some(line) = cpu_cores_msg(&mut buf, "cpu.cores", boot_info.cpu_count) {
        emit_health(bus, clock, line);
    }

    let (usable, present) = usable_memory(boot_info);
    probe_start(bus, clock, &mut buf, "mem.map");
    if let Some(line) = mem_map_msg(&mut buf, "mem.map", boot_info.memory.range_count()) {
        emit_health(bus, clock, line);
    }

    probe_start(bus, clock, &mut buf, "mem.usable");
    if let Some(line) = mem_usable_msg(&mut buf, "mem.usable", usable, present) {
        emit_health(bus, clock, line);
    }

    probe_start(bus, clock, &mut buf, "heap.small");
    let small_ok = Bytes::try_from_slice(HEAP_PROBE_SMALL).is_ok();
    if let Some(line) = heap_msg(&mut buf, "heap.small", small_ok) {
        emit_health(bus, clock, line);
    }

    probe_start(bus, clock, &mut buf, "heap.page");
    let page_ok = Bytes::try_from_slice(&HEAP_PROBE_PAGE).is_ok();
    if let Some(line) = heap_msg(&mut buf, "heap.page", page_ok) {
        emit_health(bus, clock, line);
    }

    probe_start(bus, clock, &mut buf, "log.capacity");
    if let Some(line) = log_capacity_msg(&mut buf, "log.capacity", ring.capacity()) {
        emit_health(bus, clock, line);
    }

    probe_start(bus, clock, &mut buf, "log.occupancy");
    if let Some(line) = log_occupancy_msg(&mut buf, "log.occupancy", ring.len(), ring.capacity()) {
        emit_health(bus, clock, line);
    }

    probe_start(bus, clock, &mut buf, "clock.freq");
    if let Some(line) = clock_freq_msg(&mut buf, "clock.freq", boot_info.cpu_freq_hz) {
        emit_health(bus, clock, line);
    }

    probe_start(bus, clock, &mut buf, "cpu.vendor");
    if let Some(line) = cpu_vendor_msg(&mut buf, "cpu.vendor", boot_info.cpu_id) {
        emit_health(bus, clock, line);
    }

    probe_start(bus, clock, &mut buf, "cpu.cache");
    if let Some(line) = cpu_cache_msg(
        &mut buf,
        "cpu.cache",
        boot_info.cache_line_bytes,
        boot_info.l1d_bytes,
        boot_info.l1i_bytes,
        boot_info.l2_bytes,
    ) {
        emit_health(bus, clock, line);
    }

    probe_start(bus, clock, &mut buf, "cpu.affinity");
    if let Some(line) = cpu_affinity_msg(&mut buf, "cpu.affinity", boot_info.cpu_affinity) {
        emit_health(bus, clock, line);
    }

    probe_start(bus, clock, &mut buf, "mem.freq");
    if let Some(line) = mem_freq_msg(&mut buf, "mem.freq", boot_info.mem_freq_hz) {
        emit_health(bus, clock, line);
    }

    probe_start(bus, clock, &mut buf, "heap.total");
    if let Some(line) = heap_total_msg(&mut buf, "heap.total", boot_info.heap_total_bytes) {
        emit_health(bus, clock, line);
    }

    probe_start(bus, clock, &mut buf, "clock.monotonic");
    let t1 = clock.elapsed_nanos();
    if let Some(line) =
        clock_monotonic_msg(&mut buf, "clock.monotonic", t1 > t0, t1.saturating_sub(t0))
    {
        emit_health(bus, clock, line);
    }
}
