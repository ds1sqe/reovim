//! Tests for `ds/seq.rs`, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: declared inside `seq.rs` as
//! `#[cfg(feature = "selftest")] #[path = "seq_tests.rs"] mod tests;`
//! so `super::` reaches the private `FIRST_CAPACITY` constant.

use {
    crate::{arch_test, testrt},
    core::cell::Cell,
    reovim_lib_ds::AllocError,
};

use reovim_lib_ds::{Seq, seq::FIRST_CAPACITY};

arch_test!(seq_is_send_and_sync_for_send_sync_t, {
    // Compile-time proof that `Seq<T>` crosses the arch thread boundary.
    const fn assert_send<T: Send>() {}
    const fn assert_sync<T: Sync>() {}
    assert_send::<Seq<u64>>();
    assert_sync::<Seq<u64>>();
});

arch_test!(seq_new_is_empty_and_unallocated, {
    let s: Seq<u32> = Seq::new();
    testrt::check(s.is_empty(), "new seq is empty");
    testrt::check_eq(s.len(), 0);
    testrt::check_eq(s.capacity(), 0);
});

arch_test!(seq_push_grows_and_doubles, {
    let mut s: Seq<u32> = Seq::new();
    for i in 0..u32::try_from(FIRST_CAPACITY).unwrap() {
        s.try_push(i).unwrap();
    }
    testrt::check_eq(s.capacity(), FIRST_CAPACITY);
    // One more triggers a double.
    s.try_push(99).unwrap();
    testrt::check_eq(s.capacity(), FIRST_CAPACITY * 2);
    testrt::check_eq(s.len(), FIRST_CAPACITY + 1);
});

arch_test!(seq_push_pop_lifo, {
    let mut s: Seq<i32> = Seq::new();
    s.try_push(1).unwrap();
    s.try_push(2).unwrap();
    s.try_push(3).unwrap();
    testrt::check_eq(s.pop(), Some(3));
    testrt::check_eq(s.pop(), Some(2));
    testrt::check_eq(s.pop(), Some(1));
    testrt::check_eq(s.pop(), None);
});

arch_test!(seq_get_and_index, {
    let mut s: Seq<i32> = Seq::new();
    s.try_push(10).unwrap();
    s.try_push(20).unwrap();
    testrt::check_eq(s.get(0), Some(&10));
    testrt::check_eq(s.get(1), Some(&20));
    testrt::check_eq(s.get(2), None);
    testrt::check_eq(s[0], 10);
    s[1] = 21;
    testrt::check_eq(s[1], 21);
    *s.get_mut(0).unwrap() = 11;
    testrt::check_eq(s[0], 11);
    testrt::check_eq(s.get_mut(5), None);
});

// NOTE: the original `#[should_panic]` tests (`index_out_of_bounds_panics`,
// `index_mut_out_of_bounds_panics`) cannot run in the fail-fast no_std runner
// (panic = abort, no unwind). The OOB behaviour is structurally covered by
// `seq_get_and_index` above which asserts `get(out_of_bounds) == None`. The
// panicking path through `Index::index` is documented behaviour rather than a
// tested failure mode; it is exercised by `seq_get_and_index`'s `s[0]` happy
// path proving the impl is wired correctly.

arch_test!(seq_iter_and_slice, {
    let mut s: Seq<i32> = Seq::new();
    for i in 0..5 {
        s.try_push(i).unwrap();
    }
    let sum: i32 = s.iter().sum();
    testrt::check_eq(sum, 1 + 2 + 3 + 4);
    testrt::check(s.as_slice() == &[0, 1, 2, 3, 4], "as_slice matches pushed values");
    for v in s.as_mut_slice() {
        *v += 1;
    }
    testrt::check(s.as_slice() == &[1, 2, 3, 4, 5], "as_mut_slice mutation visible");
});

arch_test!(seq_empty_slice_from_unallocated, {
    let s: Seq<u8> = Seq::new();
    testrt::check(s.as_slice().is_empty(), "unallocated slice is empty");
    testrt::check_eq(s.iter().count(), 0);
});

arch_test!(seq_default_is_new, {
    let s: Seq<u8> = Seq::default();
    testrt::check(s.is_empty(), "default seq is empty");
});

arch_test!(seq_reserve_overflow_surfaces_error, {
    let mut s: Seq<u64> = Seq::new();
    testrt::check_eq(s.try_reserve(usize::MAX / 2), Err(AllocError));
    testrt::check(s.is_empty(), "seq unchanged after failed reserve");
    // len + additional overflow branch.
    s.try_push(1).unwrap();
    testrt::check_eq(s.try_reserve(usize::MAX), Err(AllocError));
});

arch_test!(seq_try_reserve_checked_mul_overflow_on_nonempty_seq, {
    // Drive the `checked_mul` overflow arm of `try_reserve` on a *non-empty*
    // Seq. The input must make `needed` EXCEED 2^63: the doubling ladder can
    // reach 2^63 without overflow, so any `needed <= 2^63` exits the loop and
    // lands in the layout arm instead. `len=1, additional=usize::MAX-1` gives
    // `needed = usize::MAX` (checked_add fine); the ladder doubles to 2^63,
    // still below `needed`, and the next `*2` overflows -> Err.
    let mut s: Seq<u64> = Seq::new();
    s.try_push(99).unwrap(); // non-empty: cap = FIRST_CAPACITY
    let r = s.try_reserve(usize::MAX - 1);
    testrt::check(r.is_err(), "try_reserve on non-empty seq overflows checked_mul");
    // The single element survives.
    testrt::check_eq(s.len(), 1);
    testrt::check_eq(s[0], 99u64);
});

arch_test!(seq_reserve_grows_then_pushes_without_realloc, {
    let mut s: Seq<u32> = Seq::new();
    s.try_reserve(10).unwrap();
    let cap = s.capacity();
    testrt::check(cap >= 10, "reserved capacity >= 10");
    for i in 0..10 {
        s.try_push(i).unwrap();
    }
    // No further growth was needed.
    testrt::check_eq(s.capacity(), cap);
    // A reserve already covered by capacity is a no-op.
    s.try_reserve(1).unwrap();
    testrt::check_eq(s.capacity(), cap);
    // A reserve on a non-empty Seq takes the realloc path.
    s.try_reserve(cap * 4).unwrap();
    testrt::check(s.capacity() >= cap * 4, "capacity expanded");
    testrt::check_eq(s.len(), 10);
    testrt::check_eq(s[9], 9u32);
});

/// A drop-counting element to prove `Drop` runs each destructor.
struct DropCounter<'a> {
    counter: &'a Cell<usize>,
}

impl Drop for DropCounter<'_> {
    fn drop(&mut self) {
        self.counter.set(self.counter.get() + 1);
    }
}

arch_test!(seq_drop_runs_for_each_element, {
    let counter = Cell::new(0);
    {
        let mut s: Seq<DropCounter> = Seq::new();
        for _ in 0..3 {
            s.try_push(DropCounter { counter: &counter }).unwrap();
        }
        testrt::check_eq(counter.get(), 0);
    }
    testrt::check_eq(counter.get(), 3);
});

arch_test!(seq_pop_drops_only_remaining_on_drop, {
    let counter = Cell::new(0);
    {
        let mut s: Seq<DropCounter> = Seq::new();
        for _ in 0..3 {
            s.try_push(DropCounter { counter: &counter }).unwrap();
        }
        // Pop one: its destructor runs now.
        drop(s.pop());
        testrt::check_eq(counter.get(), 1);
        // Drop runs for the remaining 2.
    }
    testrt::check_eq(counter.get(), 3);
});

arch_test!(seq_grow_if_full_first_alloc_oom_propagates, {
    // `grow_if_full`'s first-grow arm calls `alloc(new_layout)?` (cap == 0).
    // The allocator fault hook forces that alloc to fail, so `try_push` on an
    // empty `Seq` propagates the `Err` — the `?` arm at the `alloc` call.
    let mut s: Seq<u64> = Seq::new();
    crate::alloc::fault::fail_after(0);
    let r = s.try_push(1);
    crate::alloc::fault::reset();
    testrt::check_eq(r, Err(AllocError));
    testrt::check(s.is_empty(), "seq unchanged after the forced alloc failure");
    // The Seq is still usable once the hook is disarmed.
    s.try_push(7).unwrap();
    testrt::check_eq(s[0], 7u64);
});

arch_test!(seq_grow_if_full_realloc_oom_propagates, {
    // Once the `Seq` holds an allocation, `grow_if_full` takes the
    // `realloc(...)?` arm. Fill to capacity, then force the next grow's inner
    // alloc (realloc → alloc) to fail, driving the realloc `?` arm.
    let mut s: Seq<u64> = Seq::new();
    for i in 0..u64::try_from(FIRST_CAPACITY).unwrap() {
        s.try_push(i).unwrap();
    }
    testrt::check_eq(s.capacity(), FIRST_CAPACITY);
    crate::alloc::fault::fail_after(0);
    let r = s.try_push(99); // at capacity → realloc grow
    crate::alloc::fault::reset();
    testrt::check_eq(r, Err(AllocError));
    // The original elements survive the failed grow (realloc leaves the source
    // intact on error).
    testrt::check_eq(s.len(), FIRST_CAPACITY);
    testrt::check_eq(s.capacity(), FIRST_CAPACITY);
    testrt::check_eq(s[0], 0u64);
});

arch_test!(seq_try_reserve_first_alloc_oom_propagates, {
    // `try_reserve` on an empty `Seq` takes its `alloc(new_layout)?` arm.
    let mut s: Seq<u64> = Seq::new();
    crate::alloc::fault::fail_after(0);
    let r = s.try_reserve(4);
    crate::alloc::fault::reset();
    testrt::check_eq(r, Err(AllocError));
    testrt::check_eq(s.capacity(), 0);
});

arch_test!(seq_try_reserve_realloc_oom_propagates, {
    // `try_reserve` on a non-empty `Seq` takes its `realloc(...)?` arm.
    let mut s: Seq<u64> = Seq::new();
    s.try_push(1).unwrap();
    crate::alloc::fault::fail_after(0);
    let r = s.try_reserve(64);
    crate::alloc::fault::reset();
    testrt::check_eq(r, Err(AllocError));
    // Unchanged: the one element survives.
    testrt::check_eq(s.len(), 1);
    testrt::check_eq(s[0], 1u64);
});

arch_test!(seq_try_reserve_layout_overflow_surfaces_error, {
    // The layout-Err arm distinct from checked_mul overflow: a slot count
    // that survives the doubling ladder's checked_mul but whose BYTE size
    // exceeds Layout::array's isize::MAX bound. For u64 (8 bytes), 2^60
    // slots (usize::MAX/16 + 1) is 2^63 bytes — checked_mul never exceeds
    // usize, but the layout overflows isize: Err, seq unchanged.
    let mut s: Seq<u64> = Seq::new();
    s.try_push(7).unwrap();
    let r = s.try_reserve(usize::MAX / 16 + 1);
    testrt::check(r.is_err(), "layout byte-size overflow surfaces AllocError");
    testrt::check_eq(s.len(), 1);
    testrt::check_eq(s[0], 7u64);
});
