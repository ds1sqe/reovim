//! Futex-backed synchronization primitives for the platform floor.
//!
//! The `std`-free `Mutex`/`RwLock`/`Condvar` the kernel boot core (turn
//! gate, sharded tables) and every higher crate consume. Each is built
//! directly over the arch `futex` syscall (2.3 §1.1, the normative
//! arch sync-primitive ordering contract):
//!
//! - Lock acquisition is an **`Acquire`** atomic op on the lock word;
//!   unlock is a **`Release`** store. Every write under the lock
//!   happens-before the next holder's first read.
//! - A futex **wait** re-checks the lock word after every wake; spurious
//!   and stolen wakes are absorbed by the retry loop.
//! - `FUTEX_WAKE` is issued **after** the `Release` store of the unlock,
//!   so a woken winner observes the holder's writes.
//! - `Condvar::wait` atomically releases its `Mutex` before sleeping and
//!   re-acquires under the same contract before returning; the missed-wake
//!   window between release and sleep is closed by the futex word re-check.
//! - `RwLock` readers take `Acquire` on entry, `Release` on exit; the
//!   writer path provides the same edges as `Mutex`.
//!
//! ## No poisoning
//!
//! These primitives carry **no poisoning concept**. Under `panic = "abort"`
//! (AB12, 1.2 §10 DAG6) a panic terminates the process — a lock can never
//! be left "poisoned" by a panicking holder because there is no surviving
//! thread to observe the poison. A poisoned-state API would be dead code,
//! so it is omitted (DEV1: code whose branches cannot be exercised is not
//! written).

mod condvar;
mod mutex;
mod rwlock;

#[cfg(feature = "selftest")]
mod condvar_tests;
#[cfg(feature = "selftest")]
mod mutex_tests;
#[cfg(feature = "selftest")]
mod rwlock_tests;

pub use {
    condvar::Condvar,
    mutex::{Mutex, MutexGuard},
    rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

#[cfg(feature = "selftest")]
mod testhooks {
    //! Test-only instrumentation that lets the contention tests assert the
    //! futex slow path executed without relying on timing luck.
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
    /// A many-concurrent-readers test asserts this incremented, proving
    /// lines 117-120 of `rwlock.rs` executed.
    static RWLOCK_READER_CAS_RETRY: AtomicUsize = AtomicUsize::new(0);

    pub(super) fn note_contended_transition() {
        CONTENDED_TRANSITIONS.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn note_wait_entered() {
        WAITS_ENTERED.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn note_rwlock_reader_wait() {
        RWLOCK_READER_WAITS.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn note_rwlock_writer_lost_cas() {
        RWLOCK_WRITER_LOST_CAS.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn note_rwlock_reader_cas_retry() {
        RWLOCK_READER_CAS_RETRY.fetch_add(1, Ordering::Relaxed);
    }

    /// The current contended-transition count (test observation point).
    pub fn contended_transitions() -> usize {
        CONTENDED_TRANSITIONS.load(Ordering::Relaxed)
    }

    /// The current waits-entered count (test observation point; consumed
    /// only by the Linux-gated, spawn-dependent cases, as are the rwlock
    /// observation points below).
    #[cfg(target_os = "linux")]
    pub fn waits_entered() -> usize {
        WAITS_ENTERED.load(Ordering::Relaxed)
    }

    /// The current `RwLock` reader-wait count (test observation point).
    #[cfg(target_os = "linux")]
    pub fn rwlock_reader_waits() -> usize {
        RWLOCK_READER_WAITS.load(Ordering::Relaxed)
    }

    /// The current `RwLock` writer lost-CAS count (test observation point).
    #[cfg(target_os = "linux")]
    pub fn rwlock_writer_lost_cas() -> usize {
        RWLOCK_WRITER_LOST_CAS.load(Ordering::Relaxed)
    }

    /// The current `RwLock` reader CAS-retry count (test observation point).
    #[cfg(target_os = "linux")]
    pub fn rwlock_reader_cas_retry() -> usize {
        RWLOCK_READER_CAS_RETRY.load(Ordering::Relaxed)
    }
}
