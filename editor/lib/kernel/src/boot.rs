//! Boot-stage driver — stages 0..7, OBS1 `boot.stage.*` events (2.2 §1).
//!
//! Each stage emits `boot.stage.start` before its work and `boot.stage.ok`
//! on success. A stage failure emits `boot.stage.fail` and immediately aborts
//! boot: no later stage starts, no later events fire, `Init` drops whole and
//! no `Kernel` is constructed (LF13).
//!
//! ## Structural stubs (2.2 §1 stub rule)
//!
//! Stages 2..6 are structural stubs: each emits its ordered
//! `boot.stage.{start,ok}` events and performs no policy work. Stage numbers
//! are stable (OBS1 goldens depend on them). Real policy for each stage arrives
//! with its respective feature.
//!
//! ## Correlation IDs (OBS2 / OBS2 exemption)
//!
//! Boot-stage events carry no correlation IDs. The correlation allocator is
//! a deferred `Kernel` field. OBS2's exemption covers all boot-stage events
//! while the allocator is absent (the exemption is "before the correlation
//! allocator exists"; no correlation IDs are present at this layer).

use reovim_uapi::abi::error::LogLevel;

use crate::{
    BootClock,
    event_bus::{
        BootStageFields, DS12Event, DS12EventBus, EVT_BOOT_STAGE_FAIL, EVT_BOOT_STAGE_OK,
        EVT_BOOT_STAGE_START,
    },
    init::BootError,
};

// ── Test-only fault injection (selftest feature) ─────────────────────────────

/// Stage index at which `boot()` should inject a failure (0 = no injection).
///
/// Set by tests via `inject_stage_failure`; read by `run_stage`. Only
/// compiled under `selftest` — the production path has no such hook.
#[cfg(feature = "selftest")]
static INJECT_FAIL_STAGE: core::sync::atomic::AtomicU8 =
    core::sync::atomic::AtomicU8::new(NO_INJECT);

#[cfg(feature = "selftest")]
const NO_INJECT: u8 = 255;

/// Arms the fault injector to fail the given stage index on the next
/// `boot()` call, then disarm automatically.
///
/// A stage index of 255 disarms without injecting anything.
///
/// # Example
///
/// ```rust,ignore
/// // ignore: requires the selftest feature — run via the kernel-selftest bin.
/// use reovim_kernel::boot::inject_stage_failure;
/// inject_stage_failure(2);
/// ```
#[cfg(feature = "selftest")]
pub fn inject_stage_failure(stage: u8) {
    INJECT_FAIL_STAGE.store(stage, core::sync::atomic::Ordering::Relaxed);
}

// ── Stage helpers ─────────────────────────────────────────────────────────────

/// Emits `boot.stage.start` for the given stage.
fn emit_start(bus: &DS12EventBus, clock: &BootClock, stage: u8) {
    bus.emit(&DS12Event {
        ts_nanos: clock.elapsed_nanos(),
        level: LogLevel::Info,
        event: EVT_BOOT_STAGE_START,
        fields: BootStageFields {
            stage,
            error_code: None,
        },
    });
}

/// Emits `boot.stage.ok` for the given stage.
fn emit_ok(bus: &DS12EventBus, clock: &BootClock, stage: u8) {
    bus.emit(&DS12Event {
        ts_nanos: clock.elapsed_nanos(),
        level: LogLevel::Info,
        event: EVT_BOOT_STAGE_OK,
        fields: BootStageFields {
            stage,
            error_code: None,
        },
    });
}

/// Emits `boot.stage.fail` for the given stage, carrying `reason` as a
/// numeric error code derived from the static reason string pointer address.
///
/// The error code in the event is the `BootError::Stage` stage number cast to
/// `i32`, matching the pattern expected by the OBS1 failure schema (9.4 §5).
fn emit_fail(bus: &DS12EventBus, clock: &BootClock, stage: u8) {
    bus.emit(&DS12Event {
        ts_nanos: clock.elapsed_nanos(),
        level: LogLevel::Error,
        event: EVT_BOOT_STAGE_FAIL,
        fields: BootStageFields {
            stage,
            error_code: Some(i32::from(stage)),
        },
    });
}

/// Runs a single boot stage.
///
/// Emits `boot.stage.start`, runs `body()`, then emits either
/// `boot.stage.ok` or `boot.stage.fail`. On failure returns
/// `Err(BootError::Stage { stage, reason })` — the caller (boot driver)
/// must abort boot immediately.
///
/// # Test injection
///
/// Under the `selftest` feature a `INJECT_FAIL_STAGE` atomic can force a
/// failure at a specific stage without modifying production code paths.
/// A real stage body. Structural-stub stages (2.2 §1 stub rule) carry `None`:
/// they hold their ordering position and emit events, but run no policy work.
///
/// `pub(crate)` so `boot_tests.rs` can supply a real `Some(failing_body)`
/// to cover the stage-body `Err` arm (boot.rs L152-154) directly.
pub type StageBody = fn() -> Result<(), &'static str>;

pub(crate) fn run_stage(
    bus: &DS12EventBus,
    clock: &BootClock,
    stage: u8,
    body: Option<StageBody>,
) -> Result<(), BootError> {
    emit_start(bus, clock, stage);

    // Test-only fault injection: if the injector matches this stage, fail it.
    #[cfg(feature = "selftest")]
    {
        let armed = INJECT_FAIL_STAGE.load(core::sync::atomic::Ordering::Relaxed);
        if armed == stage {
            // Disarm immediately so a subsequent boot call is not affected.
            INJECT_FAIL_STAGE.store(NO_INJECT, core::sync::atomic::Ordering::Relaxed);
            emit_fail(bus, clock, stage);
            return Err(BootError::Stage {
                stage,
                reason: "test: injected failure",
            });
        }
    }

    match body.map_or(Ok(()), |f| f()) {
        Ok(()) => {
            emit_ok(bus, clock, stage);
            Ok(())
        }
        Err(reason) => {
            emit_fail(bus, clock, stage);
            Err(BootError::Stage { stage, reason })
        }
    }
}

// ── Boot driver entry point ───────────────────────────────────────────────────

/// Runs the full boot sequence (stages 1..7) on the given `DS12EventBus`.
///
/// Stage 0 has already run in `Init::new` (`BootClock` capture); this function
/// drives stages 1..6 then hands off at stage 7 by returning `Ok(())`.
/// The caller (`Init::boot`) wraps the return with the actual `Kernel`
/// construction.
///
/// On the first failing stage, `boot.stage.fail` is emitted, boot aborts,
/// and `Err(BootError)` is returned. No subsequent stage runs or emits.
///
/// # Errors
///
/// Returns `Err(BootError::Stage)` when any stage body returns an error or
/// the test-only fault injector fires.
///
/// # Example
///
/// ```rust,no_run
/// // no_run: requires the arch runtime — use the kernel-selftest bin.
/// use reovim_kernel::event_bus::DS12EventBus;
/// use reovim_kernel::BootClock;
/// use reovim_kernel::boot::run_boot_stages;
///
/// let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
/// let bus = DS12EventBus::new();
/// run_boot_stages(&bus, &clock).expect("boot succeeds");
/// ```
pub fn run_boot_stages(bus: &DS12EventBus, clock: &BootClock) -> Result<(), BootError> {
    // ── Stage 1: kernel.host config materialised (CFG9) — structural stub ──
    run_stage(bus, clock, 1, None)?;

    // ── Stage 2: kernel.shell config + runtime caps — structural stub ──────
    run_stage(bus, clock, 2, None)?;

    // ── Stage 3: lockfile + library-root resolution — structural stub ──────
    run_stage(bus, clock, 3, None)?;

    // ── Stage 4: module discovery + per-module load — structural stub ──────
    run_stage(bus, clock, 4, None)?;

    // ── Stage 5: driver discovery + per-driver load — structural stub ──────
    run_stage(bus, clock, 5, None)?;

    // ── Stage 6: framed-protocol runtime / in-memory adapter — stub ────────
    run_stage(bus, clock, 6, None)?;

    // ── Stage 7: handoff ──────────────────────────────────────────────────
    // Stage 7 marks the transition from Init to Kernel (Init::boot
    // returns Shared<Kernel>). Emitting start/ok here signals that the
    // boot sequence has completed and serving begins. The bus is transferred
    // into Kernel by the caller immediately after this function returns.
    run_stage(bus, clock, 7, None)?;

    Ok(())
}

// L12 layout: tests in sibling boot_tests.rs, declared in lib.rs.
