//! The AB12 panic-handler skeleton and its write-once hook seam.
//!
//! This module owns the platform-floor *mechanism* of the panic path (6.2 §5,
//! 9.5 §9.1): the `#[panic_handler]` lang item, the allocator-free LOG2 line
//! renderer (9.5 §2 grammar, kernel-emitter form), the **final flush** to a
//! registered file descriptor, and process termination per the registered
//! disposition.
//!
//! The write-once hook *registry* itself lives down-face in `kabi/panic` — the
//! always-present fault-floor seam (Platform-Contract §3.4). arch reaches it
//! through the thin `set_*`/`load_*` shims in this module; the `set_*` entry
//! points and the [`Disposition`]/[`PanicRecord`]/[`SetError`] vocabulary are
//! re-exported here so existing callers are unchanged while ownership lives
//! down-face. `CLEANUP_CONTEXT` (the AB13 marker) is the one piece of registry
//! state arch still owns — it is an arch-local runtime flag, not a relocated
//! hook.
//!
//! ## Skeleton boundary (what arch builds vs. defers)
//!
//! Arch builds the panic-path *mechanism* and the shims onto the *hook seam*,
//! not the kernel subsystems it ultimately integrates with. There is **no**
//! kernel log ring, persistence subsystem, DS12 event, or supervision here —
//! those land in later master-plan phases. The seam is four write-once hooks
//! (owned by `kabi/panic`) the higher layers register at boot:
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
//! Each hook is an atomic static in `kabi/panic`. Registration writes use
//! `Release`; the panic handler reads use `Acquire`, so everything written
//! before registration is visible to a panic on any thread after it. The first
//! `set_*` wins; a second is rejected with [`SetError::AlreadySet`] and
//! changes nothing — re-registration is a boot-stage bug, surfaced, never
//! silently honoured.

use core::sync::atomic::{AtomicU8, Ordering::Release};

// The five write-once panic-policy atoms live in `kabi/panic` — the
// always-present down-face fault-floor seam (Platform-Contract §3.4). arch
// keeps the `#[panic_handler]` lang item, the allocator-free render/flush
// mechanism, and the AB13 cleanup-context flag; the hook registry is owned by
// `kabi/panic` and reached through the thin shims in this module.
use reovim_kabi_panic as kabi_panic;

/// The panic disposition and record types are owned up-face by `uapi/panic`;
/// re-exported here so callers naming `reovim_arch::panic::{Disposition,
/// PanicRecord}` keep resolving while the canonical definition lives up-face.
pub use reovim_uapi_panic::{Disposition, PanicRecord};

/// Why a `set_*` registration was rejected — owned by `kabi/panic` (write-once,
/// 6.2 §5.2). Re-exported so callers naming `reovim_arch::panic::SetError` keep
/// resolving through the shim. The first registration wins; a second is rejected
/// with [`SetError::AlreadySet`] and changes nothing.
pub use reovim_kabi_panic::SetError;

/// Capacity of the panic line's fixed render buffer (bytes).
///
/// The panic renderer is **allocator-free** (it must work before the platform
/// handle installs — a panic can fire mid-boot), so it formats into a fixed
/// stack buffer instead of a heap `lib/ds::Bytes`. The line is `[ts] kernel
/// panic: <message> at <file>:<line>:<col>` plus an optional ` rollback=failed`
/// suffix; 512 bytes covers a realistic panic message + source location, and a
/// longer one is truncated best-effort (panic-time rendering is best-effort by
/// design — the old `Bytes` renderer likewise swallowed an OOM mid-line).
#[cfg(any(feature = "runtime", feature = "selftest"))]
const PANIC_LINE_CAP: usize = 512;

/// An allocator-free, fallible [`core::fmt::Write`] sink over a fixed stack
/// buffer — the panic line's render target.
///
/// This is the bootstrap-safe replacement for the old `lib/ds::BytesWriter`:
/// it constructs NO handle-routed data structure, so a panic that fires before
/// `rust_entry` installs the platform handle still renders (the
/// no-DS-before-install invariant holds on the panic path by construction).
/// A write that would overflow the buffer is truncated and reported as
/// [`core::fmt::Error`], so the panic path renders best-effort rather than
/// recursing into a second panic. The partially rendered bytes still flush.
#[cfg(any(feature = "runtime", feature = "selftest"))]
pub(crate) struct StackWriter {
    /// The fixed render buffer; only the first `len` bytes are written.
    buf: [u8; PANIC_LINE_CAP],
    /// Bytes written so far (the live prefix of `buf`).
    len: usize,
}

#[cfg(any(feature = "runtime", feature = "selftest"))]
impl StackWriter {
    /// Creates an empty writer over a zeroed fixed buffer.
    pub(crate) const fn new() -> Self {
        Self {
            buf: [0; PANIC_LINE_CAP],
            len: 0,
        }
    }

    /// The rendered bytes (the live prefix of the buffer).
    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

#[cfg(any(feature = "runtime", feature = "selftest"))]
impl core::fmt::Write for StackWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let remaining = PANIC_LINE_CAP - self.len;
        if bytes.len() > remaining {
            // Buffer full: copy what fits, then report truncation so the panic
            // path stays alive rather than re-panicking. Best-effort by design.
            self.buf[self.len..].copy_from_slice(&bytes[..remaining]);
            self.len = PANIC_LINE_CAP;
            return Err(core::fmt::Error);
        }
        self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(())
    }
}

/// The signature of a registered ring-tail provider.
type RingTailProvider = fn() -> &'static [u8];
/// The signature of a registered state-record hook.
type StateRecordHook = fn(PanicRecord);
/// The signature of the pre-exit hook (gap-7, #797 Phase 4, 8.2 §2).
type PreExitHook = fn();

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
    // Delegating shim: the atom is owned by kabi/panic. arch keeps this entry
    // point so existing callers (kernel boot) are unchanged until 3d migrates
    // them to kabi/panic directly.
    kabi_panic::set_flush_fd(fd)
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
    // Delegating shim: the atom is owned by kabi/panic (3d migrates callers).
    kabi_panic::set_ring_tail_provider(provider)
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
    // Delegating shim: the atom is owned by kabi/panic (3d migrates callers).
    kabi_panic::set_state_record_hook(hook)
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
    // Delegating shim: the atom is owned by kabi/panic (3d migrates callers).
    kabi_panic::set_disposition(disposition)
}

/// Registers a pre-exit callback invoked by the panic handler BEFORE it renders
/// its final output (gap-7, #797 Phase 4, 8.2 §2).
///
/// The TUI platform runtime registers a terminal-restore function here before
/// entering raw mode. When the panic handler fires it calls this hook first, so
/// the terminal is back in cooked mode when the panic line is written to stderr.
///
/// Write-once: the first `set_pre_exit_hook` wins; a second is rejected with
/// [`SetError::AlreadySet`] and changes nothing (same contract as the other four
/// seams in this module — a double-register is a boot-stage bug, surfaced).
///
/// # Errors
///
/// Returns [`SetError::AlreadySet`] if a hook is already registered.
///
/// ```no_run
/// // Write-once process-global state — not safe to run in the doctest harness.
/// use reovim_arch::panic::set_pre_exit_hook;
/// fn restore_terminal() {}
/// set_pre_exit_hook(restore_terminal).expect("first registration always succeeds");
/// ```
pub fn set_pre_exit_hook(hook: PreExitHook) -> Result<(), SetError> {
    // Delegating shim: the atom is owned by kabi/panic (3d migrates callers).
    kabi_panic::set_pre_exit_hook(hook)
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
    // The disposition atom is owned by kabi/panic. An unset registry resolves
    // to `halt` here — an unbooted process halts (the safe posture, 6.2 §5.2).
    kabi_panic::get_disposition().unwrap_or(Disposition::Halt)
}

/// Builds the [`PanicRecord`] for the current fault from the registry.
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn current_record() -> PanicRecord {
    use core::sync::atomic::Ordering::Acquire;
    PanicRecord {
        disposition: current_disposition(),
        // Acquire pairs with the cleanup-context `Release` stores so a panic
        // observing the flag also observes everything written before it was set.
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
/// by the #797 Phase 4 fixture-exec tests and the scaffolding is unit-tested with
/// a stand-in message here.
///
/// Rendering is best-effort and **allocator-free**: it formats into the
/// caller-owned [`StackWriter`] (a fixed stack buffer), so a panic that fires
/// before the platform handle installs still renders. A buffer-overflow write
/// surfaces as `fmt::Error` and is swallowed, so the panic path never recurses
/// into a second panic; the partially rendered bytes are still flushed.
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn render_line<F>(w: &mut StackWriter, record: PanicRecord, render_msg: F)
where
    F: FnOnce(&mut StackWriter) -> core::fmt::Result,
{
    use core::fmt::Write;

    let ts = crate::time::monotonic();

    // LOG2 ts: seconds right-aligned min width 5, micros zero-padded width 6.
    // Then kernel-emitter form: emitter `kernel`, bare-subsystem address
    // `panic` (no `/`, so unambiguously a kernel address — 9.5 §3).
    let secs = ts.tv_sec;
    // `tv_nsec` is in `0..1_000_000_000`, so micros fits and the cast is exact.
    #[allow(clippy::cast_sign_loss)]
    let micros = (ts.tv_nsec / 1_000) as u64;
    // A failed write leaves `w` holding whatever rendered first; the panic
    // path stays alive rather than re-panicking on buffer overflow.
    let _ = write!(w, "[{secs:>5}.{micros:06}] kernel panic: ");
    let _ = render_msg(w);
    if record.rollback_failed {
        // AB13: the cleanup-context panic carries the rollback marker.
        let _ = w.write_str(" rollback=failed");
    }
    let _ = w.write_str("\n");
}

/// Writes the panic message + location into `w` per LOG2 message rendering.
///
/// Runtime-only: the `#[panic_handler]` feeds the live `PanicInfo`; the unit
/// tests render a synthetic message instead (a real `PanicInfo` cannot be
/// synthesized under `panic = "abort"`).
///
/// DEV1 restructure (#797 Phase 5 coverage): `PanicInfo::location()` always returns
/// `Some` for Rust panics today, but the API returns `Option` and the docs
/// say "currently" — that is not a contract. `unwrap_unchecked` here would
/// be latent UB the day the contract shifts, so the `None` arm is handled
/// safely in [`render_location`], which takes the `Option` so both arms are
/// directly unit-testable (no dead branch, no unchecked assumption).
#[cfg(feature = "runtime")]
fn render_panic_message(w: &mut StackWriter, info: &core::panic::PanicInfo) -> core::fmt::Result {
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
    w: &mut StackWriter,
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
    // The flush-fd atom is owned by kabi/panic; `None` means no sink registered.
    if let Some(fd) = kabi_panic::get_flush_fd() {
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
///
/// A delegating wrapper over `kabi/panic` (which owns the atom and the
/// fn-pointer reconstruction); arch keeps it so the panic-path callers and the
/// selftest suite reach the provider through one local name.
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn load_ring_tail_provider() -> Option<RingTailProvider> {
    kabi_panic::get_ring_tail_provider()
}

/// Loads the registered state-record hook, or `None` if unset.
///
/// Delegating wrapper over `kabi/panic` (see [`load_ring_tail_provider`]).
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn load_state_record_hook() -> Option<StateRecordHook> {
    kabi_panic::get_state_record_hook()
}

/// Loads the registered pre-exit hook, or `None` if unset.
///
/// Delegating wrapper over `kabi/panic` (see [`load_ring_tail_provider`]).
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn load_pre_exit_hook() -> Option<PreExitHook> {
    kabi_panic::get_pre_exit_hook()
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
/// "abort"` forbids synthesizing in-process): read the registry, invoke the
/// pre-exit hook (so terminal state is restored before output), render the
/// line, fire the state hook, perform the final flush, and return the
/// disposition's exit code. The caller terminates with that code.
#[cfg(any(feature = "runtime", feature = "selftest"))]
fn handle<F>(render_msg: F) -> i32
where
    F: FnOnce(&mut StackWriter) -> core::fmt::Result,
{
    let record = current_record();
    // Invoke the pre-exit hook FIRST (gap-7, 8.2 §2): the TUI runtime registers
    // a terminal-restore function here; calling it before rendering ensures the
    // panic line is written in cooked mode rather than raw mode.
    if let Some(hook) = load_pre_exit_hook() {
        hook();
    }
    // Render into a fixed stack buffer — allocator-free, so this works even
    // before the platform handle installs (a panic mid-boot still renders).
    let mut line = StackWriter::new();
    render_line(&mut line, record, render_msg);
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
/// fixture and `no_std` test-runner bins enable it; libtest builds do not.
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

/// Resets every registry slot so each test starts from the unregistered state.
/// The registry is process-global write-once; the no_std runner is
/// single-threaded sequential, so no external serialization is needed. Every
/// registry test must call this at the END of the test body to leave the
/// globals clean for the next test (runner does not tear down between tests).
///
/// The five hook atoms live in `kabi/panic` now, so the reset of those forwards
/// to its `selftest`-gated [`reovim_kabi_panic::reset`]; `CLEANUP_CONTEXT` is an
/// arch-local runtime flag (not a relocated hook), so arch resets it directly.
#[cfg(feature = "selftest")]
pub(crate) fn reset_registry() {
    use core::sync::atomic::Ordering::Relaxed;
    kabi_panic::reset();
    CLEANUP_CONTEXT.store(0, Relaxed);
}

// L12 layout (#785 Phase 5): tests live in the sibling file `panic_tests.rs`,
// declared as a `#[path]` child so `super::` reaches the private registry
// statics and test-only helpers (`reset_registry`, `handle`, etc.).
#[cfg(feature = "selftest")]
#[path = "panic_tests.rs"]
mod tests;
