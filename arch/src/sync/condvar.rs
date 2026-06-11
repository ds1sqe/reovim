//! `Condvar` — a futex sequence-number condition variable (2.3 §1.1).
//!
//! ## Missed-wake window
//!
//! The classic race is: a waiter decides to sleep, but a notifier signals in
//! the gap between releasing the mutex and entering the sleep. A naive
//! condvar would then sleep forever.
//!
//! This `Condvar` closes that window with a sequence counter:
//!
//! 1. `wait` reads the current sequence value **before** unlocking the mutex.
//! 2. it unlocks the mutex.
//! 3. it `FUTEX_WAIT`s on the sequence word with the *captured* value.
//!
//! If a `notify_*` bumps the sequence between steps 1 and 3, the captured
//! value no longer matches the word, so `FUTEX_WAIT` returns `EAGAIN`
//! immediately instead of blocking (2.3 §1.1: the futex word re-check closes
//! the missed-wake window). The mutex is then re-acquired under the same
//! `Acquire`/`Release` contract before `wait` returns.
//!
//! No spurious-wake guarantee is promised: `wait` may return without a
//! matching notify, so callers must re-check their predicate in a loop —
//! the standard condition-variable discipline.

use core::sync::atomic::{AtomicU32, Ordering::Relaxed};

use {
    super::mutex::MutexGuard,
    crate::sys::{FUTEX_PRIVATE_FLAG, FUTEX_WAIT, FUTEX_WAKE, futex},
};

/// `FUTEX_WAKE` count meaning "wake everyone".
///
/// The kernel reads the wake count as a signed `int`, so the broadcast
/// value is `i32::MAX` (the glibc convention). `u32::MAX` would arrive as
/// `-1` and the kernel's wake loop (`++woken >= nr_wake`) would stop after
/// a single waiter — a lost broadcast that strands every other sleeper.
const WAKE_ALL: u32 = 0x7FFF_FFFF; // i32::MAX, expressed unsigned

/// A condition variable paired with an arch [`Mutex`](super::Mutex).
///
/// No poisoning (see [`crate::sync`] module doc).
///
/// ```rust
/// use reovim_arch::sync::{Condvar, Mutex};
///
/// static M: Mutex<bool> = Mutex::new(false);
/// static CV: Condvar = Condvar::new();
///
/// // Set the flag from "another thread" (demonstrated single-threaded here
/// // by pre-setting the value, then calling notify, then waiting with a
/// // predicate loop that exits immediately).
/// *M.lock() = true;
/// CV.notify_one();
///
/// let mut g = M.lock();
/// while !*g {
///     g = CV.wait(g);
/// }
/// assert!(*g);
/// ```
pub struct Condvar {
    /// Monotonically-bumped sequence; the futex word every waiter sleeps on.
    seq: AtomicU32,
}

impl Condvar {
    /// Creates a new condition variable.
    ///
    /// ```rust
    /// use reovim_arch::sync::Condvar;
    /// let cv = Condvar::new();
    /// // A brand-new Condvar is inert until paired with a Mutex and waited on.
    /// cv.notify_one(); // harmless with no waiters
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            seq: AtomicU32::new(0),
        }
    }

    /// Address of the sequence word, for the raw `futex` syscall.
    fn seq_addr(&self) -> usize {
        core::ptr::from_ref(&self.seq).addr()
    }

    /// Atomically releases `guard`'s mutex and blocks until notified, then
    /// re-acquires the mutex and returns a fresh guard.
    ///
    /// Callers MUST re-check their predicate in a loop (no spurious-wake
    /// guarantee). The missed-wake window is closed by the sequence re-check
    /// (see the module doc).
    ///
    /// ```rust
    /// use reovim_arch::sync::{Condvar, Mutex};
    ///
    /// let m = Mutex::new(false);
    /// let cv = Condvar::new();
    ///
    /// // Signal before waiting: the predicate is already true, so the loop
    /// // exits on the first check without ever blocking.
    /// *m.lock() = true;
    /// cv.notify_one();
    ///
    /// let mut g = m.lock();
    /// while !*g { g = cv.wait(g); }
    /// assert!(*g);
    /// ```
    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        // Step 1: capture the sequence BEFORE releasing the mutex. Any notify
        // after this point bumps the sequence and is seen by step 3.
        let captured = self.seq.load(Relaxed);
        let mutex = guard.mutex();

        // Step 2: release the mutex. `forget` suppresses the guard's Drop so
        // the unlock happens exactly once (here), not twice.
        core::mem::forget(guard);
        mutex.unlock();

        // Step 3: sleep while the sequence still equals the captured value. A
        // notify between steps 1 and 2-3 makes the value mismatch, so
        // FUTEX_WAIT returns EAGAIN without blocking — the window is closed.
        let _ = futex(self.seq_addr(), FUTEX_WAIT | FUTEX_PRIVATE_FLAG, captured, 0, 0, 0);

        // Re-acquire under the same Acquire/Release contract before returning.
        mutex.lock()
    }

    /// Bumps the sequence and wakes one waiter.
    ///
    /// ```rust
    /// use reovim_arch::sync::Condvar;
    /// let cv = Condvar::new();
    /// cv.notify_one(); // no-op with no waiters; sequence is still bumped
    /// ```
    pub fn notify_one(&self) {
        // The bump must precede the wake so a waiter that has not yet slept
        // observes the changed sequence (EAGAIN) rather than missing the wake.
        self.seq.fetch_add(1, Relaxed);
        let _ = futex(self.seq_addr(), FUTEX_WAKE | FUTEX_PRIVATE_FLAG, 1, 0, 0, 0);
    }

    /// Bumps the sequence and wakes all waiters.
    ///
    /// ```rust
    /// use reovim_arch::sync::Condvar;
    /// let cv = Condvar::new();
    /// cv.notify_all(); // no-op with no waiters
    /// ```
    pub fn notify_all(&self) {
        self.seq.fetch_add(1, Relaxed);
        let _ = futex(self.seq_addr(), FUTEX_WAKE | FUTEX_PRIVATE_FLAG, WAKE_ALL, 0, 0, 0);
    }
}

impl Default for Condvar {
    fn default() -> Self {
        Self::new()
    }
}

// L12 layout (#785 Phase 5): tests live in the sibling file `condvar_tests.rs`,
// declared in `sync/mod.rs` as
// `#[cfg(feature = "selftest")] mod condvar_tests;`.
