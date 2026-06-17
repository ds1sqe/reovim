//! `reovim-kabi-panic` — the always-present write-once fault-floor seam.
//!
//! `kabi/panic` is the down-face counterpart of `uapi/` for the panic path:
//! it owns the process-global, write-once atomic statics the AB12 panic handler
//! reads at any boot stage. Unlike `kabi/platform` (which requires
//! `Init::boot` to have run before the handle is readable), **these atoms carry
//! no install gate** — a panic can fire before `Init::boot` completes, so the
//! fault hooks must exist unconditionally (Platform-Contract §3.4).
//!
//! ## Atom set
//!
//! Five independently-optional write-once slots mirror the registration surface
//! that `arch/src/panic.rs` currently owns (6.2 §5.2):
//!
//! | Atom | Storage | Sentinel | [`uapi/panic`] alias |
//! |---|---|---|---|
//! | flush fd | `AtomicI32` | `-1` | — (raw fd, not a hook alias) |
//! | ring-tail provider | `AtomicUsize` | `0` | [`RingTailProviderFn`] |
//! | state-record hook | `AtomicUsize` | `0` | [`StateRecordHookFn`] |
//! | disposition | `AtomicU8` | `0` (unset) | — ([`Disposition`] encoded as `u8`) |
//! | pre-exit hook | `AtomicUsize` | `0` | [`PreExitHookFn`] |
//!
//! The atoms are independent: none depends on another being set. The handler
//! reads each independently and falls back to safe defaults (stderr, halt) when
//! any is unset (6.2 §5.2).
//!
//! ## Write-once contract (6.2 §5.2)
//!
//! Every setter uses `compare_exchange(sentinel, value, Release, Acquire)`.
//! The first writer wins; a second is rejected with [`SetError::AlreadySet`]
//! and changes nothing. Readers use `load(Acquire)`, pairing with the setter's
//! `Release` — everything written before registration is visible to a panic on
//! any thread after it.
//!
//! ## Why `unsafe` is allowed here (the floor is `{arch, kabi}`)
//!
//! The fn-pointer atoms store addresses as `usize` and reconstruct the original
//! fn-pointer type in the getter with `transmute`. That transmute is an
//! irreducible unsafe operation: soundness rests on the invariant that the
//! stored address was written by the matching setter from exactly the right fn
//! type and is never mutated after. `kabi/panic` encapsulates that unsafe behind
//! a safe setter/getter API so consumers above the floor stay safe. The
//! workspace lint stays `warn`, so any crate above the floor that introduces
//! `unsafe` still flags; the allow here is scoped to this floor-contract crate,
//! mirroring `kabi/platform`'s crate-scoped allow (lib.rs:50-56).
#![no_std]
// SAFETY (lint): kabi/panic is the down-face fault-floor contract — it stores
// fn-pointer hook addresses as `usize` in atomic statics and reconstructs them
// in the getters via `transmute`. unsafe cannot be expressed away at the
// fn-pointer storage boundary; it is encapsulated here so consumers above the
// floor stay safe. The workspace lint stays `warn` so crates above the floor
// still flag unsafe.
#![allow(unsafe_code)]

use core::sync::atomic::{
    AtomicI32, AtomicU8, AtomicUsize,
    Ordering::{Acquire, Release},
};

// Re-export the uapi/panic value types so callers can import them from one
// location without a direct edge to uapi/panic. The `pub use` also serves as
// the module-internal import for the setter/getter function signatures below.
pub use reovim_uapi_panic::{
    Disposition, PanicRecord, PreExitHookFn, RingTailProviderFn, StateRecordHookFn,
};

// ── write-once atomic statics (6.2 §5.2) ─────────────────────────────────────
//
// Five atoms mirror the full registration surface of `arch/src/panic.rs`.
// They are individually optional — the handler falls back to safe defaults when
// any is unset. Storing fn pointers as their address in `AtomicUsize` (0 =
// unregistered) is the same discipline arch uses; a `Disposition` is stored as
// an encoded `u8` in `AtomicU8` (0 = unregistered).

/// The LOG7 file-sink fd for the final flush. `-1` = unregistered.
static FLUSH_FD: AtomicI32 = AtomicI32::new(FLUSH_FD_UNSET);

/// The ring-tail provider address. `0` = unregistered.
static RING_TAIL_PROVIDER: AtomicUsize = AtomicUsize::new(0);

/// The state-record hook address. `0` = unregistered.
static STATE_RECORD_HOOK: AtomicUsize = AtomicUsize::new(0);

/// The disposition encoded as `u8`. `DISPOSITION_UNSET` = unregistered.
static DISPOSITION: AtomicU8 = AtomicU8::new(DISPOSITION_UNSET);

/// The pre-exit hook address. `0` = unregistered.
static PRE_EXIT_HOOK: AtomicUsize = AtomicUsize::new(0);

// ── sentinel constants ────────────────────────────────────────────────────────

/// Sentinel for `FLUSH_FD`: no fd registered (handler writes to stderr, fd 2).
const FLUSH_FD_UNSET: i32 = -1;

/// Sentinel for `DISPOSITION`: nothing registered (handler defaults to `halt`,
/// 6.2 §5.2).
const DISPOSITION_UNSET: u8 = 0;

/// Encoded `Disposition::Recover` (must be non-zero and != `DISPOSITION_UNSET`).
const DISPOSITION_RECOVER: u8 = 1;

/// Encoded `Disposition::Halt` (must be non-zero and != `DISPOSITION_UNSET`).
const DISPOSITION_HALT: u8 = 2;

// ── error type ───────────────────────────────────────────────────────────────

/// Why a `set_*` registration was rejected.
///
/// The only variant mirrors `arch::panic::SetError` and
/// `kabi/platform::InstallError`: a double-registration is a boot-stage bug,
/// surfaced as a typed error rather than silently overwriting the live value
/// (6.2 §5.2 write-once).
///
/// ```rust
/// use reovim_kabi_panic::SetError;
///
/// let e = SetError::AlreadySet;
/// assert_eq!(format!("{e:?}"), "AlreadySet");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetError {
    /// The slot was already registered; the first registration wins and this
    /// call changed nothing (6.2 §5.2 write-once).
    AlreadySet,
}

// ── setters ──────────────────────────────────────────────────────────────────

/// Registers the final-flush file descriptor (write-once, 6.2 §5.2).
///
/// The panic handler writes the ring tail and the panic line to this fd when
/// it is set; with no fd registered the line goes to fd 2 (stderr, the safe
/// posture for an unbooted process — 6.2 §5.2 default).
///
/// # Errors
///
/// Returns [`SetError::AlreadySet`] if a flush fd is already registered; the
/// first registration wins and this call changes nothing.
///
/// ```no_run
/// // Write-once process-global state — not safe to run in the doctest
/// // harness because parallel doctests share the process and the static
/// // is write-once.
/// use reovim_kabi_panic::set_flush_fd;
/// set_flush_fd(2).expect("first registration always succeeds");
/// ```
pub fn set_flush_fd(fd: i32) -> Result<(), SetError> {
    // CAS from the sentinel (-1): the first writer wins. Release so a panic on
    // any thread observing the non-sentinel fd also observes prior writes.
    FLUSH_FD
        .compare_exchange(FLUSH_FD_UNSET, fd, Release, Acquire)
        .map(|_| ())
        .map_err(|_| SetError::AlreadySet)
}

/// Registers the ring-tail provider (write-once, 6.2 §5.2).
///
/// The provider returns one contiguous `&[u8]` already in LOG2 line format;
/// the panic handler writes it verbatim ahead of the panic line and never
/// parses it (9.5 §9.1). The provider is only called when a flush fd is also
/// registered — without a sink the tail is meaningless.
///
/// # Errors
///
/// Returns [`SetError::AlreadySet`] if a provider is already registered.
///
/// ```no_run
/// // Write-once process-global state — not safe to run in the doctest harness.
/// use reovim_kabi_panic::set_ring_tail_provider;
/// fn my_tail() -> &'static [u8] { b"" }
/// set_ring_tail_provider(my_tail).expect("first registration always succeeds");
/// ```
pub fn set_ring_tail_provider(provider: RingTailProviderFn) -> Result<(), SetError> {
    // The fn pointer is stored as its address; `get_ring_tail_provider`
    // reconstructs exactly this type from it.
    let addr = (provider as *const ()).addr();
    set_fn_hook(&RING_TAIL_PROVIDER, addr)
}

/// Registers the state-record hook (write-once, 6.2 §5.2).
///
/// The panic handler calls the hook with the [`PanicRecord`] before
/// terminating, so the persistence consumer can record lifecycle + quarantine
/// state (6.2 §5 step 3, AB13).
///
/// # Errors
///
/// Returns [`SetError::AlreadySet`] if a hook is already registered.
///
/// ```no_run
/// // Write-once process-global state — not safe to run in the doctest harness.
/// use reovim_kabi_panic::{PanicRecord, set_state_record_hook};
/// fn my_hook(_: PanicRecord) {}
/// set_state_record_hook(my_hook).expect("first registration always succeeds");
/// ```
pub fn set_state_record_hook(hook: StateRecordHookFn) -> Result<(), SetError> {
    // The fn pointer is stored as its address; `get_state_record_hook`
    // reconstructs exactly this type from it.
    let addr = (hook as *const ()).addr();
    set_fn_hook(&STATE_RECORD_HOOK, addr)
}

/// Registers the panic disposition (write-once, 6.2 §5.2).
///
/// Selects `recover` (exit 75, `EX_TEMPFAIL`) or `halt` (exit 70,
/// `EX_SOFTWARE`). With nothing registered the handler defaults to `halt` —
/// the safe posture for a process that never finished boot (6.2 §5.2).
///
/// # Errors
///
/// Returns [`SetError::AlreadySet`] if a disposition is already registered.
///
/// ```no_run
/// // Write-once process-global state — not safe to run in the doctest harness.
/// use reovim_kabi_panic::{Disposition, set_disposition};
/// set_disposition(Disposition::Recover).expect("first registration always succeeds");
/// ```
pub fn set_disposition(disposition: Disposition) -> Result<(), SetError> {
    let encoded = match disposition {
        Disposition::Recover => DISPOSITION_RECOVER,
        Disposition::Halt => DISPOSITION_HALT,
    };
    DISPOSITION
        .compare_exchange(DISPOSITION_UNSET, encoded, Release, Acquire)
        .map(|_| ())
        .map_err(|_| SetError::AlreadySet)
}

/// Registers the pre-exit callback (write-once, 6.2 §5.2, gap-7, 8.2 §2).
///
/// The panic handler invokes this hook BEFORE rendering its final output, so
/// a platform runtime (e.g. the TUI) can restore terminal state and have the
/// panic line appear in cooked mode rather than raw mode.
///
/// # Errors
///
/// Returns [`SetError::AlreadySet`] if a hook is already registered.
///
/// ```no_run
/// // Write-once process-global state — not safe to run in the doctest harness.
/// use reovim_kabi_panic::set_pre_exit_hook;
/// fn restore_terminal() {}
/// set_pre_exit_hook(restore_terminal).expect("first registration always succeeds");
/// ```
pub fn set_pre_exit_hook(hook: PreExitHookFn) -> Result<(), SetError> {
    // The fn pointer is stored as its address; `get_pre_exit_hook`
    // reconstructs exactly this type from it.
    let addr = (hook as *const ()).addr();
    set_fn_hook(&PRE_EXIT_HOOK, addr)
}

// ── getters ──────────────────────────────────────────────────────────────────

/// Returns the registered flush fd, or `None` if not yet registered.
///
/// A `None` return means the panic handler should write to stderr (fd 2), the
/// safe default for an unbooted process (6.2 §5.2). This getter requires no
/// `kabi/platform::install` to have run — the atom is unconditionally readable
/// (Platform-Contract §3.4).
///
/// ```rust
/// use reovim_kabi_panic::get_flush_fd;
///
/// // Unregistered: returns None with no install gate required.
/// // (This atom is not set by other tests in the same process, so None is safe.)
/// // Note: in a real test process other tests may have set this atom.
/// // See tests.rs for the process-global write-once test strategy.
/// let _ = get_flush_fd(); // None or Some(_) depending on process state.
/// ```
#[must_use]
pub fn get_flush_fd() -> Option<i32> {
    let fd = FLUSH_FD.load(Acquire);
    if fd == FLUSH_FD_UNSET { None } else { Some(fd) }
}

/// Returns the registered ring-tail provider, or `None` if not yet registered.
///
/// Requires no `kabi/platform::install` to have run (Platform-Contract §3.4).
///
/// ```rust
/// use reovim_kabi_panic::get_ring_tail_provider;
///
/// // Readable with zero atoms set — no install gate.
/// let _ = get_ring_tail_provider();
/// ```
#[must_use]
pub fn get_ring_tail_provider() -> Option<RingTailProviderFn> {
    match RING_TAIL_PROVIDER.load(Acquire) {
        0 => None,
        // SAFETY: the static holds either 0 (handled above) or the address of
        // a `RingTailProviderFn` stored once by `set_ring_tail_provider` via
        // `(provider as *const ()).addr()` from exactly this fn type. The
        // address is immutable after the first CAS, so reconstructing the same
        // fn pointer here is sound.
        addr => Some(unsafe { core::mem::transmute::<usize, RingTailProviderFn>(addr) }),
    }
}

/// Returns the registered state-record hook, or `None` if not yet registered.
///
/// Requires no `kabi/platform::install` to have run (Platform-Contract §3.4).
///
/// ```rust
/// use reovim_kabi_panic::get_state_record_hook;
///
/// // Readable with zero atoms set — no install gate.
/// let _ = get_state_record_hook();
/// ```
#[must_use]
pub fn get_state_record_hook() -> Option<StateRecordHookFn> {
    match STATE_RECORD_HOOK.load(Acquire) {
        0 => None,
        // SAFETY: as `get_ring_tail_provider`: the address was written once by
        // `set_state_record_hook` from exactly the `StateRecordHookFn` type;
        // reconstructing the same fn pointer is sound.
        addr => Some(unsafe { core::mem::transmute::<usize, StateRecordHookFn>(addr) }),
    }
}

/// Returns the registered disposition, or `None` if not yet registered.
///
/// `None` means the handler defaults to [`Disposition::Halt`] (6.2 §5.2).
/// Requires no `kabi/platform::install` to have run (Platform-Contract §3.4).
///
/// ```rust
/// use reovim_kabi_panic::get_disposition;
///
/// // Readable with zero atoms set — no install gate.
/// let _ = get_disposition();
/// ```
#[must_use]
pub fn get_disposition() -> Option<Disposition> {
    match DISPOSITION.load(Acquire) {
        DISPOSITION_RECOVER => Some(Disposition::Recover),
        DISPOSITION_HALT => Some(Disposition::Halt),
        // Either DISPOSITION_UNSET (0) or an unrecognised byte — treat as unset.
        _ => None,
    }
}

/// Returns the registered pre-exit hook, or `None` if not yet registered.
///
/// Requires no `kabi/platform::install` to have run (Platform-Contract §3.4).
///
/// ```rust
/// use reovim_kabi_panic::get_pre_exit_hook;
///
/// // Readable with zero atoms set — no install gate.
/// let _ = get_pre_exit_hook();
/// ```
#[must_use]
pub fn get_pre_exit_hook() -> Option<PreExitHookFn> {
    match PRE_EXIT_HOOK.load(Acquire) {
        0 => None,
        // SAFETY: the address was written once by `set_pre_exit_hook` from
        // exactly the `PreExitHookFn` type (`fn()`); reconstructing the same
        // fn pointer is sound.
        addr => Some(unsafe { core::mem::transmute::<usize, PreExitHookFn>(addr) }),
    }
}

// ── shared helper ─────────────────────────────────────────────────────────────

/// Shared write-once CAS for a fn-pointer hook stored as a `usize` address.
///
/// Performs `compare_exchange(0, value, Release, Acquire)` — the first writer
/// wins; a second is rejected. Returns `Err(SetError::AlreadySet)` on failure.
fn set_fn_hook(slot: &AtomicUsize, value: usize) -> Result<(), SetError> {
    slot.compare_exchange(0, value, Release, Acquire)
        .map(|_| ())
        .map_err(|_| SetError::AlreadySet)
}

// ── selftest reset ─────────────────────────────────────────────────────────

/// Resets every fault-floor atom to its unregistered sentinel (selftest only).
///
/// The write-once atoms are process-global; a real process never resets them
/// (write-once is permanent — 6.2 §5.2). This reset exists only for the no_std
/// selftest runner, which executes many panic-path cases in one
/// single-threaded sequential binary and must restore the registry to a clean
/// state between cases. It is gated on the `selftest` feature so the production
/// fault floor carries no reset path. Mirrors what `arch::panic::reset_registry`
/// did before the atoms were relocated here; arch's `reset_registry` now
/// forwards to this function (and still resets its own `CLEANUP_CONTEXT` flag).
#[cfg(feature = "selftest")]
pub fn reset() {
    use core::sync::atomic::Ordering::Relaxed;
    FLUSH_FD.store(FLUSH_FD_UNSET, Relaxed);
    RING_TAIL_PROVIDER.store(0, Relaxed);
    STATE_RECORD_HOOK.store(0, Relaxed);
    DISPOSITION.store(DISPOSITION_UNSET, Relaxed);
    PRE_EXIT_HOOK.store(0, Relaxed);
}

// L12 layout: tests live in the sibling file `tests.rs`, declared as a
// `#[path]` child so `super::` reaches the private statics and the sentinel
// constants. These atoms are process-global write-once, so each test is
// designed to own distinct atoms to avoid inter-test races (see tests.rs).
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
