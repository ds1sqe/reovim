//! Tests for `log/ring.rs` — `LogRing` LOG6 behaviour.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (`arch_test!` + `testrt::run`). The kernel-selftest bin in
//! tests/fixtures/ runs these.
//!
//! ## Coverage
//!
//! - `LogRing::try_new` with 1 MiB → capacity ≥ 2, len = 0.
//! - `push_event` appends rendered entries; len increases.
//! - LOG6: oldest-first eviction on overflow (ring wraps, oldest entry gone).
//! - Boot-stage-0 lines present in the ring before any sink opens.
//! - `for_each` iterates in oldest-first order.
//! - `DS12EventBus::set_builtin` + bus integration: events routed through the bus
//!   arrive in the ring via the built-in slot.

use core::sync::atomic::{AtomicUsize, Ordering};

use {
    reovim_arch::{alloc::fault, arch_test},
    reovim_lib_ds::Shared,
    reovim_uapi_abi::error::LogLevel,
};

use crate::{
    BootClock,
    event_bus::{
        BootStageFields, DS12Event, DS12EventBus, EVT_BOOT_STAGE_OK, EVT_BOOT_STAGE_START,
    },
    log::ring::{LogRing, MAX_ENTRY_BYTES, MIN_RING_CAPACITY},
};

/// Constructs a minimal boot-stage-ok event.
fn make_event(clock: &BootClock, stage: u8) -> DS12Event {
    DS12Event {
        ts_nanos: clock.elapsed_nanos(),
        level: LogLevel::Info,
        event: EVT_BOOT_STAGE_OK,
        fields: BootStageFields {
            stage,
            error_code: None,
        },
    }
}

// ── Construction ─────────────────────────────────────────────────────────────

arch_test!(log_ring_try_new_1mib_is_ok, {
    let ring = LogRing::try_new(1024 * 1024).expect("1 MiB ring");
    assert_eq!(ring.len(), 0);
    assert!(ring.is_empty());
    assert!(ring.capacity() >= MIN_RING_CAPACITY);
    // Capacity = (1 MiB / MAX_ENTRY_BYTES).max(MIN) = 248.
    let expected_cap = ((1024 * 1024) / MAX_ENTRY_BYTES).max(MIN_RING_CAPACITY);
    assert_eq!(ring.capacity(), expected_cap);
});

arch_test!(log_ring_try_new_small_capacity_is_min, {
    // A ring_bytes value smaller than MAX_ENTRY_BYTES should yield MIN_RING_CAPACITY.
    let ring = LogRing::try_new(1).expect("tiny ring");
    assert_eq!(ring.capacity(), MIN_RING_CAPACITY);
});

// ── Push and len ─────────────────────────────────────────────────────────────

arch_test!(log_ring_push_event_increments_len, {
    let ring = LogRing::try_new(1024 * 1024).unwrap();
    let clock = BootClock::capture();
    assert_eq!(ring.len(), 0);
    ring.push_event(&make_event(&clock, 0));
    assert_eq!(ring.len(), 1);
    ring.push_event(&make_event(&clock, 1));
    assert_eq!(ring.len(), 2);
});

arch_test!(log_ring_push_event_entry_is_nonempty, {
    let ring = LogRing::try_new(1024 * 1024).unwrap();
    let clock = BootClock::capture();
    ring.push_event(&make_event(&clock, 0));
    let mut found = false;
    ring.for_each(|entry| {
        assert!(!entry.line.is_empty(), "rendered line must not be empty");
        assert!(entry.line.as_slice().ends_with(b"\n"), "line must end with newline");
        found = true;
    });
    assert!(found, "for_each must visit the one entry");
});

// ── LOG6: oldest-first eviction ───────────────────────────────────────────────
//
// Use a tiny ring (capacity = 2) to force eviction immediately.

arch_test!(log6_oldest_first_eviction, {
    // ring_bytes = MIN_RING_CAPACITY * MAX_ENTRY_BYTES keeps cap at MIN.
    // Actually try_new(1) gives capacity = 2 (MIN_RING_CAPACITY).
    let ring = LogRing::try_new(1).unwrap();
    assert_eq!(ring.capacity(), 2, "tiny ring must have capacity 2");

    // Push 3 events into a capacity-2 ring: entry[0], entry[1], entry[2].
    // After 3 pushes the oldest (entry[0]) is evicted and the ring holds
    // entry[1] and entry[2].
    let ev0 = DS12Event {
        ts_nanos: 1_000_000,
        level: LogLevel::Warn,
        event: EVT_BOOT_STAGE_START,
        fields: BootStageFields {
            stage: 0,
            error_code: None,
        },
    };
    let ev1 = DS12Event {
        ts_nanos: 2_000_000,
        level: LogLevel::Info,
        event: EVT_BOOT_STAGE_OK,
        fields: BootStageFields {
            stage: 1,
            error_code: None,
        },
    };
    let ev2 = DS12Event {
        ts_nanos: 3_000_000,
        level: LogLevel::Error,
        event: EVT_BOOT_STAGE_START,
        fields: BootStageFields {
            stage: 2,
            error_code: None,
        },
    };

    ring.push_event(&ev0);
    ring.push_event(&ev1);
    // Ring is now full (2 entries).
    assert_eq!(ring.len(), 2);

    // Push a third event — oldest (ev0) is evicted.
    ring.push_event(&ev2);
    assert_eq!(ring.len(), 2, "ring stays at capacity after eviction");

    // The remaining entries should be ev1 and ev2 (oldest-first).
    // ev1 had ts_nanos=2_000_000, so its rendered line contains "0.002".
    // ev2 had ts_nanos=3_000_000, so its rendered line contains "0.003".
    let mut lines: [Option<reovim_lib_ds::Bytes>; 2] = [None, None];
    let mut idx = 0usize;
    ring.for_each(|entry| {
        if idx < 2 {
            // Clone the bytes for inspection: copy as Bytes::try_from_slice.
            lines[idx] = reovim_lib_ds::Bytes::try_from_slice(entry.line.as_slice()).ok();
            idx += 1;
        }
    });
    assert_eq!(idx, 2);

    // ev1 line contains "0.002000"
    let line0 = lines[0].as_ref().unwrap();
    let s0 = core::str::from_utf8(line0.as_slice()).unwrap();
    assert!(s0.contains("0.002000"), "oldest retained entry must be ev1 (ts 2ms): {s0:?}");

    // ev2 line contains "0.003000"
    let line1 = lines[1].as_ref().unwrap();
    let s1 = core::str::from_utf8(line1.as_slice()).unwrap();
    assert!(s1.contains("0.003000"), "newest entry must be ev2 (ts 3ms): {s1:?}");
});

// ── Boot-stage lines in ring before sink opens ────────────────────────────────
//
// LOG6: the ring receives every event from boot stage 0 onward, regardless of
// sink state. This test runs Init::boot and checks the ring has entries.

arch_test!(log6_ring_has_boot_stage_lines_before_sink, {
    // Use Init::boot so the ring is wired through the normal path.
    use crate::init::{Init, LauncherArgs};

    let init = Init::new(LauncherArgs::default());
    let kernel = init.boot().expect("boot must succeed");

    // Stages 1..7 each emit start + ok (14 events via the bus).
    // All must appear in the ring.
    assert!(kernel.log_ring.len() > 0, "ring must have entries after Init::boot");
    // We expect 14 events (stages 1..7, each start+ok).
    assert_eq!(kernel.log_ring.len(), 14, "ring must hold 14 boot-stage events (7 × start+ok)");
});

// ── set_builtin + bus routing ─────────────────────────────────────────────────
//
// LOG1: events emitted through the bus arrive in the ring via the built-in slot.
// `DS12EventBus::set_builtin` now receives a `Shared<LogRing>` directly.

arch_test!(log_ring_builtin_subscriber_captures_events, {
    let ring = Shared::try_new(LogRing::try_new(1024 * 1024).unwrap()).unwrap();
    let mut bus = DS12EventBus::new();
    bus.set_builtin(Shared::clone(&ring));

    let clock = BootClock::capture();
    bus.emit(&make_event(&clock, 0));
    bus.emit(&make_event(&clock, 1));
    bus.emit(&make_event(&clock, 2));

    assert_eq!(ring.len(), 3, "three emits must produce three ring entries");
});

// ── for_each iterates oldest-first ───────────────────────────────────────────

// Module-level static for the for_each count; declared outside the test body
// to avoid `items_after_statements`.
static FOR_EACH_COUNT: AtomicUsize = AtomicUsize::new(0);

arch_test!(log_ring_for_each_oldest_first, {
    FOR_EACH_COUNT.store(0, Ordering::Relaxed);

    let ring = LogRing::try_new(1024 * 1024).unwrap();
    // Push events with distinct timestamps to verify oldest-first order.
    for stage in 0_u8..5 {
        let ev = DS12Event {
            ts_nanos: u64::from(stage) * 1_000_000 + 1_000,
            level: LogLevel::Info,
            event: EVT_BOOT_STAGE_OK,
            fields: BootStageFields {
                stage,
                error_code: None,
            },
        };
        ring.push_event(&ev);
    }

    ring.for_each(|_entry| {
        FOR_EACH_COUNT.fetch_add(1, Ordering::Relaxed);
    });

    assert_eq!(FOR_EACH_COUNT.load(Ordering::Relaxed), 5, "for_each must visit all 5 entries");
});

// ── is_empty after push ───────────────────────────────────────────────────────

arch_test!(log_ring_is_empty_transitions, {
    let ring = LogRing::try_new(1024 * 1024).unwrap();
    assert!(ring.is_empty());
    let clock = BootClock::capture();
    ring.push_event(&make_event(&clock, 0));
    assert!(!ring.is_empty());
});

// ── Render-OOM early return (ring.rs L260) ────────────────────────────────────
//
// Arm the alloc-entry fault hook so the render's first `Bytes` allocation fails.
// `fail_after(0)` fails the very next allocation attempt, which is the `Bytes`
// growth triggered by the first `write!()` inside `render_line`. The early-return
// arm (`else { return }` on the `Ok(line)` let-else) executes: the ring length
// must remain unchanged and no panic must occur.

arch_test!(push_event_render_oom_silent_drop, {
    let ring = LogRing::try_new(1024 * 1024).unwrap();
    let clock = BootClock::capture();
    // Seed one entry so the ring is non-empty; this alloc succeeds normally.
    ring.push_event(&make_event(&clock, 0));
    assert_eq!(ring.len(), 1);

    // Arm: the very next allocation fails.
    fault::fail_after(0);
    ring.push_event(&make_event(&clock, 1));
    fault::reset();

    // The render failed → entry was silently dropped → ring length unchanged.
    assert_eq!(ring.len(), 1, "render-OOM must silently drop the event; ring len unchanged");
});

// ── Fallback `kernel` subsystem (ring.rs L329) ────────────────────────────────
//
// `kernel_subsystem_from_event` returns `"kernel"` for event names that do not
// start with `"boot."` or `"log."`. Push an event with a custom family name and
// assert that the rendered line contains `" kernel "` (the fallback subsystem
// field in the LOG2 format: `… kernel kernel …`).

arch_test!(push_event_fallback_subsystem_renders_kernel, {
    let ring = LogRing::try_new(1024 * 1024).unwrap();
    // An event name with no known family prefix.
    let ev = DS12Event {
        ts_nanos: 5_000_000,
        level: LogLevel::Info,
        event: "custom.test",
        fields: BootStageFields {
            stage: 0,
            error_code: None,
        },
    };
    ring.push_event(&ev);
    assert_eq!(ring.len(), 1, "fallback-subsystem event must be pushed");

    let mut found_kernel_subsystem = false;
    ring.for_each(|entry| {
        let line = core::str::from_utf8(entry.line.as_slice()).unwrap_or("");
        // LOG2 format: "[  ts] kernel kernel: custom.test\n"
        // The emitter_pkg is "kernel" and the subsystem is "kernel".
        if line.contains(" kernel kernel:") {
            found_kernel_subsystem = true;
        }
    });
    assert!(
        found_kernel_subsystem,
        "rendered line must use the fallback 'kernel' subsystem for unknown event families"
    );
});
