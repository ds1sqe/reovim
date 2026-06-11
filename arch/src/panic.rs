//! The AB12 panic-handler skeleton and its write-once hook seam.
//!
//! This module owns the platform-floor half of the panic path (6.2 §5,
//! 9.5 §9.1). It renders the panic into a LOG2 line (9.5 §2 grammar,
//! kernel-emitter form), performs the **final flush** to a registered file
//! descriptor, fires a registered state-record hook, and terminates the
//! process per the registered disposition.
//!
//! ## Skeleton boundary (what arch builds vs. defers)
//!
//! Arch builds the panic-path *mechanism* and the *hook seam*, not the
//! kernel subsystems it ultimately integrates with. There is **no** kernel
//! log ring, persistence subsystem, DS12 event, or supervision here — those
//! land in later master-plan phases. The seam is four write-once hooks the
//! higher layers register at boot:
//!
//! - [`set_flush_fd`] — the LOG7 file sink fd the final flush writes to;
//! - [`set_ring_tail_provider`] — returns a pre-rendered LOG2 byte tail
//!   (arch writes it verbatim ahead of the panic line; arch never parses
//!   ring entries — 9.5 §9.1);
//! - [`set_state_record_hook`] — called with the [`PanicRecord`] so the
//!   persistence consumer (later) can record lifecycle + quarantine state;
//! - [`set_disposition`] — selects `recover` (exit 75) or `halt` (exit 70).
//!
//! With nothing registered the handler renders the panic line to fd 2
//! (stderr) and exits with the `halt` code — the safe posture for a process
//! that never finished boot (6.2 §5.2 default).
//!
//! ## Write-once contract (6.2 §5.2)
//!
//! Each hook is an atomic static. Registration writes use `Release`; the
//! panic handler reads use `Acquire`, so everything written before
//! registration is visible to a panic on any thread after it. The first
//! `set_*` wins; a second is rejected with [`SetError::AlreadySet`] and
//! changes nothing — re-registration is a boot-stage bug, surfaced, never
//! silently honoured.

use core::sync::atomic::{
    AtomicI32, AtomicU8, AtomicUsize,
    Ordering::{Acquire, Release},
};

// `Bytes`/`BytesWriter` are the panic line's render target, used only by the
// gated panic-path internals (the handler + tests). A non-runtime non-test
// rlib build compiles none of them, so the import is gated to match (else
// `unused_imports` under the zero-warning deny).
#[cfg(any(feature = "runtime", feature = "selftest"))]
use crate::ds::{Bytes, BytesWriter};

/// What the panic handler does after recording the fault (6.2 §5, §5.1).
///
/// The two dispositions exit with fixed `sysexits` status codes that the
/// supervising launcher treats as a contract (§5.1): exactly `75` is
/// restart-requested; any other non-zero exit is a no-restart fault.
///
/// ```rust
/// use reovim_arch::panic::Disposition;
///
/// assert_eq!(Disposition::Recover.exit_code(), 75);
/// assert_eq!(Disposition::Halt.exit_code(), 70);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// `EX_TEMPFAIL` (75): transient failure — restart requested. The
    /// supervisor restarts and quarantines the attributed owner.
    Recover,
    /// `EX_SOFTWARE` (70): internal software error — stop for analysis. The
    /// flushed log and process state are preserved (dev/test posture).
    Halt,
}

impl Disposition {
    /// The normative process exit code for this disposition (6.2 §5.1).
    ///
    /// ```rust
    /// use reovim_arch::panic::Disposition;
    ///
    /// // EX_TEMPFAIL (75) signals a restart-requested transient failure.
    /// assert_eq!(Disposition::Recover.exit_code(), 75);
    /// // EX_SOFTWARE (70) signals a halt-for-analysis internal error.
    /// assert_eq!(Disposition::Halt.exit_code(), 70);
    /// ```
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        match self {
            // EX_TEMPFAIL: the launcher treats exactly 75 as restart-requested.
            Self::Recover => 75,
            // EX_SOFTWARE: any non-75 non-zero exit is a no-restart fault.
            Self::Halt => 70,
        }
    }

    /// Encodes the disposition as the `u8` stored in the atomic registry.
    const fn as_u8(self) -> u8 {
        match self {
            Self::Recover => DISPOSITION_RECOVER,
            Self::Halt => DISPOSITION_HALT,
        }
    }
}

/// The record handed to the state-record hook (6.2 §5 step 3, AB13).
///
/// Arch fills the disposition and the AB13 cleanup marker; the persistence
/// consumer (a later phase) maps it onto lifecycle + quarantine state. Arch
/// keeps it minimal: no kernel DS is referenced here (skeleton boundary).
///
/// ```rust
/// use reovim_arch::panic::{Disposition, PanicRecord};
///
/// let r = PanicRecord { disposition: Disposition::Halt, rollback_failed: false };
/// assert_eq!(r.disposition.exit_code(), 70);
/// assert!(!r.rollback_failed);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanicRecord {
    /// The disposition the handler will terminate under.
    pub disposition: Disposition,
    /// Whether the panic happened in a cleanup context (AB13). When `true`
    /// the flushed line carries `rollback = failed` and the persisted state
    /// records `TombstonedFailedUnload`.
    pub rollback_failed: bool,
}

/// Why a `set_*` registration was rejected.
///
/// ```rust
/// use reovim_arch::panic::SetError;
///
/// // AlreadySet is the only variant; its Debug form is stable.
/// let e = SetError::AlreadySet;
/// assert_eq!(format!("{e:?}"), "AlreadySet");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetError {
    /// The hook was already registered; the first registration wins and
    /// this call changed nothing (6.2 §5.2 write-once).
    AlreadySet,
}

// ---- the four write-once hook statics (6.2 §5.2) --------------------------
//
// Function-pointer hooks are stored as their address in an `AtomicUsize`
// (0 = unregistered); the disposition as an `AtomicU8` (0 = unregistered).
// `usize` carries a `fn` pointer losslessly on this crate's only target.

/// The LOG7 file-sink fd for the final flush. `-1` = unregistered.
static FLUSH_FD: AtomicI32 = AtomicI32::new(-1);
/// `fn() -> &'static [u8]` returning the pre-rendered ring tail. 0 = unset.
static RING_TAIL_PROVIDER: AtomicUsize = AtomicUsize::new(0);
/// `fn(PanicRecord)` state-record hook. 0 = unset.
static STATE_RECORD_HOOK: AtomicUsize = AtomicUsize::new(0);
/// The disposition value (encoded `u8`). `DISPOSITION_UNSET` = unset.
static DISPOSITION: AtomicU8 = AtomicU8::new(DISPOSITION_UNSET);

/// Encoded `DISPOSITION` sentinel: nothing registered (handler defaults to
/// `halt`, 6.2 §5.2).
const DISPOSITION_UNSET: u8 = 0;
/// Encoded `DISPOSITION` value for [`Disposition::Recover`].
const DISPOSITION_RECOVER: u8 = 1;
/// Encoded `DISPOSITION` value for [`Disposition::Halt`].
const DISPOSITION_HALT: u8 = 2;

/// The signature of a registered ring-tail provider.
type RingTailProvider = fn() -> &'static [u8];
/// The signature of a registered state-record hook.
type StateRecordHook = fn(PanicRecord);

/// The AB13 cleanup-panic marker: a process-global, TLS-free flag the caller
/// raises around a shutdown/drop/unregister so a panic there records
/// `rollback = failed` (6.2 §AB13). `false` = normal context.
static CLEANUP_CONTEXT: AtomicU8 = AtomicU8::new(0);

/// Registers the final-flush file descriptor (write-once, 6.2 §5.2).
///
/// # Errors
///
/// Returns [`SetError::AlreadySet`] if a flush fd is already registered; the
/// first registration wins and this call changes nothing.
///
/// ```no_run
/// // Write-once process-global state — not safe to run in the doctest harness
/// // because parallel doctests share the process and the static is write-once.
/// use reovim_arch::panic::set_flush_fd;
/// set_flush_fd(2).expect("first registration always succeeds");
/// ```
pub fn set_flush_fd(fd: i32) -> Result<(), SetError> {
    // CAS from the unset sentinel (-1): the first writer wins. Release so a
    // panic on any thread observing the fd also observes prior writes.
    FLUSH_FD
        .compare_exchange(-1, fd, Release, Acquire)
        .map(|_| ())
        .map_err(|_| SetError::AlreadySet)
}

/// Registers the ring-tail provider (write-once, 6.2 §5.2).
///
/// The provider returns one contiguous `&[u8]` already in LOG2 line format;
/// arch writes it verbatim ahead of the panic line and never parses it
/// (9.5 §9.1).
///
/// # Errors
///
/// Returns [`SetError::AlreadySet`] if a provider is already registered.
///
/// ```no_run
/// // Write-once process-global state — not safe to run in the doctest harness.
/// use reovim_arch::panic::set_ring_tail_provider;
/// fn my_tail() -> &'static [u8] { b"" }
/// set_ring_tail_provider(my_tail).expect("first registration always succeeds");
/// ```
pub fn set_ring_tail_provider(provider: RingTailProvider) -> Result<(), SetError> {
    // The fn pointer is stored as its address; `load_ring_tail_provider`
    // reconstructs exactly this type from it.
    let addr = (provider as *const ()).addr();
    set_fn_hook(&RING_TAIL_PROVIDER, addr)
}

/// Registers the state-record hook (write-once, 6.2 §5.2).
///
/// The handler calls it with the [`PanicRecord`] before terminating, so the
/// persistence consumer can record lifecycle + quarantine state.
///
/// # Errors
///
/// Returns [`SetError::AlreadySet`] if a hook is already registered.
///
/// ```no_run
/// // Write-once process-global state — not safe to run in the doctest harness.
/// use reovim_arch::panic::{PanicRecord, set_state_record_hook};
/// fn my_hook(_: PanicRecord) {}
/// set_state_record_hook(my_hook).expect("first registration always succeeds");
/// ```
pub fn set_state_record_hook(hook: StateRecordHook) -> Result<(), SetError> {
    // The fn pointer is stored as its address; `load_state_record_hook`
    // reconstructs exactly this type from it.
    let addr = (hook as *const ()).addr();
    set_fn_hook(&STATE_RECORD_HOOK, addr)
}

/// Registers the panic disposition (write-once, 6.2 §5.2).
///
/// # Errors
///
/// Returns [`SetError::AlreadySet`] if a disposition is already registered.
///
/// ```no_run
/// // Write-once process-global state — not safe to run in the doctest harness.
/// use reovim_arch::panic::{Disposition, set_disposition};
/// set_disposition(Disposition::Recover).expect("first registration always succeeds");
/// ```
pub fn set_disposition(disposition: Disposition) -> Result<(), SetError> {
    DISPOSITION
        .compare_exchange(DISPOSITION_UNSET, disposition.as_u8(), Release, Acquire)
        .map(|_| ())
        .map_err(|_| SetError::AlreadySet)
}

/// Shared write-once CAS for a function-pointer hook stored as a `usize`.
fn set_fn_hook(slot: &AtomicUsize, value: usize) -> Result<(), SetError> {
    slot.compare_exchange(0, value, Release, Acquire)
        .map(|_| ())
        .map_err(|_| SetError::AlreadySet)
}

/// Marks the calling context as a cleanup context (AB13).
///
/// A panic while this flag is set records `rollback = failed` in the
/// [`PanicRecord`] and the flushed line. The flag is process-global and
/// TLS-free (no thread-locals exist in `arch/`); the caller raises it
/// around a single-threaded shutdown/drop/unregister sequence and lowers it
/// with [`clear_cleanup_context`] after.
///
/// ```no_run
/// // Process-global write — not safe to run in the parallel doctest harness.
/// use reovim_arch::panic::{enter_cleanup_context, clear_cleanup_context};
/// enter_cleanup_context();
/// // ... perform cleanup ...
/// clear_cleanup_context();
/// ```
pub fn enter_cleanup_context() {
    // Release: a subsequent panic's Acquire read of the flag observes this.
    CLEANUP_CONTEXT.store(1, Release);
}

/// Lowers the cleanup-context marker raised by [`enter_cleanup_context`].
///
/// ```no_run
/// // Process-global write — not safe to run in the parallel doctest harness.
/// use reovim_arch::panic::clear_cleanup_context;
/// clear_cleanup_context(); // safe to call even without a prior enter
/// ```
pub fn clear_cleanup_context() {
    CLEANUP_CONTEXT.store(0, Release);
}

// The panic-path internals below are compiled only where they are exercised:
// the `#[panic_handler]` (the `runtime` feature) and the unit tests. A plain
// non-runtime rlib build never reaches them, so gating keeps the zero-warning
// `dead_code` deny satisfied without a blanket allow.

/// Resolves the effective disposition: the registered value, or `halt` when
/// nothing is registered (6.2 §5.2 default).
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn current_disposition() -> Disposition {
    match DISPOSITION.load(Acquire) {
        DISPOSITION_RECOVER => Disposition::Recover,
        // Both the explicit `halt` registration and the unset default land
        // here: an unbooted process halts (the safe posture).
        _ => Disposition::Halt,
    }
}

/// Builds the [`PanicRecord`] for the current fault from the registry.
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn current_record() -> PanicRecord {
    PanicRecord {
        disposition: current_disposition(),
        rollback_failed: CLEANUP_CONTEXT.load(Acquire) != 0,
    }
}

/// Renders a LOG2 kernel-emitter line whose message is produced by `render_msg`.
///
/// Form: `[ts] kernel panic: <message>` with the AB13 ` rollback=failed`
/// suffix appended when the cleanup marker is set, terminated by `\n`. The
/// timestamp is the monotonic clock (the LOG2 `ts` source). Attribution
/// (the cdylib emitter address) is omitted — arch renders kernel-emitter
/// form unless a hook supplies attribution (deferred to a later phase).
///
/// The message is rendered through the `render_msg` callback so the panic
/// handler can feed the live [`core::panic::PanicInfo`] while unit tests feed
/// a synthetic message: under `panic = "abort"` a real `PanicInfo` cannot be
/// caught and synthesized in-process, so the message extraction is exercised
/// by the Phase 4 fixture-exec tests and the scaffolding is unit-tested with
/// a stand-in message here.
///
/// Rendering is best-effort: a refused growth surfaces as `fmt::Error` from
/// [`BytesWriter`] and is swallowed, so the panic path never recurses into a
/// second panic. The partially rendered bytes are still flushed.
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn render_line<F>(record: PanicRecord, render_msg: F) -> Bytes
where
    F: FnOnce(&mut BytesWriter) -> core::fmt::Result,
{
    use core::fmt::Write;

    let ts = crate::time::monotonic();
    let mut line = Bytes::new();
    let mut w = BytesWriter::new(&mut line);

    // LOG2 ts: seconds right-aligned min width 5, micros zero-padded width 6.
    // Then kernel-emitter form: emitter `kernel`, bare-subsystem address
    // `panic` (no `/`, so unambiguously a kernel address — 9.5 §3).
    let secs = ts.tv_sec;
    // `tv_nsec` is in `0..1_000_000_000`, so micros fits and the cast is exact.
    #[allow(clippy::cast_sign_loss)]
    let micros = (ts.tv_nsec / 1_000) as u64;
    // A failed write leaves `line` holding whatever rendered first; the panic
    // path stays alive rather than re-panicking on OOM.
    let _ = write!(w, "[{secs:>5}.{micros:06}] kernel panic: ");
    let _ = render_msg(&mut w);
    if record.rollback_failed {
        // AB13: the cleanup-context panic carries the rollback marker.
        let _ = w.write_str(" rollback=failed");
    }
    let _ = w.write_str("\n");
    line
}

/// Writes the panic message + location into `w` per LOG2 message rendering.
///
/// Runtime-only: the `#[panic_handler]` feeds the live `PanicInfo`; the unit
/// tests render a synthetic message instead (a real `PanicInfo` cannot be
/// synthesized under `panic = "abort"`).
///
/// DEV1 restructure (Phase 5 coverage): `PanicInfo::location()` always returns
/// `Some` for Rust panics today, but the API returns `Option` and the docs
/// say "currently" — that is not a contract. `unwrap_unchecked` here would
/// be latent UB the day the contract shifts, so the `None` arm is handled
/// safely in [`render_location`], which takes the `Option` so both arms are
/// directly unit-testable (no dead branch, no unchecked assumption).
#[cfg(feature = "runtime")]
fn render_panic_message(w: &mut BytesWriter, info: &core::panic::PanicInfo) -> core::fmt::Result {
    use core::fmt::Write;

    // `PanicInfo::message()` is the structured payload; `Display` renders it.
    write!(w, "{}", info.message())?;
    render_location(w, info.location())
}

/// Renders the ` at file:line:col` suffix, or ` at <unknown>` when the
/// panic machinery supplied no location (not contractually impossible —
/// see [`render_panic_message`]). Takes the `Option` so a unit test can
/// drive both arms without synthesizing a `PanicInfo`.
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn render_location(
    w: &mut BytesWriter,
    location: Option<&core::panic::Location<'_>>,
) -> core::fmt::Result {
    use core::fmt::Write;

    match location {
        Some(loc) => write!(w, " at {}:{}:{}", loc.file(), loc.line(), loc.column()),
        None => w.write_str(" at <unknown>"),
    }
}

/// Performs the final flush (9.5 §9.1): write the registered ring tail (if
/// any) then the panic line to the registered fd. With no fd registered the
/// line goes to fd 2 (stderr) and the ring tail is skipped (the provider's
/// tail is the ring's, meaningless without the sink).
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn final_flush(line: &[u8]) {
    let fd = FLUSH_FD.load(Acquire);
    if fd >= 0 {
        // A registered sink: write the pre-rendered ring tail verbatim ahead
        // of the panic line (9.5 §9.1 ordering). A short or failed write is
        // swallowed — at panic time there is no recovery, only best effort.
        if let Some(provider) = load_ring_tail_provider() {
            write_all(fd, provider());
        }
        write_all(fd, line);
    } else {
        // Default posture (6.2 §5.2): no sink registered, render to stderr.
        write_all(2, line);
    }
}

/// Loads the registered ring-tail provider, or `None` if unset.
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn load_ring_tail_provider() -> Option<RingTailProvider> {
    match RING_TAIL_PROVIDER.load(Acquire) {
        0 => None,
        // SAFETY: the static holds either 0 (handled above) or the address
        // of a `RingTailProvider` written once by `set_ring_tail_provider`,
        // which transmuted exactly this fn type to `usize`. Reconstructing
        // the same fn pointer is sound.
        addr => Some(unsafe { core::mem::transmute::<usize, RingTailProvider>(addr) }),
    }
}

/// Loads the registered state-record hook, or `None` if unset.
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn load_state_record_hook() -> Option<StateRecordHook> {
    match STATE_RECORD_HOOK.load(Acquire) {
        0 => None,
        // SAFETY: as `load_ring_tail_provider`: the address was written once
        // by `set_state_record_hook` from exactly this fn type.
        addr => Some(unsafe { core::mem::transmute::<usize, StateRecordHook>(addr) }),
    }
}

/// Writes the whole of `buf` to `fd`, looping over short writes; gives up on
/// the first error (panic-time best effort, no retry budget).
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn write_all(fd: i32, buf: &[u8]) {
    let mut off = 0;
    while off < buf.len() {
        match crate::sys::write(fd, &buf[off..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => off += n,
        }
    }
}

/// The AB12 panic sequence (6.2 §5), parameterized over the message renderer
/// so it is unit-testable without an actual `PanicInfo` (which `panic =
/// "abort"` forbids synthesizing in-process): read the registry, render the
/// line, fire the state hook, perform the final flush, and return the
/// disposition's exit code. The caller terminates with that code.
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn handle<F>(render_msg: F) -> i32
where
    F: FnOnce(&mut BytesWriter) -> core::fmt::Result,
{
    let record = current_record();
    let line = render_line(record, render_msg);
    // Fire the state-record hook (6.2 §5 step 3) before flushing, so a hook
    // that wants to influence the tail has run; the flush is the last step.
    if let Some(hook) = load_state_record_hook() {
        hook(record);
    }
    final_flush(line.as_slice());
    record.disposition.exit_code()
}

/// The AB12 panic handler (gated on `runtime`, see the module doc and
/// `arch/Cargo.toml`).
///
/// Defining `#[panic_handler]` unconditionally would clash with the std
/// libtest binary that hosts arch's pre-migration tests (bootstrap state 1):
/// std already provides the lang item. The `runtime` feature is the gate —
/// fixture and no_std test-runner bins enable it; libtest builds do not.
#[cfg(feature = "runtime")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let code = handle(|w| render_panic_message(w, info));
    // Under coverage instrumentation, flush the profile before the panic
    // exit too — the panic path bypasses `start::exit_process`, and the
    // panic fixtures' coverage would otherwise be lost with the process.
    #[cfg(arch_coverage)]
    {
        // SAFETY: the panic sequence is single-threaded and terminal; this
        // is the sole profile write on this path (the write-file contract).
        unsafe {
            crate::profiler::__llvm_profile_write_file();
        }
    }
    crate::sys::exit_group(code)
}

/// Satisfies the prebuilt sysroot `libcore`'s `DW.ref.rust_eh_personality`
/// reference at link time. Under DAG6 there is no unwinder (`panic =
/// "abort"` everywhere), so unwinding machinery can never invoke this; if
/// control ever arrives here the binary is corrupt — halt immediately with
/// the AB12 halt code.
#[cfg(feature = "runtime")]
#[unsafe(no_mangle)]
extern "C" fn rust_eh_personality() -> ! {
    crate::sys::exit_group(Disposition::Halt.exit_code())
}

/// Resets every registry static so each test starts from the unregistered
/// state. The registry is process-global write-once; the no_std runner is
/// single-threaded sequential, so no external serialization is needed. Every
/// registry test must call this at the END of the test body to leave the
/// globals clean for the next test (runner does not tear down between tests).
#[cfg(feature = "selftest")]
pub(crate) fn reset_registry() {
    use core::sync::atomic::Ordering::Relaxed;
    FLUSH_FD.store(-1, Relaxed);
    RING_TAIL_PROVIDER.store(0, Relaxed);
    STATE_RECORD_HOOK.store(0, Relaxed);
    DISPOSITION.store(DISPOSITION_UNSET, Relaxed);
    CLEANUP_CONTEXT.store(0, Relaxed);
}

// L12 layout (#785 Phase 5): tests live in the sibling file `panic_tests.rs`,
// declared as a `#[path]` child so `super::` reaches the private registry
// statics and test-only helpers (`reset_registry`, `handle`, etc.).
#[cfg(feature = "selftest")]
#[path = "panic_tests.rs"]
mod tests;
