//! `reovim-lib-ds` — portable heap-DS algorithms (the Math half of the floor's
//! data structures).
//!
//! Because the `alloc` crate is forbidden (DAG6), these are the floor's
//! `Vec`/`String`/`HashMap`/`Arc`/`Mutex`/`RwLock`/`Condvar` analogs. Each owns
//! raw memory and is fallible where allocation can fail — but unlike the old
//! `arch::ds`/`arch::sync`, the algorithm reaches its **backend primitive**
//! (allocator, futex park/unpark) through the boot-installed `kabi` platform
//! handle, never through `arch` directly. A value's layout + algorithm is
//! identical on every target; only the injected backend is World.
//!
//! - [`Seq`] — growable sequence (`Vec` analog).
//! - [`Bytes`]/[`Str`] — owned byte/UTF-8 strings.
//! - [`Map`] — open-addressing hash map (`HashMap` analog).
//! - [`Shared`] — atomic-refcount shared reference (`Arc` analog).
//! - [`Ring`] — bounded ring buffer with oldest-first eviction (LOG6 substrate).
//! - [`Mutex`]/[`RwLock`]/[`Condvar`] — futex-backed sync primitives, reached
//!   through the handle's `park`/`unpark`/`unpark_all` wrappers.
//!
//! ## lib/ds is Math (master invariant 2: `lib/ds ⊄ arch`)
//!
//! This crate depends on `reovim-kabi-platform` ONLY. It names no `arch`
//! symbol. `AllocError` is [`reovim_kabi_platform::AllocError`]; the allocator
//! and futex backends are reached via [`reovim_kabi_platform::handle`]. lib/ds
//! is portable: the same source compiles for every target, and which provider's
//! backend it dispatches to is decided at boot by the installed handle.
//!
//! ## Bootstrap prerequisite (no DS before install)
//!
//! Every DS operation that allocates or parks reads the `kabi` handle, which is
//! installed write-once by the boot path (`rust_entry` → arch inits the
//! allocator heap-free → builds the static vtable → installs the handle). No
//! `lib/ds` data structure may be constructed before that install. This is a
//! by-construction boot prerequisite, not a runtime guard: [`handle`] panics on
//! an unset handle, so a pre-install DS op surfaces as a boot-ordering bug. See
//! `Documentation/02-Process/05-Machine-Boot.md` for the boot order.
//!
//! [`handle`]: reovim_kabi_platform::handle
//!
//! ## Doctest note
//!
//! All examples in this crate are marked `no_run`: constructing a `lib/ds`
//! value allocates through the boot-installed `kabi` platform handle, which a
//! doctest process (no `rust_entry`) has not installed; the runtime behaviour
//! is exercised by the booted `arch`-hosted selftests.
#![no_std]
// SAFETY (lint): lib/ds carries DS-LAYOUT unsafe — raw-pointer container
// internals (NonNull, manual `ptr::write`/`read`/`drop_in_place`, slice
// reconstruction) that a `Vec`/`HashMap`/`Arc`/`Mutex` analog needs to manage
// its own memory. This is DISTINCT from the floor's FFI-boundary unsafe in
// `{arch, kabi}`: lib/ds touches no OS, no inline asm, no `extern "C"` vtable —
// every effectful primitive (alloc, park) is reached through the SAFE kabi
// handle wrappers. lib/ds therefore stays Math (portable algorithm; backend
// injected via the kabi handle), not World. The allow is scoped to this crate
// and every `unsafe` block carries a `// SAFETY:` comment; the workspace lint
// stays `warn` so crates above the floor still flag unsafe.
#![allow(unsafe_code)]

mod bytes;
mod condvar;
mod mutex;
mod ring;
mod rwlock;

// `map`/`seq`/`shared` carry `pub`-under-`selftest` internals
// (`FIRST_CAPACITY`, `cyclic_in_range`, `MAX_REFCOUNT`, `refcount_overflowed`)
// that the relocated, `arch`-hosted selftests reach by module path — those
// tests live in `arch` (which owns the no_std runner + live allocator/futex),
// so they cannot use `super::` private access. Expose the module path itself
// only under `selftest`; in shipped builds the modules stay private and only
// the `pub use` re-exports below are visible.
#[cfg(not(feature = "selftest"))]
mod map;
#[cfg(feature = "selftest")]
pub mod map;
#[cfg(not(feature = "selftest"))]
mod seq;
#[cfg(feature = "selftest")]
pub mod seq;
#[cfg(not(feature = "selftest"))]
mod shared;
#[cfg(feature = "selftest")]
pub mod shared;

pub use {
    bytes::{Bytes, BytesWriter, Str},
    condvar::Condvar,
    map::{FxHasher, Map, MapIter},
    mutex::{Mutex, MutexGuard},
    ring::{Ring, RingIter},
    rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard},
    seq::Seq,
    shared::Shared,
};

/// Test-only instrumentation that lets the contention selftests assert a futex
/// slow path executed without relying on timing luck.
///
/// The counters are bumped from inside the `Mutex`/`RwLock` slow paths
/// (`crate::testhooks::note_*`, gated `selftest`). The selftests that READ them
/// live in `arch` (which owns the `no_std` runner, threads, and the live
/// allocator/futex the handle routes to) and reach these through
/// `reovim_lib_ds::testhooks::*` with `reovim-lib-ds/selftest` enabled.
#[cfg(feature = "selftest")]
pub mod testhooks {
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// Counts every `1 -> 2` (uncontended → contended) mutex transition.
    /// A `FUTEX_WAIT`-forcing test asserts this incremented, proving the
    /// contended branch ran.
    static CONTENDED_TRANSITIONS: AtomicUsize = AtomicUsize::new(0);

    /// Counts every `FUTEX_WAIT` a mutex waiter actually entered.
    static WAITS_ENTERED: AtomicUsize = AtomicUsize::new(0);

    /// Counts every `FUTEX_WAIT` an `RwLock` reader entered (blocked behind a
    /// writer past the spin limit). A writer-blocks-reader test asserts this
    /// incremented, proving `read()`'s `FUTEX_WAIT` line ran.
    static RWLOCK_READER_WAITS: AtomicUsize = AtomicUsize::new(0);

    /// Counts every lost-CAS spin a contending `RwLock` writer took (the
    /// `s == 0` retry arm of `write()`).
    static RWLOCK_WRITER_LOST_CAS: AtomicUsize = AtomicUsize::new(0);

    /// Counts every lost-CAS retry a contending `RwLock` reader took (the
    /// "CAS lost a race; retry" arm of `read()` when `WRITER == 0`).
    static RWLOCK_READER_CAS_RETRY: AtomicUsize = AtomicUsize::new(0);

    pub(crate) fn note_contended_transition() {
        CONTENDED_TRANSITIONS.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn note_wait_entered() {
        WAITS_ENTERED.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn note_rwlock_reader_wait() {
        RWLOCK_READER_WAITS.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn note_rwlock_writer_lost_cas() {
        RWLOCK_WRITER_LOST_CAS.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn note_rwlock_reader_cas_retry() {
        RWLOCK_READER_CAS_RETRY.fetch_add(1, Ordering::Relaxed);
    }

    /// The current contended-transition count (test observation point).
    #[must_use]
    pub fn contended_transitions() -> usize {
        CONTENDED_TRANSITIONS.load(Ordering::Relaxed)
    }

    /// The current waits-entered count (test observation point; consumed
    /// only by the Linux-gated, spawn-dependent cases, as are the rwlock
    /// observation points below).
    #[cfg(target_os = "linux")]
    #[must_use]
    pub fn waits_entered() -> usize {
        WAITS_ENTERED.load(Ordering::Relaxed)
    }

    /// The current `RwLock` reader-wait count (test observation point).
    #[cfg(target_os = "linux")]
    #[must_use]
    pub fn rwlock_reader_waits() -> usize {
        RWLOCK_READER_WAITS.load(Ordering::Relaxed)
    }

    /// The current `RwLock` writer lost-CAS count (test observation point).
    #[cfg(target_os = "linux")]
    #[must_use]
    pub fn rwlock_writer_lost_cas() -> usize {
        RWLOCK_WRITER_LOST_CAS.load(Ordering::Relaxed)
    }

    /// The current `RwLock` reader CAS-retry count (test observation point).
    #[cfg(target_os = "linux")]
    #[must_use]
    pub fn rwlock_reader_cas_retry() -> usize {
        RWLOCK_READER_CAS_RETRY.load(Ordering::Relaxed)
    }
}
