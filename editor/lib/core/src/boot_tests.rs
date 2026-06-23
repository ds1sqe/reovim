//! Tests for `boot.rs` — boot-stage driver, OBS1 event ordering, forced failure.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (arch_test! + testrt::run). The editor-core-selftest bin in
//! tests/fixtures/ runs these.
//!
//! ## Coverage
//!
//! - Subscriber ordering: `boot.stage.*` events arrive in stage order.
//! - Full-boot integration smoke: all 7 stage start/ok pairs for stages 1..7
//!   from a call to `run_boot_stages` with a recording subscriber.
//! - Forced-failure: injected failure emits `boot.stage.fail` at the target
//!   stage; boot returns `BootError::Stage`; no events fire after it.
//! - `EditorInit::boot` returns a `EditorCore` with a live `event_bus` after success.
//! - `EditorInit::boot` with an injected failure returns `Err(BootError::Stage)`.
//! - Stage-order invariant: start always precedes ok for the same stage.

use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use reovim_arch::arch_test;

use crate::{
    BootClock,
    boot::{inject_stage_failure, run_stage},
    event_bus::{
        DS12Event, DS12EventBus, EVT_BOOT_STAGE_FAIL, EVT_BOOT_STAGE_OK, EVT_BOOT_STAGE_START,
    },
    init::{BootError, EditorInit, LauncherArgs},
};

// ── Event recording infrastructure ──────────────────────────────────────────
//
// We encode each recorded event as a `u32`:
//   bits [7:0]  = stage index (0-indexed)
//   bits [9:8]  = kind: 0=start, 1=ok, 2=fail, 3=unknown
//
// A linear AtomicU32 array + a length counter forms the recorder.
// AtomicU32 is Sync, so no UnsafeCell needed.

const MAX_RECORDED: usize = 32;
const KIND_START: u32 = 0;
const KIND_OK: u32 = 1;
const KIND_FAIL: u32 = 2;
const KIND_UNKNOWN: u32 = 3;

/// Encodes a recorded event into a u32.
const fn encode_event(stage: u8, kind: u32) -> u32 {
    (kind << 8) | (stage as u32)
}

/// Decodes the stage from an encoded event.
const fn decode_stage(v: u32) -> u8 {
    (v & 0xFF) as u8
}

/// Decodes the kind from an encoded event.
const fn decode_kind(v: u32) -> u32 {
    (v >> 8) & 0x3
}

static RECORDED: [AtomicU32; MAX_RECORDED] = {
    const ZERO: AtomicU32 = AtomicU32::new(0);
    [ZERO; MAX_RECORDED]
};
static RECORDED_LEN: AtomicUsize = AtomicUsize::new(0);

/// Resets the recorder; call at the start of each test.
fn reset_recorder() {
    RECORDED_LEN.store(0, Ordering::Relaxed);
    // Zero out all slots.
    for slot in &RECORDED {
        slot.store(0, Ordering::Relaxed);
    }
}

/// Callback that appends the event to the global recorder.
fn record_event(e: &DS12Event) {
    let kind = if e.event == EVT_BOOT_STAGE_START {
        KIND_START
    } else if e.event == EVT_BOOT_STAGE_OK {
        KIND_OK
    } else if e.event == EVT_BOOT_STAGE_FAIL {
        KIND_FAIL
    } else {
        KIND_UNKNOWN
    };
    let idx = RECORDED_LEN.fetch_add(1, Ordering::Relaxed);
    if idx < MAX_RECORDED {
        RECORDED[idx].store(encode_event(e.fields.stage, kind), Ordering::Relaxed);
    }
}

/// Returns (stage, kind) for recorded event at index `i`.
fn get_recorded(i: usize) -> (u8, u32) {
    let v = RECORDED[i].load(Ordering::Relaxed);
    (decode_stage(v), decode_kind(v))
}

/// Returns the count of recorded events.
fn recorded_len() -> usize {
    RECORDED_LEN.load(Ordering::Relaxed).min(MAX_RECORDED)
}

// ── Empty boot driver produces stage events ──────────────────────────────────

arch_test!(run_boot_stages_produces_fourteen_events, {
    reset_recorder();

    let bus = DS12EventBus::new();
    bus.subscribe(record_event).unwrap();
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());

    crate::boot::run_boot_stages(&bus, &clock).expect("boot must succeed");

    // Stages 1..7 emit start then ok = 14 events.
    assert_eq!(recorded_len(), 14, "expected 14 events for stages 1..7 (start+ok each)");
});

// ── Boot-stage event ordering smoke test ─────────────────────────────────────

arch_test!(boot_stages_emit_start_then_ok_in_order, {
    reset_recorder();

    let bus = DS12EventBus::new();
    bus.subscribe(record_event).unwrap();
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());

    crate::boot::run_boot_stages(&bus, &clock).expect("boot must succeed");

    let n = recorded_len();
    assert_eq!(n, 14, "expected 14 events for stages 1..7");

    // Verify each adjacent pair is (start, ok) for the same stage in order.
    for stage in 1_u8..=7 {
        let i = usize::from((stage - 1) * 2);
        let (s_stage, s_kind) = get_recorded(i);
        let (o_stage, o_kind) = get_recorded(i + 1);
        assert_eq!(s_kind, KIND_START, "stage {stage}: event[{i}] must be start");
        assert_eq!(s_stage, stage, "stage {stage}: start.stage mismatch");
        assert_eq!(o_kind, KIND_OK, "stage {stage}: event[{}] must be ok", i + 1);
        assert_eq!(o_stage, stage, "stage {stage}: ok.stage mismatch");
    }
});

// ── Full-boot integration smoke via EditorInit::boot ───────────────────────────────
//
// Verifies that EditorInit::boot succeeds and returns a EditorCore with a live
// event_bus Shared. The bus + stage driver are already smoke-tested via
// run_boot_stages above; here we verify the EditorInit::boot wrapper does not
// break the invariant.

arch_test!(init_boot_returns_editor_core_with_event_bus, {
    let init = EditorInit::new(LauncherArgs::default());
    let editor_core = init.boot().expect("boot must succeed");

    // event_bus is a Shared<DS12EventBus>; strong count must be 1 at handoff.
    assert_eq!(
        editor_core.event_bus.strong_count(),
        1,
        "EditorCore.event_bus strong count must be 1 immediately after boot"
    );
});

// ── Forced-failure test ──────────────────────────────────────────────────────
//
// Injects a failure at stage 3. Verifies:
// 1. boot.stage.start emitted for stages 1, 2, 3.
// 2. boot.stage.fail emitted for stage 3 (not ok).
// 3. No events for stages 4..7 (boot aborted).
// 4. run_boot_stages returns Err(BootError::Stage { stage: 3 }).

arch_test!(forced_stage_failure_emits_fail_and_aborts, {
    reset_recorder();

    let bus = DS12EventBus::new();
    bus.subscribe(record_event).unwrap();
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());

    inject_stage_failure(3);
    let result = crate::boot::run_boot_stages(&bus, &clock);

    // Must return Err.
    assert!(result.is_err(), "boot must fail when stage 3 is injected");
    match result {
        Err(BootError::Stage { stage, .. }) => {
            assert_eq!(stage, 3, "BootError must name stage 3");
        }
        Err(BootError::Alloc | BootError::SeamRegistration { .. }) => {
            panic!("unexpected non-stage error")
        }
        Ok(()) => panic!("expected Err"),
    }

    // Expected: stage 1 start+ok, stage 2 start+ok, stage 3 start+fail = 6 events.
    assert_eq!(
        recorded_len(),
        6,
        "expected 6 events: start+ok for stages 1,2 and start+fail for stage 3"
    );

    // Stage 1: start, ok.
    let (s, k) = get_recorded(0);
    assert_eq!(k, KIND_START);
    assert_eq!(s, 1);
    let (s, k) = get_recorded(1);
    assert_eq!(k, KIND_OK);
    assert_eq!(s, 1);

    // Stage 2: start, ok.
    let (s, k) = get_recorded(2);
    assert_eq!(k, KIND_START);
    assert_eq!(s, 2);
    let (s, k) = get_recorded(3);
    assert_eq!(k, KIND_OK);
    assert_eq!(s, 2);

    // Stage 3: start, fail.
    let (s, k) = get_recorded(4);
    assert_eq!(k, KIND_START);
    assert_eq!(s, 3);
    let (s, k) = get_recorded(5);
    assert_eq!(k, KIND_FAIL);
    assert_eq!(s, 3);
});

// ── Forced-failure via EditorInit::boot drops EditorInit whole (LF13) ──────────────────

arch_test!(init_boot_with_injected_failure_returns_err, {
    inject_stage_failure(2);
    let init = EditorInit::new(LauncherArgs::default());
    let result = init.boot();
    assert!(result.is_err(), "boot must return Err when stage 2 is injected");
    match result {
        Err(BootError::Stage { stage, .. }) => assert_eq!(stage, 2),
        _ => panic!("unexpected result"),
    }
});

// ── Stage-order invariant: start always precedes ok for the same stage ───────

arch_test!(start_precedes_ok_for_every_stage, {
    reset_recorder();

    let bus = DS12EventBus::new();
    bus.subscribe(record_event).unwrap();
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());

    crate::boot::run_boot_stages(&bus, &clock).unwrap();

    // For each stage 1..7, find its start and ok positions and assert start < ok.
    let n = recorded_len();
    for stage in 1_u8..=7 {
        let start_pos = (0..n).find(|&i| {
            let (s, k) = get_recorded(i);
            s == stage && k == KIND_START
        });
        let ok_pos = (0..n).find(|&i| {
            let (s, k) = get_recorded(i);
            s == stage && k == KIND_OK
        });
        let sp = start_pos.expect("start event must exist for each stage");
        let op = ok_pos.expect("ok event must exist for each stage");
        assert!(sp < op, "stage {stage}: start({sp}) must precede ok({op})");
    }
});

// ── No fail events in a successful boot ──────────────────────────────────────

arch_test!(successful_boot_has_no_fail_events, {
    reset_recorder();

    let bus = DS12EventBus::new();
    bus.subscribe(record_event).unwrap();
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());

    crate::boot::run_boot_stages(&bus, &clock).unwrap();

    let n = recorded_len();
    for i in 0..n {
        let (_, kind) = get_recorded(i);
        assert_ne!(
            kind, KIND_FAIL,
            "no fail events must appear in a successful boot (event index {i})"
        );
    }
});

// ── run_stage with a real failing body (boot.rs L152-154) ───────────────────
//
// Calls `run_stage` directly with `Some(body)` where `body` returns `Err`.
// Verifies:
// 1. `boot.stage.fail` is emitted.
// 2. The returned `BootError::Stage { stage, reason }` carries the correct
//    stage number and the reason string from the body.

fn failing_body() -> Result<(), &'static str> {
    Err("test-body-failure")
}

arch_test!(run_stage_real_body_err_emits_fail_and_returns_err, {
    reset_recorder();

    let bus = DS12EventBus::new();
    bus.subscribe(record_event).unwrap();
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());

    let result = run_stage(&bus, &clock, 4, Some(failing_body));

    assert!(result.is_err(), "run_stage with failing body must return Err");
    match result {
        Err(BootError::Stage { stage, reason }) => {
            assert_eq!(stage, 4, "BootError::Stage must carry stage 4");
            assert_eq!(reason, "test-body-failure", "reason must be preserved");
        }
        Err(BootError::Alloc | BootError::SeamRegistration { .. }) => {
            panic!("unexpected non-stage error")
        }
        Ok(()) => panic!("expected Err"),
    }

    // Events: start (stage 4) then fail (stage 4) — exactly 2.
    assert_eq!(recorded_len(), 2, "run_stage with Err body must emit start + fail (2 events)");
    let (s, k) = get_recorded(0);
    assert_eq!(k, KIND_START, "first event must be start");
    assert_eq!(s, 4, "start.stage must be 4");
    let (s, k) = get_recorded(1);
    assert_eq!(k, KIND_FAIL, "second event must be fail");
    assert_eq!(s, 4, "fail.stage must be 4");
});

// ── KIND_UNKNOWN arm of the test decoder (boot_tests.rs L84) ─────────────────
//
// `record_event` maps unknown event strings to `KIND_UNKNOWN`. Exercise the
// arm by subscribing `record_event` and emitting a non-boot event.

arch_test!(record_event_maps_unknown_event_to_kind_unknown, {
    reset_recorder();

    let bus = DS12EventBus::new();
    bus.subscribe(record_event).unwrap();
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());

    // Emit an event that is not boot.stage.{start,ok,fail}.
    bus.emit(&DS12Event {
        ts_nanos: clock.elapsed_nanos(),
        level: reovim_uapi::abi::error::LogLevel::Info,
        event: "custom.unrecognised",
        fields: crate::event_bus::BootStageFields {
            stage: 77,
            error_code: None,
        },
    });

    assert_eq!(recorded_len(), 1, "one event must be recorded");
    let (_stage, kind) = get_recorded(0);
    assert_eq!(kind, KIND_UNKNOWN, "unrecognised event must map to KIND_UNKNOWN");
    // Verify decode_kind is working for an out-of-range value.
    let v = encode_event(77, KIND_UNKNOWN);
    assert_eq!(decode_kind(v), KIND_UNKNOWN, "decode_kind must recover KIND_UNKNOWN");
    assert_eq!(decode_stage(v), 77, "decode_stage must recover 77");
});

arch_test!(injected_failure_at_every_stage_aborts_boot, {
    // Each stage's `?` continuation in `run_boot_stages` only executes when
    // THAT stage fails; sweep the injector across all driver-run stages.
    for stage in 1..=7u8 {
        reset_recorder();
        let bus = DS12EventBus::new();
        let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
        inject_stage_failure(stage);
        let result = crate::boot::run_boot_stages(&bus, &clock);
        match result {
            Err(BootError::Stage { stage: s, .. }) => assert_eq!(s, stage),
            _ => panic!("stage failure injection must surface BootError::Stage"),
        }
    }
});
