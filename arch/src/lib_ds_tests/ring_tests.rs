//! Tests for `ds/ring.rs`, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: declared inside `ring.rs` as
//! `#[cfg(feature = "selftest")] #[path = "ring_tests.rs"] mod tests;`.
//! All items under test are public; `super::` is not needed.
//!
//! `std::vec::Vec` from the original tests is replaced with `reovim_lib_ds::Seq`
//! (the arch Vec analog under DAG6/no_std).

use {
    crate::{arch_test, testrt},
    core::cell::Cell,
    reovim_kabi_platform::AllocError,
    reovim_lib_ds::{Ring, Seq},
};

// Helper: collect ring items into a Seq for comparison.
fn collect<T: Copy>(r: &Ring<T>) -> Seq<T> {
    let mut s = Seq::new();
    for v in r.iter() {
        s.try_push(*v).expect("collect push");
    }
    s
}

arch_test!(ring_fill_up_to_capacity_without_eviction, {
    let mut r: Ring<u32> = Ring::try_with_capacity(3).unwrap();
    testrt::check(r.is_empty(), "new ring is empty");
    testrt::check_eq(r.capacity(), 3);
    testrt::check_eq(r.push(1), None);
    testrt::check_eq(r.push(2), None);
    testrt::check_eq(r.push(3), None);
    testrt::check(r.is_full(), "ring full after 3 pushes");
    testrt::check_eq(r.len(), 3);
    let got = collect(&r);
    testrt::check(got.as_slice() == &[1u32, 2, 3], "fill order preserved");
});

arch_test!(ring_push_when_full_evicts_oldest, {
    let mut r: Ring<u32> = Ring::try_with_capacity(3).unwrap();
    r.push(1);
    r.push(2);
    r.push(3);
    testrt::check_eq(r.push(4), Some(1));
    testrt::check_eq(r.push(5), Some(2));
    let got = collect(&r);
    testrt::check(got.as_slice() == &[3u32, 4, 5], "eviction order correct");
    testrt::check_eq(r.len(), 3);
});

arch_test!(ring_eviction_wraps_head_past_end, {
    // Push more than 2x capacity to force head to wrap at least once.
    let mut r: Ring<u32> = Ring::try_with_capacity(2).unwrap();
    for i in 0..6u32 {
        r.push(i);
    }
    // Only the last two survive, oldest-first.
    let got = collect(&r);
    testrt::check(got.as_slice() == &[4u32, 5], "only last two survive");
});

arch_test!(ring_pop_oldest_drains_in_order, {
    let mut r: Ring<u32> = Ring::try_with_capacity(4).unwrap();
    r.push(10);
    r.push(20);
    r.push(30);
    testrt::check_eq(r.pop_oldest(), Some(10));
    testrt::check_eq(r.pop_oldest(), Some(20));
    testrt::check_eq(r.len(), 1);
    testrt::check_eq(r.pop_oldest(), Some(30));
    testrt::check_eq(r.pop_oldest(), None);
    testrt::check(r.is_empty(), "ring empty after draining");
});

arch_test!(ring_pop_then_push_exercises_tail_wrap, {
    let mut r: Ring<u32> = Ring::try_with_capacity(3).unwrap();
    r.push(1);
    r.push(2);
    r.push(3);
    testrt::check_eq(r.pop_oldest(), Some(1));
    testrt::check_eq(r.pop_oldest(), Some(2));
    testrt::check_eq(r.push(4), None);
    let got = collect(&r);
    testrt::check(got.as_slice() == &[3u32, 4], "wrap-around push order");
});

arch_test!(ring_iter_on_empty_yields_nothing, {
    let r: Ring<u32> = Ring::try_with_capacity(2).unwrap();
    testrt::check_eq(r.iter().count(), 0);
    let mut count = 0;
    for _ in &r {
        count += 1;
    }
    testrt::check_eq(count, 0);
});

arch_test!(ring_into_iter_for_ref_yields_in_order, {
    let mut r: Ring<u32> = Ring::try_with_capacity(3).unwrap();
    r.push(7);
    r.push(8);
    let got = collect(&r);
    testrt::check(got.as_slice() == &[7u32, 8], "ref iter order");
});

arch_test!(ring_zero_capacity_is_error, {
    testrt::check_eq(Ring::<u32>::try_with_capacity(0).err(), Some(AllocError));
});

arch_test!(ring_absurd_capacity_surfaces_error_not_panic, {
    testrt::check_eq(Ring::<u64>::try_with_capacity(usize::MAX / 8).err(), Some(AllocError));
});

/// A drop-counting element to prove `Drop` runs each live destructor once.
struct DropCounter<'a> {
    counter: &'a Cell<usize>,
}

impl Drop for DropCounter<'_> {
    fn drop(&mut self) {
        self.counter.set(self.counter.get() + 1);
    }
}

arch_test!(ring_drop_runs_only_for_live_elements, {
    let counter = Cell::new(0);
    {
        let mut r: Ring<DropCounter> = Ring::try_with_capacity(2).unwrap();
        r.push(DropCounter { counter: &counter });
        r.push(DropCounter { counter: &counter });
        // Push 3rd into a cap-2 ring: the evicted first is returned to caller.
        let evicted = r.push(DropCounter { counter: &counter });
        testrt::check(evicted.is_some(), "evicted element returned");
        drop(evicted); // evicted element's destructor runs now
        testrt::check_eq(counter.get(), 1);
        // Drop of the ring runs the 2 live destructors.
    }
    testrt::check_eq(counter.get(), 3);
});

arch_test!(ring_try_with_capacity_alloc_oom_propagates, {
    // ring.rs L58: `alloc(layout)?` inside `try_with_capacity`. The
    // layout-overflow arm (L57) is exercised by
    // `ring_absurd_capacity_surfaces_error_not_panic`; this drives the alloc
    // syscall-`Err` arm via the allocator fault hook (a normal-size request the
    // hook forces to fail).
    crate::alloc::fault::fail_after(0);
    let r = Ring::<u64>::try_with_capacity(4);
    crate::alloc::fault::reset();
    testrt::check_eq(r.err(), Some(AllocError));
    // A normal request succeeds once the hook is disarmed.
    let ring = Ring::<u64>::try_with_capacity(4).expect("ring builds after reset");
    testrt::check_eq(ring.capacity(), 4);
});

arch_test!(ring_pop_oldest_wraps_head_at_capacity_boundary, {
    // Drive the `self.head = 0` arm (ring.rs L134) of `pop_oldest`:
    // `if head + 1 >= self.cap { 0 } else { head + 1 }`.
    // With cap=2: fill (head=0), evict via push (head becomes 1), then pop
    // (head+1 = 2 >= cap=2, so head wraps to 0).
    let mut r: Ring<u32> = Ring::try_with_capacity(2).unwrap();
    // Fill the ring: head=0, len=2.
    testrt::check_eq(r.push(10), None);
    testrt::check_eq(r.push(20), None);
    // Third push evicts slot 0 (value 10) and advances head to 1.
    let evicted = r.push(30);
    testrt::check_eq(evicted, Some(10));
    // head is now 1; len=2 (slots 1 and 0 in order: 20, 30).
    testrt::check_eq(r.len(), 2);
    // pop_oldest: reads slot 1 (value 20), then head+1 = 2 >= cap=2 → head = 0.
    testrt::check_eq(r.pop_oldest(), Some(20));
    testrt::check_eq(r.len(), 1);
    // head is now 0; only 30 remains.
    testrt::check_eq(r.pop_oldest(), Some(30));
    testrt::check_eq(r.pop_oldest(), None);
    testrt::check(r.is_empty(), "ring empty after all pops");
});
