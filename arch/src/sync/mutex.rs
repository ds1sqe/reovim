//! `Mutex<T>` — a three-state futex mutex (2.3 §1.1).
//!
//! The lock word is an `AtomicU32` with three states (Drepper, "Futexes Are
//! Tricky"):
//!
//! - `0` — free (unlocked).
//! - `1` — locked, no waiters (uncontended).
//! - `2` — locked, one or more waiters may be blocked (contended).
//!
//! The contended state is sticky: once a thread blocks in `FUTEX_WAIT`, the
//! word is `2` and stays `2` until an unlock that observes `2` issues a
//! `FUTEX_WAKE`. This guarantees a sleeping waiter is always woken, at the
//! cost of one possibly-spurious wake per unlock-from-contended.

use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{
        AtomicU32,
        Ordering::{Acquire, Relaxed, Release},
    },
};

use crate::sys::{FUTEX_PRIVATE_FLAG, FUTEX_WAIT, FUTEX_WAKE, futex};

/// Lock-word state: free.
const FREE: u32 = 0;
/// Lock-word state: locked, no known waiters.
const LOCKED: u32 = 1;
/// Lock-word state: locked, a waiter may be blocked.
const CONTENDED: u32 = 2;

/// Spin attempts before falling back to `FUTEX_WAIT`. A short bounded spin
/// absorbs brief contention without a syscall; past it, sleeping is cheaper
/// than burning the core.
const SPIN_LIMIT: u32 = 100;

/// A mutual-exclusion lock guarding a `T`.
///
/// No poisoning: under `panic = "abort"` a panicking holder kills the
/// process, so a poisoned state can never be observed (see the module doc).
///
/// ```rust
/// use reovim_arch::sync::Mutex;
///
/// let m = Mutex::new(0u32);
/// {
///     let mut g = m.lock();
///     *g = 42;
/// } // guard dropped → lock released
/// let g = m.lock();
/// assert_eq!(*g, 42);
/// ```
pub struct Mutex<T> {
    /// The three-state lock word (`FREE`/`LOCKED`/`CONTENDED`).
    word: AtomicU32,
    /// The guarded value, reached only while the word is held.
    value: UnsafeCell<T>,
}

// SAFETY: the mutex serializes all access to `value` through the lock word,
// so concurrent use is data-race-free provided `T` itself is `Send` (the
// value moves across threads only under the lock). `Sync` follows the same
// reasoning: a `&Mutex<T>` lets another thread lock and obtain `&mut T`,
// which is sound for any `T: Send`.
unsafe impl<T: Send> Send for Mutex<T> {}
// SAFETY: see above; a shared `&Mutex<T>` only ever hands out access under
// the lock, so `T: Send` suffices.
unsafe impl<T: Send> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    /// Creates a new, unlocked mutex wrapping `value`.
    ///
    /// ```rust
    /// use reovim_arch::sync::Mutex;
    /// let m = Mutex::new(7u32);
    /// assert_eq!(*m.lock(), 7);
    /// ```
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self {
            word: AtomicU32::new(FREE),
            value: UnsafeCell::new(value),
        }
    }

    /// Addr of the lock word, for the raw `futex` syscall.
    fn word_addr(&self) -> usize {
        core::ptr::from_ref(&self.word).addr()
    }

    /// Acquires the lock, blocking via `FUTEX_WAIT` if it is held.
    ///
    /// The returned [`MutexGuard`] releases the lock on drop.
    ///
    /// ```rust
    /// use reovim_arch::sync::Mutex;
    /// let m = Mutex::new(0u32);
    /// let mut g = m.lock();
    /// *g += 1;
    /// drop(g);
    /// assert_eq!(*m.lock(), 1);
    /// ```
    pub fn lock(&self) -> MutexGuard<'_, T> {
        // Fast path: an uncontended `FREE -> LOCKED` acquire.
        if self
            .word
            .compare_exchange(FREE, LOCKED, Acquire, Relaxed)
            .is_ok()
        {
            return MutexGuard { mutex: self };
        }
        self.lock_contended();
        MutexGuard { mutex: self }
    }

    /// The slow path: spin briefly, then transition to `CONTENDED` and
    /// `FUTEX_WAIT` until the word is won.
    fn lock_contended(&self) {
        // Bounded spin: retry the uncontended acquire while the word is FREE.
        let mut spins = 0;
        while spins < SPIN_LIMIT {
            if self
                .word
                .compare_exchange(FREE, LOCKED, Acquire, Relaxed)
                .is_ok()
            {
                return;
            }
            // If the word is already CONTENDED, do not spin: a waiter is
            // queued, so sleeping is correct. Otherwise keep spinning.
            if self.word.load(Relaxed) == CONTENDED {
                break;
            }
            core::hint::spin_loop();
            spins += 1;
        }

        // Sleep loop. Mark the word CONTENDED (swap returns the prior state);
        // if it was not already FREE, a holder exists and we must wait.
        loop {
            // `swap` to CONTENDED with Acquire: if the prior value was FREE we
            // won the lock (and leave it CONTENDED, conservatively waking on
            // unlock); otherwise we sleep.
            let prev = self.word.swap(CONTENDED, Acquire);
            if prev == FREE {
                return;
            }
            #[cfg(feature = "selftest")]
            if prev == LOCKED {
                super::testhooks::note_contended_transition();
            }
            // Wait while the word is CONTENDED. A mismatched value (EAGAIN)
            // or any wake re-loops and re-checks via the swap above. This
            // re-check absorbs spurious/stolen wakes (2.3 §1.1).
            #[cfg(feature = "selftest")]
            super::testhooks::note_wait_entered();
            // The futex result is intentionally ignored: EAGAIN (value moved
            // off CONTENDED) and a genuine wake both lead back to the swap,
            // which re-decides. Errors other than EAGAIN cannot occur for a
            // private futex on a valid word.
            let _ = futex(self.word_addr(), FUTEX_WAIT | FUTEX_PRIVATE_FLAG, CONTENDED, 0, 0, 0);
        }
    }

    /// Attempts to acquire the lock without blocking.
    ///
    /// Returns `Some(guard)` if the lock was free, `None` if it was held.
    ///
    /// ```rust
    /// use reovim_arch::sync::Mutex;
    /// let m = Mutex::new(0u32);
    /// let g = m.try_lock();
    /// assert!(g.is_some());
    /// // Second try_lock while the first guard is alive returns None.
    /// assert!(m.try_lock().is_none());
    /// ```
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        if self
            .word
            .compare_exchange(FREE, LOCKED, Acquire, Relaxed)
            .is_ok()
        {
            Some(MutexGuard { mutex: self })
        } else {
            None
        }
    }

    /// Releases the lock (called from the guard's `Drop`, and by
    /// [`Condvar`](super::Condvar) which releases-then-reacquires manually).
    ///
    /// `Release` store per 2.3 §1.1; `FUTEX_WAKE` is issued **after** the
    /// store and **only** when the word was `CONTENDED`, so a sleeping
    /// waiter sees the holder's writes when it wins.
    pub(super) fn unlock(&self) {
        // `swap(FREE, Release)` publishes all writes made under the lock and
        // tells us whether a waiter needs waking.
        if self.word.swap(FREE, Release) == CONTENDED {
            // Wake exactly one waiter: it will swap CONTENDED back in and
            // either win or re-sleep, keeping the sticky-contended invariant.
            let _ = futex(self.word_addr(), FUTEX_WAKE | FUTEX_PRIVATE_FLAG, 1, 0, 0, 0);
        }
    }
}

/// An RAII guard that releases its [`Mutex`] on drop.
///
/// Derefs to the guarded `T`.
///
/// ```rust
/// use reovim_arch::sync::Mutex;
/// let m = Mutex::new(5u32);
/// let mut g = m.lock();
/// *g = 10; // DerefMut
/// assert_eq!(*g, 10); // Deref
/// ```
#[must_use = "the lock is released when the guard is dropped"]
pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<'a, T> MutexGuard<'a, T> {
    /// The mutex this guard holds. Used by [`Condvar`](super::Condvar) to
    /// release-then-reacquire the same lock across a wait.
    pub(super) const fn mutex(&self) -> &'a Mutex<T> {
        self.mutex
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: the guard's existence proves the lock is held, so this is
        // the unique live reference to the value.
        unsafe { &*self.mutex.value.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: the guard's existence proves the lock is held exclusively,
        // so a `&mut` to the value is sound.
        unsafe { &mut *self.mutex.value.get() }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.unlock();
    }
}

// L12 layout (#785 Phase 5): tests live in the sibling file `mutex_tests.rs`,
// declared in `sync/mod.rs` as
// `#[cfg(feature = "selftest")] mod mutex_tests;`.
// All items under test are public. The testhooks module in `sync/mod.rs` is
// now gated `#[cfg(feature = "selftest")]` so the selftest tests
// can call it too.
