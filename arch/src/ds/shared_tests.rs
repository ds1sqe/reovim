//! Tests for `ds/shared.rs`, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: declared inside `shared.rs` as
//! `#[cfg(feature = "selftest")] #[path = "shared_tests.rs"] mod tests;`.
//! `super::` reaches the private `MAX_REFCOUNT` and `refcount_overflowed`
//! symbols.
//!
//! The multithreaded clone/drop test previously used `std::thread::spawn` and
//! `std::vec::Vec`. Replaced with `crate::thread::spawn` (the arch thread
//! primitive) and `crate::ds::Seq`.

use {
    crate::{arch_test, testrt},
    core::cell::Cell,
};

use super::{MAX_REFCOUNT, Shared, refcount_overflowed};

arch_test!(shared_new_has_count_one_and_derefs, {
    let s = Shared::try_new(42u32).unwrap();
    testrt::check_eq(s.strong_count(), 1);
    testrt::check_eq(*s, 42);
});

arch_test!(shared_refcount_overflow_guard_decision, {
    // Both sides of the saturation guard, exercised directly (a real clone
    // cannot reach MAX_REFCOUNT).
    testrt::check(!refcount_overflowed(0), "0 does not overflow");
    testrt::check(!refcount_overflowed(MAX_REFCOUNT - 1), "MAX-1 does not overflow");
    testrt::check(refcount_overflowed(MAX_REFCOUNT), "MAX overflows");
    testrt::check(refcount_overflowed(usize::MAX), "usize::MAX overflows");
});

arch_test!(shared_clone_increments_drop_decrements, {
    let s = Shared::try_new(7u32).unwrap();
    let c = s.clone();
    testrt::check_eq(s.strong_count(), 2);
    testrt::check_eq(c.strong_count(), 2);
    testrt::check_eq(*c, 7);
    drop(c);
    testrt::check_eq(s.strong_count(), 1);
});

/// A drop-counting value to prove the box drops exactly once at count zero.
///
/// The counter is a `Cell<usize>` borrowed for the test's scope. This works
/// because the arch runner is single-threaded sequential — the thread that
/// runs the test also runs all destructors before the borrow expires.
struct DropCounter<'a> {
    counter: &'a Cell<usize>,
}

impl Drop for DropCounter<'_> {
    fn drop(&mut self) {
        self.counter.set(self.counter.get() + 1);
    }
}

arch_test!(shared_value_dropped_once_when_last_ref_drops, {
    let counter = Cell::new(0);
    {
        let s = Shared::try_new(DropCounter { counter: &counter }).unwrap();
        let c = s.clone();
        drop(c);
        // Not yet: one reference remains.
        testrt::check_eq(counter.get(), 0);
    }
    // Now the last reference dropped: exactly one destructor run.
    testrt::check_eq(counter.get(), 1);
});

arch_test!(shared_try_new_alloc_oom_propagates, {
    // shared.rs L77: `alloc(layout)?` in `try_new`. The allocator fault hook
    // forces that alloc to fail, driving the `?` arm.
    crate::alloc::fault::fail_after(0);
    let r = Shared::try_new(42u32);
    crate::alloc::fault::reset();
    testrt::check(r.is_err(), "try_new surfaces AllocError under fault");
    // Succeeds once disarmed.
    let s = Shared::try_new(7u32).expect("alloc after reset");
    testrt::check_eq(*s, 7u32);
});

// Spawn-dependent: needs a thread floor, which freestanding targets do not
// realize. Stays in every hosted suite.
#[cfg(target_os = "linux")]
arch_test!(shared_threaded_clones_and_drops_balance, {
    // Atomic refcount stress: 8 arch threads each clone-and-drop 10 000 times;
    // count must return to 1 with no double-free.
    //
    // `JoinHandle<F,T>` is generic, so all 8 handles share the same concrete
    // closure type (captured `Shared<u64>`, returns `()`). A fixed-size array
    // is used to store them without requiring a Vec or Seq of a type-erased
    // handle trait — the array size is known at compile time.
    use crate::thread::spawn;

    let s = Shared::try_new(0u64).unwrap();

    macro_rules! make_handle {
        ($s:expr) => {{
            let local = $s.clone();
            spawn(move || {
                for _ in 0..10_000u32 {
                    let inner = local.clone();
                    testrt::check(*inner == 0, "shared value is zero");
                    drop(inner);
                }
            })
            .expect("thread spawn")
        }};
    }

    let h0 = make_handle!(s);
    let h1 = make_handle!(s);
    let h2 = make_handle!(s);
    let h3 = make_handle!(s);
    let h4 = make_handle!(s);
    let h5 = make_handle!(s);
    let h6 = make_handle!(s);
    let h7 = make_handle!(s);
    let () = h0.join();
    let () = h1.join();
    let () = h2.join();
    let () = h3.join();
    let () = h4.join();
    let () = h5.join();
    let () = h6.join();
    let () = h7.join();

    // The 8 per-thread clones dropped at closure scope end; only the
    // original remains.
    testrt::check_eq(s.strong_count(), 1);
});
