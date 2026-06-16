//! `RwLock<T>` — a single-word reader/writer lock over `futex` (2.3 §1.1).
//!
//! ## Protocol (single `AtomicU32` state word)
//!
//! The lock packs both the writer flag and the reader count into one word
//! `state`:
//!
//! ```text
//!   bit 31        : WRITER  — a writer holds (or is installing) the lock.
//!   bits 0..=30   : reader count (number of live read guards).
//! ```
//!
//! Invariants:
//!
//! - `WRITER` set ⇒ reader count is `0` (writers are exclusive).
//! - reader count `> 0` ⇒ `WRITER` clear (readers exclude writers).
//! - `state == 0` ⇒ the lock is fully free.
//!
//! Acquisition rules:
//!
//! - **`read()`**: spin/`compare_exchange` to increment the reader count, but
//!   only while `WRITER` is clear. If a writer holds, the reader
//!   `FUTEX_WAIT`s on `state` until it changes, then retries. Entry is
//!   `Acquire` so the reader sees the previous writer's writes.
//! - **`write()`**: `compare_exchange` `0 -> WRITER`. If the word is non-zero
//!   (readers present or another writer), `FUTEX_WAIT` on `state` and retry.
//!   Acquisition is `Acquire`.
//!
//! Release rules (both `Release` stores per 2.3 §1.1, with `FUTEX_WAKE`
//! issued **after** the store):
//!
//! - **read release**: `fetch_sub(1, Release)`; if the count dropped to `0`,
//!   `FUTEX_WAKE` all (a waiting writer needs the wake; waiting readers are
//!   harmlessly woken and re-check).
//! - **write release**: `store(0, Release)` then `FUTEX_WAKE` all (wake every
//!   blocked reader and the next writer; the word re-check serializes them).
//!
//! Waking *all* on release is the simplest correct scheme (no reader/writer
//! starvation guarantee is promised — the floor has no fairness consumer
//! yet; rule of three). Every woken thread re-checks `state`, so spurious
//! wakes are absorbed (2.3 §1.1).

use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{
        AtomicU32,
        Ordering::{Acquire, Relaxed, Release},
    },
};

use reovim_kabi_platform::handle;

/// High bit of the state word: a writer holds the lock.
const WRITER: u32 = 1 << 31;
/// Mask for the reader-count bits.
#[cfg(all(feature = "selftest", target_os = "linux"))]
const READERS: u32 = WRITER - 1;
/// Spin attempts before falling back to a `park`.
const SPIN_LIMIT: u32 = 100;

/// A reader/writer lock guarding a `T`.
///
/// No poisoning (see the [`crate`] module doc): under `panic = "abort"`
/// a poisoned state can never be observed.
///
/// ```no_run
/// use reovim_lib_ds::RwLock;
///
/// let rw = RwLock::new(0u32);
/// // Multiple concurrent readers are fine in a single-threaded context.
/// let r1 = rw.read();
/// let r2 = rw.read();
/// assert_eq!(*r1, 0);
/// assert_eq!(*r2, 0);
/// drop(r1);
/// drop(r2);
/// // Exclusive write.
/// let mut w = rw.write();
/// *w = 42;
/// drop(w);
/// assert_eq!(*rw.read(), 42);
/// ```
pub struct RwLock<T> {
    /// Packed writer-flag + reader-count word; also the futex address.
    state: AtomicU32,
    /// The guarded value, reached only while a guard is held.
    value: UnsafeCell<T>,
}

// SAFETY: the state word serializes access: writers gain exclusive access,
// readers gain shared `&T`. `Send` needs `T: Send` (the value may move across
// threads under the write lock); `Sync` needs `T: Send + Sync` because
// concurrent readers hold `&T` on multiple threads simultaneously.
unsafe impl<T: Send> Send for RwLock<T> {}
// SAFETY: a `&RwLock<T>` hands out concurrent `&T` to readers across threads,
// requiring `T: Sync`, and `&mut T` to a writer, requiring `T: Send`.
unsafe impl<T: Send + Sync> Sync for RwLock<T> {}

impl<T> RwLock<T> {
    /// Creates a new, unlocked reader/writer lock wrapping `value`.
    ///
    /// ```no_run
    /// use reovim_lib_ds::RwLock;
    /// let rw = RwLock::new(3u32);
    /// assert_eq!(*rw.read(), 3);
    /// ```
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            value: UnsafeCell::new(value),
        }
    }

    /// Acquires a shared read lock, blocking while a writer holds.
    ///
    /// ```no_run
    /// use reovim_lib_ds::RwLock;
    /// let rw = RwLock::new(1u32);
    /// let g = rw.read();
    /// assert_eq!(*g, 1);
    /// ```
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        let mut spins = 0;
        loop {
            let s = self.state.load(Relaxed);
            if s & WRITER == 0 {
                // No writer: try to bump the reader count with Acquire.
                if self
                    .state
                    .compare_exchange_weak(s, s + 1, Acquire, Relaxed)
                    .is_ok()
                {
                    return RwLockReadGuard { lock: self };
                }
                // CAS lost a race; retry without sleeping.
                #[cfg(feature = "selftest")]
                crate::testhooks::note_rwlock_reader_cas_retry();
                core::hint::spin_loop();
                continue;
            }
            // A writer holds. Spin briefly, then sleep on the word.
            if spins < SPIN_LIMIT {
                spins += 1;
                core::hint::spin_loop();
                continue;
            }
            // The expected value passed to `park` is the writer-held word we
            // just observed; if it changed, `park` returns immediately and the
            // loop re-checks.
            #[cfg(feature = "selftest")]
            crate::testhooks::note_rwlock_reader_wait();
            handle().park(&self.state, s);
        }
    }

    /// Acquires an exclusive write lock, blocking while any guard is held.
    ///
    /// ```no_run
    /// use reovim_lib_ds::RwLock;
    /// let rw = RwLock::new(0u32);
    /// *rw.write() = 7;
    /// assert_eq!(*rw.read(), 7);
    /// ```
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        let mut spins = 0;
        loop {
            // Only `0 -> WRITER` is a valid acquire (no readers, no writer).
            if self
                .state
                .compare_exchange_weak(0, WRITER, Acquire, Relaxed)
                .is_ok()
            {
                return RwLockWriteGuard { lock: self };
            }
            let s = self.state.load(Relaxed);
            if spins < SPIN_LIMIT && s != 0 {
                spins += 1;
                core::hint::spin_loop();
                continue;
            }
            if s == 0 {
                // Lost a CAS race against another acquirer; retry at once.
                #[cfg(feature = "selftest")]
                crate::testhooks::note_rwlock_writer_lost_cas();
                core::hint::spin_loop();
                continue;
            }
            handle().park(&self.state, s);
        }
    }

    /// Releases a read guard.
    fn read_unlock(&self) {
        // Release so the next writer sees this reader's writes. If the count
        // reached zero, a writer may be waiting — wake everyone (`unpark_all`).
        if self.state.fetch_sub(1, Release) == 1 {
            handle().unpark_all(&self.state);
        }
    }

    /// Releases a write guard.
    fn write_unlock(&self) {
        // Release publishes the writer's mutations; wake all (`unpark_all`) so a
        // blocked writer or any blocked readers re-check the now-free word.
        self.state.store(0, Release);
        handle().unpark_all(&self.state);
    }

    /// The current number of live read guards (selftest observation hook;
    /// `pub` under `selftest` so the arch-hosted `rwlock` tests, which own the
    /// `no_std` runner, can assert it — only their Linux-gated, spawn-dependent
    /// cases do).
    #[cfg(all(feature = "selftest", target_os = "linux"))]
    #[must_use]
    pub fn reader_count(&self) -> u32 {
        self.state.load(Relaxed) & READERS
    }
}

/// An RAII shared-read guard; releases the read lock on drop. Derefs to `&T`.
///
/// See [`RwLock::read`] for usage.
#[must_use = "the read lock is released when the guard is dropped"]
pub struct RwLockReadGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<T> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: a read guard proves the writer flag is clear, so shared
        // `&T` access is sound (no exclusive writer can coexist).
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.read_unlock();
    }
}

/// An RAII exclusive-write guard; releases the write lock on drop. Derefs to
/// `&mut T`.
///
/// See [`RwLock::write`] for usage.
#[must_use = "the write lock is released when the guard is dropped"]
pub struct RwLockWriteGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<T> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: a write guard proves exclusive ownership of the lock word.
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: a write guard is exclusive, so `&mut T` is sound.
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.write_unlock();
    }
}
