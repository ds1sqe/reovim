//! The panic-seam registration shims (the product-facing `arch::panic` surface).
//!
//! The `#[panic_handler]` lang item, its allocator-free render/flush mechanism,
//! `rust_eh_personality`, and the LLVM coverage profiler runtime were relocated
//! out of arch into the per-target `reovim-arch-floor-{target}` crates (SP03):
//! they are per-binary LINK symbols, one per linked binary, supplied by the
//! composition root's floor dep. What stays here is the **registration API**
//! product/test code consumes: thin shims onto the write-once fault-floor hook
//! atoms (the registry that already lives down-face in `kabi/panic`, the
//! always-present seam — Platform-Contract §3.4) and the
//! [`Disposition`]/[`PanicRecord`]/[`SetError`] vocabulary re-exports.
//!
//! ## The hook seam (registration API, not lang items)
//!
//! The seam is six fault-floor atoms owned by `kabi/panic`: five write-once
//! fn-pointer hooks the higher layers register at boot, plus the re-settable
//! AB13 cleanup-context marker (delegated, not write-once):
//!
//! - [`set_flush_fd`] — the LOG7 file sink fd the final flush writes to;
//! - [`set_ring_tail_provider`] — returns a pre-rendered LOG2 byte tail
//!   (the relocated handler writes it verbatim ahead of the panic line — 9.5 §9.1);
//! - [`set_state_record_hook`] — called with the [`PanicRecord`] so the
//!   persistence consumer can record lifecycle + quarantine state;
//! - [`set_disposition`] — selects `recover` (exit 75) or `halt` (exit 70);
//! - [`set_pre_exit_hook`] — a pre-render terminal-restore callback (gap-7);
//! - [`enter_cleanup_context`]/[`clear_cleanup_context`] — raise/lower the AB13
//!   marker so a panic during cleanup records `rollback = failed`.
//!
//! Each `set_*` forwards to `kabi/panic`; the relocated floor handler reads the
//! same atoms through its own `kabi/panic` dep, so a product registration is
//! observed by the handler with no floor→arch edge (master-plan invariant #8).
//!
//! ## Write-once contract (6.2 §5.2)
//!
//! Each hook is an atomic static in `kabi/panic`. Registration writes use
//! `Release`; the panic handler reads use `Acquire`. The first `set_*` wins; a
//! second is rejected with [`SetError::AlreadySet`] and changes nothing. The
//! AB13 cleanup-context marker is the lone re-settable atom (also in `kabi/panic`
//! now — SP03): `enter`/`clear` are plain stores, not a write-once CAS.

// The six fault-floor atoms live in `kabi/panic` — the always-present down-face
// fault-floor seam (Platform-Contract §3.4): five write-once fn-pointer atoms
// (flush fd, ring-tail provider, state-record hook, disposition, pre-exit hook)
// plus the re-settable AB13 cleanup-context marker (CLEANUP_CONTEXT), which is
// NOT write-once — `enter_cleanup_context`/`clear_cleanup_context` delegate
// to `kabi/panic` plain stores, not a CAS. arch keeps only the product-facing
// registration shims that forward to it; the `#[panic_handler]` + render/flush
// mechanism moved to the `reovim-arch-floor-{target}` crates (SP03).
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

/// The signature of a registered ring-tail provider.
type RingTailProvider = fn() -> &'static [u8];
/// The signature of a registered state-record hook.
type StateRecordHook = fn(PanicRecord);
/// The signature of the pre-exit hook (gap-7, #797 Phase 4, 8.2 §2).
type PreExitHook = fn();

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
/// the relocated floor handler writes it verbatim ahead of the panic line and
/// never parses it (9.5 §9.1).
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
/// The marker is the 6th `kabi/panic` fault-floor atom (SP03): this setter and
/// the relocated floor handler reach the SAME static through their shared
/// `kabi/panic` dep, so a product-raised flag is observed by the handler with
/// no floor→arch edge.
///
/// ```no_run
/// // Process-global write — not safe to run in the parallel doctest harness.
/// use reovim_arch::panic::{enter_cleanup_context, clear_cleanup_context};
/// enter_cleanup_context();
/// // ... perform cleanup ...
/// clear_cleanup_context();
/// ```
pub fn enter_cleanup_context() {
    // Delegating shim: the AB13 marker is owned by kabi/panic (the relocated
    // handler reads it there); Release ordering is preserved by the accessor.
    kabi_panic::enter_cleanup_context();
}

/// Lowers the cleanup-context marker raised by [`enter_cleanup_context`].
///
/// ```no_run
/// // Process-global write — not safe to run in the parallel doctest harness.
/// use reovim_arch::panic::clear_cleanup_context;
/// clear_cleanup_context(); // safe to call even without a prior enter
/// ```
pub fn clear_cleanup_context() {
    // Delegating shim: the AB13 marker is owned by kabi/panic.
    kabi_panic::clear_cleanup_context();
}

// L12 layout (#785 Phase 5): the panic-path unit tests (`panic_tests.rs`) are
// PARKED → SP07, not re-homed. They reach `super::{handle, render_*, …}` which
// moved to the floor crate while the hook shims stay here — re-homing would
// split the module across two crates and give the minimal floor crate a
// `selftest`-gated `testrt` dep. Their `reset_registry` helper (a thin forward
// to `kabi/panic::reset` used only to clear the registry between sequential
// no_std cases) parks with them — with panic_tests undeclared it has no caller,
// so it leaves arch this flight rather than dangling as dead code. The
// panic-path mechanism's behaviour is covered at runtime by the
// `panic-{ab13,halt,recover}` fixtures (SP03 Phase 4), which boot the relocated
// handler on a real `*-none` triple. Re-home tracked in 00-master-plan's
// deferred list → SP07; `arch/src/panic_tests.rs` stays in place but undeclared.
