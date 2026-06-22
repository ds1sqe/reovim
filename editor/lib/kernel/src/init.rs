//! `Init` — the boot actor (2.1 §2, 2.2 §1, LF13).
//!
//! `Init` exists only during boot. It exclusively owns the kernel state under
//! construction (`&mut self`, no locks), drives the boot stages, and is
//! *consumed* by the handoff into `Shared<Kernel>`. Using an `Init` after
//! `boot()` is a compile error (the value is moved).
//!
//! Boot stages 0..7 (2.2 §1):
//!
//! ```text
//! stage 0: Init::new  — LauncherArgs capture, BootClock allocation
//! stage 1: kernel.host config materialised (CFG9)  — structural stub
//! stage 2: kernel.shell config + runtime caps       — structural stub
//! stage 3: lockfile + library-root resolution       — structural stub
//! stage 4: module discovery + per-module load       — structural stub
//! stage 5: driver discovery + per-driver load       — structural stub
//! stage 6: framed-protocol runtime / in-memory adapter started — structural stub
//! stage 7: handoff — Init::boot returns Shared<Kernel>; serving
//! ```
//!
//! Stages 1..6 run as structural stubs: each emits its
//! `boot.stage.{start,ok}` events and performs no policy work (2.2 §1
//! structural-stub rule). Real policy for each stage arrives with its
//! respective feature.
//!
//! On boot failure at any stage, `Init` drops whole — no partially-constructed
//! kernel state escapes (LF13).

use {
    reovim_lib_ds::{RwLock, Shared},
    reovim_uapi::{
        log::{LogSinkControl, LogSinkError, LogSinkHandle},
        panic::{
            Disposition, PanicConfigError, PanicControl, PanicFlushTarget, PanicRecord,
            PreExitHookFn, RingTailProviderFn, StateRecordHookFn,
        },
        sched::{ClockControl, ThreadControl},
        system::{BootInfo, DeviceInventory},
    },
};

use crate::{
    BootClock,
    boot::run_boot_stages,
    event_bus::DS12EventBus,
    kernel::{KERNEL_ABI_VERSION, Kernel, KernelAbi},
    log::{flush, ring::LogRing},
    router::DomainRouter,
    session::{
        BufferId, DomainAttachmentId, Session, SessionId, SessionState, SessionTable, WindowId,
    },
};

// ── LauncherArgs ─────────────────────────────────────────────────────────────

/// Arguments parsed from the process entry point and passed into `Init::new`.
///
/// This is the boot-only config source for values that must be readable before
/// the config service exists. The full config service (CFG1..CFG10) is
/// deferred. What `LauncherArgs` carries:
///
/// - `disposition`: the panic-handler disposition (`recover` | `halt`),
///   sourced from a CLI flag or environment variable; defaults to `Recover`
///   per spec (6.2 §5 default). The config-driven form lands with the config
///   service.
/// - `panic`: up-face panic/log control table supplied by composition roots.
///   The default table is a no-op for tests and type construction; real
///   process entries should pass the system-kernel bridge table.
/// - `log`: up-face log sink control table supplied by composition roots.
///   The default table is a no-op for tests and type construction; real
///   process entries should pass the system-kernel bridge table.
/// - `clock`: up-face clock control table supplied by composition roots.
///   The default table returns zero for tests and type construction; real
///   process entries should pass the system-kernel bridge table.
/// - `thread`: up-face thread identity control table supplied by composition
///   roots. The default table returns zero for tests and type construction;
///   real process entries should pass the system-kernel bridge table.
/// - `log_ring_bytes`: the log-ring capacity override; defaults to 1 MiB per
///   spec (LOG6). The config-driven form lands with the config service.
/// - `boot_info`: up-face hardware facts shaped by the system-kernel bridge and
///   pushed in at entry; defaults to empty on hosted builds with no firmware to
///   query. The boot-tail diagnostics read it to print the banner.
/// - `device_inventory`: up-face static device enumeration shaped by the
///   system-kernel bridge at boot; defaults to empty on hosted builds and on
///   freestanding targets without a DTB. The kernel stores this for later
///   driver-discovery phases.
///
/// Fields are intentionally minimal per the rule-of-three: only what stage 0 and
/// the boot-tail diagnostics need. Config fields for later stages arrive with
/// those features.
///
/// # Example
///
/// ```rust
/// use reovim_kernel::LauncherArgs;
///
/// // Default args: Recover disposition, 1 MiB ring.
/// let args = LauncherArgs::default();
/// assert_eq!(args.log_ring_bytes, 1024 * 1024);
/// ```
#[derive(Debug, Clone)]
pub struct LauncherArgs {
    /// Panic-handler disposition (sourced from launcher CLI until the config
    /// service exists). Default: `Recover` (6.2 §5).
    pub disposition: Disposition,

    /// Product-facing panic/log control table used during boot registration.
    ///
    /// Composition roots pass the system-kernel bridge implementation here.
    /// The default value is a no-op table so tests can construct `LauncherArgs`
    /// without importing the system bridge directly.
    pub panic: PanicControl,

    /// Product-facing log sink control table used by LOG7/LOG8 sinks.
    ///
    /// Composition roots pass the system-kernel bridge implementation here.
    /// The default value is a no-op table so tests can construct `LauncherArgs`
    /// without importing the system bridge directly.
    pub log: LogSinkControl,

    /// Product-facing clock control table used by the boot clock.
    ///
    /// Composition roots pass the system-kernel bridge implementation here.
    /// The default value is a no-op table so tests can construct `LauncherArgs`
    /// without importing the system bridge directly.
    pub clock: ClockControl,

    /// Product-facing thread identity control table used by service ownership
    /// registration.
    ///
    /// Composition roots pass the system-kernel bridge implementation here.
    /// The default value is a no-op table so tests can construct `LauncherArgs`
    /// without importing the system bridge directly.
    pub thread: ThreadControl,

    /// Log-ring capacity in bytes (LOG6 `log-ring-bytes`). Default: 1 MiB.
    /// The config-driven form lands with the config service.
    pub log_ring_bytes: usize,

    /// Hardware facts shaped by the system-kernel bridge and pushed in at entry.
    /// Default: empty (hosted builds with no firmware). The boot-tail
    /// diagnostics read this to emit the hardware banner.
    pub boot_info: BootInfo,

    /// Static device inventory from the firmware device tree, shaped by the
    /// system-kernel bridge and pushed in at entry. Default: empty (hosted
    /// builds and freestanding targets without a DTB). No consumer reads it in
    /// this phase beyond storage in `LauncherArgs`.
    pub device_inventory: DeviceInventory,
}

impl Default for LauncherArgs {
    /// Default launcher args: `Recover` disposition (6.2 §5 default), 1 MiB
    /// log ring (LOG6 default).
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_kernel::LauncherArgs;
    ///
    /// let args = LauncherArgs::default();
    /// assert_eq!(args.log_ring_bytes, 1024 * 1024);
    /// ```
    fn default() -> Self {
        Self {
            disposition: Disposition::Recover,
            panic: noop_panic_control(),
            log: noop_log_sink_control(),
            clock: ClockControl::default(),
            thread: ThreadControl::default(),
            log_ring_bytes: 1024 * 1024, // 1 MiB
            boot_info: BootInfo::default(),
            device_inventory: DeviceInventory::default(),
        }
    }
}

const fn noop_panic_control() -> PanicControl {
    PanicControl::new(
        noop_set_disposition,
        noop_set_ring_tail_provider,
        noop_set_state_record_hook,
        noop_set_flush_target,
        noop_set_pre_exit_hook,
    )
}

fn noop_set_disposition(_: Disposition) -> Result<(), PanicConfigError> {
    Ok(())
}

fn noop_set_ring_tail_provider(_: RingTailProviderFn) -> Result<(), PanicConfigError> {
    Ok(())
}

fn noop_set_state_record_hook(_: StateRecordHookFn) -> Result<(), PanicConfigError> {
    Ok(())
}

fn noop_set_flush_target(_: PanicFlushTarget) -> Result<(), PanicConfigError> {
    Ok(())
}

fn noop_set_pre_exit_hook(_: PreExitHookFn) -> Result<(), PanicConfigError> {
    Ok(())
}

const fn noop_log_sink_control() -> LogSinkControl {
    LogSinkControl::new(
        noop_open_log_sink,
        noop_write_log_sink,
        noop_close_log_sink,
        noop_register_panic_flush,
        noop_write_diagnostic,
    )
}

fn noop_open_log_sink(_: &[u8]) -> Result<LogSinkHandle, LogSinkError> {
    Err(LogSinkError::new(0))
}

fn noop_write_log_sink(_: LogSinkHandle, _: &[u8]) -> Result<(), LogSinkError> {
    Ok(())
}

fn noop_close_log_sink(_: LogSinkHandle) -> Result<(), LogSinkError> {
    Ok(())
}

fn noop_register_panic_flush(_: LogSinkHandle) -> Result<(), PanicConfigError> {
    Ok(())
}

fn noop_write_diagnostic(_: &[u8]) -> Result<(), LogSinkError> {
    Ok(())
}

// ── BootError ────────────────────────────────────────────────────────────────

/// Errors that can abort the boot sequence.
///
/// When `Init::boot` returns `Err(BootError)`, the `Init` value has been
/// consumed — no partially-constructed kernel state escapes (LF13). The
/// variant names the failing stage for diagnostic purposes.
///
/// # Example
///
/// ```rust
/// use reovim_kernel::BootError;
///
/// let e = BootError::Stage { stage: 1, reason: "config not available" };
/// // Stage number is preserved for the caller.
/// match e {
///     BootError::Stage { stage, .. } => assert_eq!(stage, 1),
///     BootError::Alloc => panic!("wrong variant"),
///     BootError::SeamRegistration { seam } => panic!("wrong variant: {seam}"),
/// }
/// ```
#[derive(Debug)]
pub enum BootError {
    /// A boot stage failed; `stage` is the 0-indexed stage number (2.2 §1).
    Stage {
        /// The 0-indexed boot-stage number that failed.
        stage: u8,
        /// Human-readable reason (static lifetime: no allocation in error paths).
        reason: &'static str,
    },
    /// Allocation failed during kernel construction.
    Alloc,
    /// A panic/log configuration slot was already registered.
    ///
    /// A second boot in the same process (outside the `selftest` environment
    /// where `cfg(feature = "selftest")` applies the AlreadyConfigured-ok
    /// rule) is a programming error. `seam` names the hook for diagnostics.
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_kernel::BootError;
    ///
    /// let e = BootError::SeamRegistration { seam: "ring_tail_provider" };
    /// match e {
    ///     BootError::SeamRegistration { seam } => assert_eq!(seam, "ring_tail_provider"),
    ///     _ => panic!("wrong variant"),
    /// }
    /// ```
    SeamRegistration {
        /// The hook name that returned `AlreadyConfigured` (static lifetime).
        seam: &'static str,
    },
}

// ── Panic state-record slot (AB12 step 3) ────────────────────────────────────
//
// `STATE_RECORD` holds the last `PanicRecord` observed by the
// `record_panic_state` hook. Under `selftest` the test harness reads it via
// `last_panic_record()` to assert the hook was called with the correct record.
// In production the hook is called exactly once (from the panic handler) and
// the slot is never read back (full persistence is deferred).
//
// The slot is encoded as an `AtomicU32`:
//   - bit 31 (MSB): `1` = slot populated, `0` = empty (initial).
//   - bit 0: disposition: `0` = Halt, `1` = Recover.
//   - bit 1: rollback_failed: `0` = false, `1` = true.
//
// This avoids a `static mut PanicRecord` and keeps the encoding `const`-safe.

use core::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

/// Encoded `STATE_RECORD` sentinel: slot is empty.
const RECORD_EMPTY: u32 = 0;
/// Bit mask: slot-populated flag.
const RECORD_POPULATED: u32 = 1 << 31;
/// Bit index: disposition (0 = Halt, 1 = Recover).
const RECORD_DISPOSE_BIT: u32 = 0;
/// Bit index: `rollback_failed` flag.
const RECORD_ROLLBACK_BIT: u32 = 1;

/// The process-global state-record slot (selftest-accessible).
static STATE_RECORD: AtomicU32 = AtomicU32::new(RECORD_EMPTY);

/// The AB12 state-record hook (6.2 §5 step 3).
///
/// Called by `arch`'s panic handler with the [`PanicRecord`] before the
/// process terminates. This boot-core stub stores the disposition and rollback
/// marker in [`STATE_RECORD`] so the test harness can assert correct hook
/// invocation. Full persistence (lifecycle + quarantine state) is deferred.
///
/// ```rust,no_run
/// // no_run: mutates process-global state — parallel doctests share the process.
/// use reovim_kernel::init::record_panic_state;
/// use reovim_uapi::panic::{Disposition, PanicRecord};
///
/// let rec = PanicRecord { disposition: Disposition::Recover, rollback_failed: false };
/// record_panic_state(rec);
/// ```
pub fn record_panic_state(record: PanicRecord) {
    let mut bits = RECORD_POPULATED;
    if matches!(record.disposition, Disposition::Recover) {
        bits |= 1 << RECORD_DISPOSE_BIT;
    }
    if record.rollback_failed {
        bits |= 1 << RECORD_ROLLBACK_BIT;
    }
    // Relaxed: this is called from the panic handler in a single-threaded
    // terminal context; no other thread races the write, and the test harness
    // reads it after the panic handler returns (in the same thread under the
    // selftest synthetic call).
    STATE_RECORD.store(bits, AtomicOrdering::Relaxed);
}

/// Returns the last [`PanicRecord`] stored by [`record_panic_state`], or `None`
/// if the hook has not been called yet.
///
/// Only available under the `selftest` feature. Used by flush tests to assert
/// the hook received the expected record.
///
/// ```rust,no_run
/// // no_run: selftest-gated — not available in production builds.
/// ```
#[cfg(feature = "selftest")]
pub fn last_panic_record() -> Option<PanicRecord> {
    let bits = STATE_RECORD.load(AtomicOrdering::Relaxed);
    if bits & RECORD_POPULATED == 0 {
        return None;
    }
    let disposition = if bits & (1 << RECORD_DISPOSE_BIT) != 0 {
        Disposition::Recover
    } else {
        Disposition::Halt
    };
    Some(PanicRecord {
        disposition,
        rollback_failed: bits & (1 << RECORD_ROLLBACK_BIT) != 0,
    })
}

/// Resets the state-record slot to empty.
///
/// Must be called at the start of every test that calls `record_panic_state`
/// directly, to avoid state pollution between tests.
///
/// Only available under the `selftest` feature.
///
/// ```rust,no_run
/// // no_run: selftest-gated — not available in production builds.
/// ```
#[cfg(feature = "selftest")]
pub fn reset_state_record_for_test() {
    STATE_RECORD.store(RECORD_EMPTY, AtomicOrdering::Relaxed);
}

// ── Seam registration helper ──────────────────────────────────────────────────
//
// Maps `PanicConfigError::AlreadyConfigured` to `BootError::SeamRegistration`
// in production builds. Under `selftest`, `AlreadyConfigured` is silently
// accepted because the no_std selftest runner runs multiple tests in one
// process and the panic configuration atoms are process-global write-once; a
// prior boot test may have already registered them. The safe posture is to
// accept the existing registration rather than aborting.

const fn register_panic_config(
    result: Result<(), PanicConfigError>,
    seam: &'static str,
) -> Result<(), BootError> {
    match result {
        Ok(()) => Ok(()),
        Err(PanicConfigError::AlreadyConfigured) => {
            // In the selftest environment the runner shares one process across
            // all tests; a prior Init::boot already registered this seam with
            // the correct values. Accept it as success.
            #[cfg(feature = "selftest")]
            {
                // `seam` is unused in this arm under selftest; suppress the lint.
                let _ = seam;
                Ok(())
            }
            // In production a second boot is a bug: surface it as BootError.
            #[cfg(not(feature = "selftest"))]
            {
                Err(BootError::SeamRegistration { seam })
            }
        }
    }
}

// ── Init ─────────────────────────────────────────────────────────────────────

/// The boot actor (2.1 §2, LF13).
///
/// Created by `Init::new` and consumed by `Init::boot`. While `Init` exists,
/// the kernel state is under exclusive mutable ownership (`&mut self`) — no
/// locks, no concurrent observers (LF13). Using an `Init` after `boot()` is a
/// compile error.
///
/// # Example
///
/// ```rust,no_run
/// // no_run: requires the arch runtime — use the kernel-selftest bin.
/// use reovim_kernel::{Init, LauncherArgs};
///
/// let init = Init::new(LauncherArgs::default());
/// let kernel = init.boot().expect("boot must succeed in a healthy process");
/// let _ = kernel; // Shared<Kernel> — steady-state root
/// ```
pub struct Init {
    /// Boot-only launcher arguments; die with `Init` at the handoff (LF13).
    pub(crate) args: LauncherArgs,
    /// Monotonic + wall-clock anchor captured in stage 0 (7.5 §4).
    pub(crate) boot_anchor: BootClock,
}

impl Init {
    /// Stage 0: capture `LauncherArgs` and the boot-clock anchor.
    ///
    /// This is the *only* `Init` constructor. It allocates nothing — stage 0
    /// is entirely on the stack — and always succeeds.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use reovim_kernel::{Init, LauncherArgs};
    /// use reovim_uapi::sched::ClockControl;
    ///
    /// fn mono() -> i64 { 1 }
    /// fn realtime() -> i64 { 946_684_800_000_000_001 }
    ///
    /// let mut args = LauncherArgs::default();
    /// args.clock = ClockControl::new(mono, realtime);
    /// let init = Init::new(args);
    /// // boot_anchor wall-clock is after year 2000 (Unix-epoch nanos).
    /// assert!(init.boot_anchor().wall_anchor > 946_684_800_000_000_000);
    /// ```
    #[must_use]
    pub fn new(args: LauncherArgs) -> Self {
        // Stage 0: capture the boot clock anchor (CLOCK_MONOTONIC + CLOCK_REALTIME).
        let boot_anchor = BootClock::capture(args.clock);
        Self { args, boot_anchor }
    }

    /// The `BootClock` anchor captured in stage 0.
    ///
    /// Provided so callers can inspect the anchor without consuming `self`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use reovim_kernel::{Init, LauncherArgs};
    /// use reovim_uapi::sched::ClockControl;
    ///
    /// fn mono() -> i64 { 1 }
    /// fn realtime() -> i64 { 2 }
    ///
    /// let mut args = LauncherArgs::default();
    /// args.clock = ClockControl::new(mono, realtime);
    /// let init = Init::new(args);
    /// let anchor = init.boot_anchor();
    /// assert!(anchor.elapsed_nanos() < u64::MAX);
    /// ```
    #[must_use]
    pub const fn boot_anchor(&self) -> &BootClock {
        &self.boot_anchor
    }

    /// The launcher args captured in stage 0.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use reovim_kernel::{Init, LauncherArgs};
    ///
    /// let init = Init::new(LauncherArgs::default());
    /// assert_eq!(init.args().log_ring_bytes, 1024 * 1024);
    /// ```
    #[must_use]
    pub const fn args(&self) -> &LauncherArgs {
        &self.args
    }

    /// Stages 1..7 + handoff: run the boot sequence and return the
    /// steady-state `Shared<Kernel>`.
    ///
    /// This is the **only** `Kernel` constructor (LF13). On success `Init` is
    /// consumed and the boot-only fields (`args`) are dropped. On failure
    /// `Init` drops whole — no partially-constructed kernel state escapes.
    ///
    /// The log ring (LOG6) and DS12 event bus are created before stage 1. The
    /// ring is wired as the always-present built-in subscriber (LOG1), so it
    /// captures every `boot.stage.*` event. Both are transferred into `Kernel`
    /// at the stage-7 handoff.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when any boot stage fails. The specific stage
    /// number is preserved in `BootError::Stage { stage, .. }`. On failure
    /// `Init` has been consumed — caller retains no kernel state.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// // no_run: requires the arch runtime — use the kernel-selftest bin.
    /// use reovim_kernel::{Init, LauncherArgs};
    ///
    /// let init = Init::new(LauncherArgs::default());
    /// let kernel = init.boot().expect("boot succeeds");
    /// drop(kernel);
    /// ```
    pub fn boot(self) -> Result<Shared<Kernel>, BootError> {
        // Destructure so boot-only fields drop at function exit on the
        // error path rather than requiring an explicit drop call.
        let Self { args, boot_anchor } = self;
        crate::log::sink::install_control(args.log);

        // ── Stage 0: allocate the log ring (LOG6) ────────────────────────────
        //
        // The ring must exist before any event is emitted (LOG6 requires the
        // ring to receive every event from boot stage 0 onward). Allocate it
        // before the bus, then wire it as the bus's built-in subscriber.
        let ring = LogRing::try_new(args.log_ring_bytes).map_err(|_| BootError::Alloc)?;
        let ring_shared = Shared::try_new(ring).map_err(|_| BootError::Alloc)?;

        // ── Stage 0: create the DS12 event bus and wire the ring ─────────────
        //
        // The ring is wired as the always-present built-in subscriber (LOG1)
        // by handing the bus a `Shared<LogRing>` clone. After `set_builtin`,
        // every `emit()` call on the bus calls `ring.push_event` directly —
        // no raw-pointer indirection. The ring clone here and the one stored
        // on `Kernel` both keep the refcount ≥2 through the process lifetime.
        let mut bus = DS12EventBus::new();
        bus.set_builtin(Shared::clone(&ring_shared));

        // ── Stage 0: register panic/log policy (AB12, 9.5 §9.1) ─────────────
        //
        // All four hooks are write-once (6.2 §5.2). Registration happens in
        // stage 0 so every subsequent boot stage runs with the panic path fully
        // wired. A second registration in the same process returns AlreadyConfigured;
        // `register_panic_config` maps that to BootError::SeamRegistration in
        // production and accepts it silently under `selftest` (process-shared
        // statics).
        //
        // The panic flush target is intentionally NOT registered here. The LOG7
        // sink fd does not exist until `FileSink::open_and_subscribe` is called (a later
        // phase). The sink-open path registers the flush target at that point.
        // Registering it at boot with an invalid target would be wrong; the spec
        // write-once contract prevents correction after the fact. The default
        // posture (no target registered → panic writes to stderr, 6.2 §5.2) is
        // safe until the sink opens.
        //
        // Ring-tail provider: the flush mirror is allocated at compile time
        // (static BSS); `flush::ring_tail` is safe to register before any push.
        register_panic_config(
            args.panic.set_ring_tail_provider(flush::ring_tail),
            "ring_tail_provider",
        )?;
        // State-record hook: the boot-core stub records disposition + rollback
        // marker into the process-global STATE_RECORD slot for test inspection.
        // Full persistence is deferred.
        register_panic_config(
            args.panic.set_state_record_hook(record_panic_state),
            "state_record_hook",
        )?;
        // Disposition: sourced from `LauncherArgs` (spec default `Recover`
        // when the field is absent or unconfigured, 6.2 §5).
        register_panic_config(args.panic.set_disposition(args.disposition), "disposition")?;

        // ── Stages 1..7 (structural stubs, OBS1 events) ─────────────────────
        //
        // Every stage emits through `bus`; the built-in slot captures them all.
        run_boot_stages(&bus, &boot_anchor)?;

        // ── Boot tail: hardware banner + live health probes ─────────────────
        //
        // dmesg-style diagnostics riding the live console (LOG1): the discovered
        // hardware facts plus real checks of the subsystems that exist
        // (clock/heap/ring/memory) — not per-stage theater on the structural-stub
        // stages. Emitted through `bus` (and its built-in ring) before the bus is
        // moved into `Kernel`; `args.boot_info` is the floor's discovered facts.
        crate::diagnostics::run_at_boot_tail(&bus, &boot_anchor, &ring_shared, &args.boot_info);

        // ── Stage 7 handoff: construct Kernel ────────────────────────────────
        //
        // Stage 7's start/ok events were emitted by run_boot_stages.
        // Boot-only state (`args`) is dropped here (already destructured).
        // Everything that must survive boot is moved into Kernel's fields.
        let abi = KernelAbi::new(KERNEL_ABI_VERSION);
        let abi_shared = Shared::try_new(abi).map_err(|_| BootError::Alloc)?;

        let bus_shared = Shared::try_new(bus).map_err(|_| BootError::Alloc)?;

        // ── Walking-skeleton: empty DomainRouter + placeholder Session ────────
        //
        // The composition root (and kernel-selftest tests) register the text
        // Domain and attach a session AFTER boot by calling `Kernel::setup_domain`
        // and `Kernel::setup_session`. The kernel cannot depend on ext crates
        // (core/ext boundary), so `Init::boot` provides empty containers.
        //
        // Placeholder `SessionState` uses `DomainId` value 0's NonZeroU32 — but
        // since `DomainId` is `NonZeroU32` there is no zero value. The session
        // here is a stub; `Kernel::setup_session` replaces it with a real one.
        // We use a sentinel domain_id (1) here; it is replaced immediately by
        // `Kernel::setup_session` before any dispatch happens.
        let router = DomainRouter::new();
        let router_shared = Shared::try_new(RwLock::new(router)).map_err(|_| BootError::Alloc)?;

        // Placeholder session: single-entry focus chain with sentinel ids.
        // The composition root replaces the session contents via `setup_session`
        // before any dispatch call; these are never dispatched without registration.
        let sentinel_domain = crate::router::DomainId::new(core::num::NonZeroU32::MIN);
        let session_state = SessionState::new(
            sentinel_domain,
            DomainAttachmentId::new(1),
            BufferId::new(1),
            WindowId::new(1),
        );
        let session = Session::new(SessionId::new(1), session_state);
        let primary_session = Shared::try_new(session).map_err(|_| BootError::Alloc)?;
        let mut sessions = SessionTable::new();
        sessions
            .insert_shared(Shared::clone(&primary_session))
            .map_err(|_| BootError::Alloc)?;
        let session_table = Shared::try_new(RwLock::new(sessions)).map_err(|_| BootError::Alloc)?;
        let state = crate::state::StateSubstrate::new_with_thread(args.thread);
        let state_shared = Shared::try_new(RwLock::new(state)).map_err(|_| BootError::Alloc)?;

        let kernel = Kernel::new(crate::kernel::KernelParts {
            abi_shared,
            boot_anchor,
            event_bus: bus_shared,
            log_ring: ring_shared,
            session: primary_session,
            sessions: session_table,
            domain_router: router_shared,
            state: state_shared,
        });
        Shared::try_new(kernel).map_err(|_| BootError::Alloc)
    }
}

// L12 layout: tests in sibling init_tests.rs, declared in lib.rs.
