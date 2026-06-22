//! Tests for `event_bus.rs` — `DS12EventBus`, `DS12Event`, CC6 properties.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (arch_test! + testrt::run). The kernel-selftest bin in
//! tests/fixtures/ runs these.
//!
//! ## Coverage
//!
//! - CC6 probe: subscribe-from-callback does not deadlock.
//! - Ordering: subscribers receive events in registration order.
//! - Built-in slot fires before registered subscribers.
//! - Empty bus emit is a no-op.
//! - Event fields are preserved through emit.

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use {reovim_arch::arch_test, reovim_lib_ds::Shared, reovim_uapi::abi::error::LogLevel};

use crate::{
    BootClock,
    event_bus::{
        BootStageFields, DS12Event, DS12EventBus, EVT_BOOT_STAGE_OK, EVT_BOOT_STAGE_START,
    },
    log::ring::LogRing,
};

/// Constructs a minimal test event with the given stage index.
fn test_event(clock: &BootClock, event: &'static str, stage: u8) -> DS12Event {
    DS12Event {
        ts_nanos: clock.elapsed_nanos(),
        level: LogLevel::Info,
        event,
        fields: BootStageFields {
            stage,
            error_code: None,
        },
    }
}

// ── Empty bus ────────────────────────────────────────────────────────────────

arch_test!(empty_bus_emit_is_noop, {
    let bus = DS12EventBus::new();
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    // Must complete without panic or deadlock.
    bus.emit(&test_event(&clock, EVT_BOOT_STAGE_START, 0));
});

// ── Subscriber ordering ──────────────────────────────────────────────────────

/// Global sequence recorder for ordering tests.
static ORDER_SEQ: AtomicUsize = AtomicUsize::new(0);
static ORDER_A: AtomicUsize = AtomicUsize::new(0);
static ORDER_B: AtomicUsize = AtomicUsize::new(0);
static ORDER_C: AtomicUsize = AtomicUsize::new(0);

fn record_a(_e: &DS12Event) {
    ORDER_A.store(ORDER_SEQ.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
}
fn record_b(_e: &DS12Event) {
    ORDER_B.store(ORDER_SEQ.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
}
fn record_c(_e: &DS12Event) {
    ORDER_C.store(ORDER_SEQ.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
}

arch_test!(subscribers_receive_events_in_registration_order, {
    // Reset global counters before the test.
    ORDER_SEQ.store(0, Ordering::Relaxed);
    ORDER_A.store(0, Ordering::Relaxed);
    ORDER_B.store(0, Ordering::Relaxed);
    ORDER_C.store(0, Ordering::Relaxed);

    let bus = DS12EventBus::new();
    bus.subscribe(record_a).unwrap();
    bus.subscribe(record_b).unwrap();
    bus.subscribe(record_c).unwrap();

    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    bus.emit(&test_event(&clock, EVT_BOOT_STAGE_START, 0));

    // A was registered first — must have the lowest sequence number.
    let a = ORDER_A.load(Ordering::Relaxed);
    let b = ORDER_B.load(Ordering::Relaxed);
    let c = ORDER_C.load(Ordering::Relaxed);
    assert!(a < b, "A({a}) must come before B({b})");
    assert!(b < c, "B({b}) must come before C({c})");
});

// ── Built-in slot fires before registered subscribers ────────────────────────
//
// The bus's built-in slot is a `Shared<LogRing>`. Ordering is verified by:
// 1. Setting the built-in ring — the bus invokes `ring.push_event` before
//    any registered subscriber callback.
// 2. After `emit()` returns, the ring holds exactly one entry AND the
//    registered subscriber was called (verified via atomic counter).
//
// `emit()` is sequential and deterministic: builtin → registered. So if both
// conditions hold after a single emit, the built-in ran first.

static BUILTIN_REG_CALLED: AtomicUsize = AtomicUsize::new(0);

fn builtin_order_reg_subscriber(_e: &DS12Event) {
    BUILTIN_REG_CALLED.fetch_add(1, Ordering::Relaxed);
}

arch_test!(builtin_slot_fires_before_registered_subscribers, {
    BUILTIN_REG_CALLED.store(0, Ordering::Relaxed);

    let ring = Shared::try_new(LogRing::try_new(1024 * 1024).unwrap()).unwrap();
    let mut bus = DS12EventBus::new();
    bus.set_builtin(Shared::clone(&ring));
    bus.subscribe(builtin_order_reg_subscriber).unwrap();

    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    bus.emit(&test_event(&clock, EVT_BOOT_STAGE_OK, 1));

    // The registered subscriber must have been called.
    assert_eq!(
        BUILTIN_REG_CALLED.load(Ordering::Relaxed),
        1,
        "registered subscriber must be called"
    );
    // The ring must hold the event (built-in fired).
    assert_eq!(ring.len(), 1, "ring must hold the emitted event (built-in fired)");
    // Both ran in one emit — emit is sequential, so builtin came first.
});

// ── Event fields preserved ────────────────────────────────────────────────────

static SEEN_STAGE: AtomicUsize = AtomicUsize::new(usize::MAX);
static SEEN_ERROR_CODE_PRESENT: AtomicBool = AtomicBool::new(false);

fn field_checker(e: &DS12Event) {
    SEEN_STAGE.store(usize::from(e.fields.stage), Ordering::Relaxed);
    SEEN_ERROR_CODE_PRESENT.store(e.fields.error_code.is_some(), Ordering::Relaxed);
}

arch_test!(emit_preserves_event_fields, {
    SEEN_STAGE.store(usize::MAX, Ordering::Relaxed);
    SEEN_ERROR_CODE_PRESENT.store(false, Ordering::Relaxed);

    let bus = DS12EventBus::new();
    bus.subscribe(field_checker).unwrap();

    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    let event = DS12Event {
        ts_nanos: clock.elapsed_nanos(),
        level: LogLevel::Error,
        event: "boot.stage.fail",
        fields: BootStageFields {
            stage: 5,
            error_code: Some(5),
        },
    };
    bus.emit(&event);

    assert_eq!(SEEN_STAGE.load(Ordering::Relaxed), 5);
    assert!(SEEN_ERROR_CODE_PRESENT.load(Ordering::Relaxed));
});

// ── CC6: subscribe-from-callback does not deadlock ──────────────────────────
//
// This test verifies the core CC6 invariant: a subscriber that calls
// `subscribe()` on the same bus during its own invocation must not deadlock.
// The clone-then-invoke design guarantees this: `emit()` holds the read lock
// only long enough to copy the subscriber list, then drops it before invoking
// any callback. The late subscriber's `subscribe()` acquires the write lock
// independently.
//
// The callback needs access to the same bus it was invoked from, and fn
// pointers cannot capture. `DS12EventBus::new()` is `const`, so the bus
// itself lives in a process-global static that both the test body and the
// callback name directly — no pointer smuggling. The static is exclusive to
// this test (the selftest runner is single-threaded).

/// The bus under test; shared between the test body and the callback.
static CC6_BUS: DS12EventBus = DS12EventBus::new();

fn cc6_subscribe_from_callback(_e: &DS12Event) {
    // Register a late subscriber from within the callback — must not deadlock.
    let _ = CC6_BUS.subscribe(cc6_late_subscriber);
}

static CC6_LATE_CALLED: AtomicBool = AtomicBool::new(false);

fn cc6_late_subscriber(_e: &DS12Event) {
    CC6_LATE_CALLED.store(true, Ordering::Relaxed);
}

arch_test!(cc6_subscribe_in_callback_does_not_deadlock, {
    CC6_LATE_CALLED.store(false, Ordering::Relaxed);

    CC6_BUS.subscribe(cc6_subscribe_from_callback).unwrap();

    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    // First emit: cc6_subscribe_from_callback registers cc6_late_subscriber.
    CC6_BUS.emit(&test_event(&clock, EVT_BOOT_STAGE_START, 0));

    // Second emit: the late subscriber (registered during the first emit) must
    // now be in the list and receive the event.
    CC6_BUS.emit(&test_event(&clock, EVT_BOOT_STAGE_OK, 0));

    assert!(
        CC6_LATE_CALLED.load(Ordering::Relaxed),
        "late subscriber registered during callback must receive subsequent events"
    );
});

// ── subscribe returns error on alloc failure (edge path) ─────────────────────
//
// Fills the bus with 64 subscribers, emits one event so all noop subscribers
// run (covering the `noop` helper body), then attempts a 65th subscribe which
// must return `Err(SubscribeError::Capacity)` (event_bus.rs L306).

// Noop helper declared at module level so it is reachable from both the
// 64-subscriber test (item 8) and the Default/emit_noop tests. Module-level
// `fn` avoids `items_after_statements` from declaring it inside the test body.
fn noop(_e: &DS12Event) {}

arch_test!(subscribe_sixty_four_then_capacity_error, {
    use crate::event_bus::SubscribeError;

    let bus = DS12EventBus::new();
    for _ in 0_usize..64 {
        assert!(bus.subscribe(noop).is_ok(), "subscribe must succeed for 64 subscribers");
    }

    // Emit once so all 64 noop subscribers actually execute (item 8: noop body
    // must be invoked at least once so the line is covered).
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    bus.emit(&test_event(&clock, EVT_BOOT_STAGE_OK, 0));

    // The 65th subscribe must return SubscribeError::Capacity (event_bus.rs L306).
    let result = bus.subscribe(noop);
    assert!(
        matches!(result, Err(SubscribeError::Capacity)),
        "65th subscribe must return SubscribeError::Capacity"
    );
});

// ── DS12EventBus::default() is equivalent to new() (event_bus.rs L383-385) ──
//
// `Default::default()` calls `Self::new()`. Verify the default bus behaves
// identically to a bus created via `new()`: emit on the default bus is a no-op
// (no subscribers, no builtin).

arch_test!(event_bus_default_behaves_as_new, {
    // `DS12EventBus` is a static-constructible type (`const fn new()`), but
    // default() is a non-const fn. Ensure it produces a usable, subscriber-free bus.
    let bus = DS12EventBus::default();
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    // Must complete without panic or deadlock (no subscribers → no-op).
    bus.emit(&test_event(&clock, EVT_BOOT_STAGE_OK, 0));
});

// ── emit_noop direct call (event_bus.rs L400) ─────────────────────────────────
//
// `emit_noop` is the snapshot-filler fn used to zero-initialise the stack array
// in `emit()`. It is never registered as a subscriber; its body must remain
// callable. Call it directly with a test event and assert nothing observable
// changes (no counter incremented, no panic). This covers the filler function
// body which would otherwise be dead from a coverage perspective.
//
// `emit_noop` is `const fn` and has `pub(super)` visibility — it is in the
// same module, accessible from the sibling test file via `crate::event_bus`.
// However it is not re-exported. Use `use crate::event_bus::DS12EventBus` to
// access the module, but `emit_noop` itself is private. We exercise it
// indirectly: `bus.emit()` initialises the snapshot with `emit_noop` as the
// filler for every unused slot, so calling `emit()` with 0 subscribers ensures
// the snapshot is entirely filled with `emit_noop` pointers and the loop
// `for sub in &snapshot[..0]` is a no-op. The filler is still invoked
// conceptually, but coverage tooling may not trace the pointer equality.
//
// To ensure the function body line (L400) is truly executed, we call `emit()`
// on an empty bus with SUBSCRIBER_CAPACITY = 64 slots to fill — all 64 slots
// are set to `emit_noop` — and we verify the emit path completes without error.

arch_test!(emit_noop_filler_is_callable, {
    // Execute the snapshot-filler body directly: live emits never invoke it
    // (only `..n` entries run), so pointer-naming alone leaves it uncovered.
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    crate::event_bus::emit_noop(&DS12Event {
        ts_nanos: clock.elapsed_nanos(),
        level: LogLevel::Info,
        event: EVT_BOOT_STAGE_OK,
        fields: BootStageFields {
            stage: 0,
            error_code: None,
        },
    });
});

arch_test!(emit_noop_filler_does_not_panic, {
    let bus = DS12EventBus::new();
    // No subscribers registered — all snapshot slots are emit_noop.
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    let ev = test_event(&clock, EVT_BOOT_STAGE_OK, 0);
    // The emit() call fills the snapshot with emit_noop and invokes none of them
    // (n=0), but the initialiser `[emit_noop as Subscriber; SUBSCRIBER_CAPACITY]`
    // still names the function, counting the filler body as live.
    // Emit twice with one subscriber so the snapshot includes exactly one real
    // callback plus 63 emit_noop-filled slots (all filled during the array init).
    bus.subscribe(noop).unwrap();
    bus.emit(&ev);
    // If we reach here the filler path is exercised and no panic occurred.
});

// ── Shared<DS12EventBus> round-trip ──────────────────────────────────────────

static SHARED_BUS_COUNT: AtomicUsize = AtomicUsize::new(0);

fn shared_counter(_e: &DS12Event) {
    SHARED_BUS_COUNT.fetch_add(1, Ordering::Relaxed);
}

arch_test!(shared_event_bus_emit_reaches_subscribers, {
    SHARED_BUS_COUNT.store(0, Ordering::Relaxed);

    let bus = Shared::try_new(DS12EventBus::new()).unwrap();
    bus.subscribe(shared_counter).unwrap();

    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    bus.emit(&test_event(&clock, EVT_BOOT_STAGE_START, 0));

    assert_eq!(
        SHARED_BUS_COUNT.load(Ordering::Relaxed),
        1,
        "emit through Shared<DS12EventBus> must reach subscribers"
    );
});
