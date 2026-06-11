//! Selftest-only allocator fault injection (#785 Phase 5 coverage).
//!
//! The allocator's `Err` arms — and every data-structure `?`-propagation arm
//! built on them — are otherwise only reachable when the kernel refuses a
//! normal-sized `mmap`, which cannot be forced in-process without `RLIMIT_AS`
//! games (a subprocess operation). This module is the deterministic in-process
//! alternative: a `cfg(feature = "selftest")` countdown that makes a chosen
//! allocation attempt return `Err(AllocError)` as if the kernel had refused it.
//!
//! It is compiled out entirely in any non-selftest build, so flight code is
//! byte-for-byte unaffected (the hook sites are `#[cfg(feature = "selftest")]`
//! blocks that vanish without the feature). It mirrors the `sync` testhooks
//! precedent: instrumentation that lives beside the code it probes, gated on
//! `selftest`.
//!
//! ## Two independent hooks
//!
//! - [`FAIL_AFTER`] gates the [`alloc`](super::alloc) entry: `fail_after(n)`
//!   makes the `n`-th subsequent allocation attempt fail, then self-disarms so
//!   no later allocation is affected. `n == 0` fails the very next attempt.
//!   This drives the size-front and large-path entry `Err` arms and, through
//!   them, every DS-level `?` arm (`Seq`, `Bytes`, `Map`, `Ring`, `Shared`,
//!   `ThreadShared`).
//! - [`FAIL_REFILL`] gates the size-class refill specifically
//!   ([`refill_class`](super::refill_class)): the entry hook returns before the
//!   refill path runs, so a dedicated flag is needed to reach the refill
//!   `mmap`-refusal arm. One-shot, self-disarming.
//!
//! Every test that arms a hook must leave it disarmed (the no_std runner shares
//! one process across all tests); [`reset`] restores both to the disabled
//! state and the helpers below self-disarm on fire as a second guard.

use core::sync::atomic::{AtomicIsize, Ordering};

/// Countdown for the [`alloc`](super::alloc) entry hook. `-1` (==
/// [`DISABLED`]) means no injection; `n >= 0` fails the allocation reached
/// after `n` more attempts (so `0` fails the next one).
static FAIL_AFTER: AtomicIsize = AtomicIsize::new(DISABLED);

/// Sentinel meaning "no injection armed".
const DISABLED: isize = -1;

/// One-shot flag for the refill hook: non-zero arms a single refill failure.
static FAIL_REFILL: AtomicIsize = AtomicIsize::new(0);

/// Arms the alloc-entry hook so the allocation reached after `n` further
/// attempts fails. `n == 0` fails the next attempt.
pub(crate) fn fail_after(n: isize) {
    FAIL_AFTER.store(n, Ordering::SeqCst);
}

/// Arms a single refill-path failure (the next size-class refill `mmap`).
pub(crate) fn fail_next_refill() {
    FAIL_REFILL.store(1, Ordering::SeqCst);
}

/// Disarms both hooks. Tests call this at the end of the body to leave the
/// shared process clean for the next test.
pub(crate) fn reset() {
    FAIL_AFTER.store(DISABLED, Ordering::SeqCst);
    FAIL_REFILL.store(0, Ordering::SeqCst);
}

/// Whether the alloc-entry hook should fire for this attempt. Decrements the
/// countdown; on reaching the fire point it self-disarms and returns `true`.
///
/// Plain load/store, no CAS: the seam contract is single-threaded arming —
/// each test arms the hook around its OWN allocations inside the sequential
/// selftest runner. A racing allocator would make the injection target
/// nondeterministic, defeating the seam's purpose, so the contract excludes
/// it and the code carries no unreachable retry arms.
pub(super) fn alloc_should_fail() -> bool {
    let cur = FAIL_AFTER.load(Ordering::SeqCst);
    if cur < 0 {
        return false;
    }
    if cur == 0 {
        // Fire once, then disarm so later allocations are unaffected.
        FAIL_AFTER.store(DISABLED, Ordering::SeqCst);
        return true;
    }
    // Not yet: count this attempt down by one.
    FAIL_AFTER.store(cur - 1, Ordering::SeqCst);
    false
}

/// Whether the refill hook should fire. One-shot: self-disarms on fire.
pub(super) fn refill_should_fail() -> bool {
    FAIL_REFILL
        .compare_exchange(1, 0, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}
