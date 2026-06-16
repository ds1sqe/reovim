//! DS integration tests, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: declared in `ds/mod.rs` as
//! `#[cfg(feature = "selftest")] mod tests;` (parent-declared sibling;
//! tests need only the public DS API and the public allocator accounting).
//!
//! Previously `extern crate std` was used for `std::vec::Vec` (ring
//! iteration collect) and for `std::sync::Arc` (Mutex sharing). Replaced with
//! `reovim_lib_ds::Seq` and `reovim_lib_ds::Shared` respectively.

use {
    reovim_kabi_platform::AllocError,
    reovim_lib_ds::{Map, Ring, Seq, Shared},
};

use crate::{alloc::live_bytes, arch_test, testrt};

arch_test!(ds_compose_and_return_to_baseline, {
    let base = live_bytes();
    {
        // A Seq that grows across several doublings.
        let mut seq: Seq<u64> = Seq::new();
        for i in 0..50u64 {
            seq.try_push(i).unwrap();
        }
        testrt::check_eq(seq.len(), 50);

        // A Map that grows across rehashes.
        let mut map: Map<u64, u64> = Map::new();
        for i in 0..50u64 {
            map.try_insert(i, i * 3).unwrap();
        }
        testrt::check_eq(map.get(&49), Some(&147));

        // A Ring driven past capacity to force evictions; collect into Seq.
        let mut ring: Ring<u64> = Ring::try_with_capacity(4).unwrap();
        for i in 0..10u64 {
            ring.push(i);
        }
        let mut live: Seq<u64> = Seq::new();
        for &v in ring.iter() {
            live.try_push(v).expect("collect ring");
        }
        testrt::check(live.as_slice() == &[6u64, 7, 8, 9], "ring last four survive");

        // A Shared cloned and dropped: the box frees only at count zero.
        let shared = Shared::try_new([0u64; 16]).unwrap();
        let clone = shared.clone();
        testrt::check_eq(shared.strong_count(), 2);
        drop(clone);
        testrt::check_eq(shared.strong_count(), 1);

        // Everything is still live here, so live_bytes exceeds baseline.
        testrt::check(live_bytes() > base, "live_bytes exceeds baseline while DS live");
    }
    // All four DS dropped: the allocator's accounting returns to baseline.
    testrt::check_eq(live_bytes(), base);
});

arch_test!(ds_every_fallible_constructor_surfaces_oom_without_panic, {
    // Seq: an absurd reservation overflows the layout size.
    let mut seq: Seq<u64> = Seq::new();
    testrt::check_eq(seq.try_reserve(usize::MAX / 2), Err(AllocError));
    testrt::check(seq.is_empty(), "seq is unchanged after OOM reserve");

    // Ring: an absurd capacity the OS cannot map.
    testrt::check_eq(Ring::<u64>::try_with_capacity(usize::MAX / 8).err(), Some(AllocError));

    // Shared's constructor cannot be driven to OOM here: `try_new(value)`
    // takes the value by move, so any `T` large enough to make the box
    // allocation fail would first have to exist as a stack temporary (which
    // overflows the stack, not the allocator). Shared's OOM branch is the
    // same `alloc(layout)?` early-return covered by the allocator's own OOM
    // tests; there is no DS-level shape to force it without an absurd
    // stack materialization.
    let _ = Shared::try_new(0u64).unwrap();
});
