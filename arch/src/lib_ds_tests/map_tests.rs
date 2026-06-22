//! Tests for `ds/map.rs`, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: declared inside `map.rs` as
//! `#[cfg(feature = "selftest")] #[path = "map_tests.rs"] mod tests;`.
//! `super::` reaches the private `cyclic_in_range` function and the
//! private `FIRST_CAPACITY` constant.

use {
    crate::{arch_test, testrt},
    core::{
        cell::Cell,
        hash::{Hash, Hasher},
    },
    reovim_lib_ds::AllocError,
};

use reovim_lib_ds::{
    FxHasher, Map,
    map::{FIRST_CAPACITY, cyclic_in_range},
};

arch_test!(map_new_is_empty_and_unallocated, {
    let m: Map<u32, u32> = Map::new();
    testrt::check(m.is_empty(), "new map is empty");
    testrt::check_eq(m.len(), 0);
    testrt::check(m.get(&1).is_none(), "get on unallocated returns None");
    // get_mut and remove on an unallocated map take the cap == 0 path.
    let mut m2: Map<u32, u32> = Map::new();
    testrt::check(m2.get_mut(&1).is_none(), "get_mut on unallocated returns None");
    testrt::check_eq(m2.remove(&1), None);
    // iter over an unallocated map yields nothing.
    testrt::check_eq(m.iter().count(), 0);
});

arch_test!(map_insert_get_and_displace, {
    let mut m: Map<u32, u32> = Map::new();
    testrt::check_eq(m.try_insert(1, 10).unwrap(), None);
    testrt::check_eq(m.try_insert(2, 20).unwrap(), None);
    testrt::check_eq(m.len(), 2);
    testrt::check_eq(m.get(&1), Some(&10));
    testrt::check_eq(m.get(&2), Some(&20));
    testrt::check(m.get(&3).is_none(), "absent key returns None");
    // Re-insert displaces, returns the old value, len unchanged.
    testrt::check_eq(m.try_insert(1, 11).unwrap(), Some(10));
    testrt::check_eq(m.get(&1), Some(&11));
    testrt::check_eq(m.len(), 2);
});

arch_test!(map_get_mut_mutates, {
    let mut m: Map<u32, u32> = Map::new();
    m.try_insert(5, 50).unwrap();
    *m.get_mut(&5).unwrap() += 1;
    testrt::check_eq(m.get(&5), Some(&51));
    testrt::check(m.get_mut(&99).is_none(), "get_mut on missing key returns None");
});

arch_test!(map_grow_across_rehash_preserves_all, {
    let mut m: Map<u32, u32> = Map::new();
    // Insert enough to force multiple grows past FIRST_CAPACITY.
    for i in 0..100u32 {
        testrt::check_eq(m.try_insert(i, i * 2).unwrap(), None);
    }
    testrt::check_eq(m.len(), 100);
    for i in 0..100u32 {
        testrt::check_eq(m.get(&i), Some(&(i * 2)));
    }
});

arch_test!(map_remove_returns_value_and_missing_none, {
    let mut m: Map<u32, u32> = Map::new();
    m.try_insert(1, 10).unwrap();
    m.try_insert(2, 20).unwrap();
    testrt::check_eq(m.remove(&1), Some(10));
    testrt::check_eq(m.len(), 1);
    testrt::check(m.get(&1).is_none(), "removed key is gone");
    // Removing a missing key is None and does not change len.
    testrt::check_eq(m.remove(&1), None);
    testrt::check_eq(m.remove(&99), None);
    testrt::check_eq(m.len(), 1);
    // The surviving key is still reachable.
    testrt::check_eq(m.get(&2), Some(&20));
});

/// A key whose hash is fully controlled, to force same-bucket collisions.
#[derive(PartialEq, Eq, Clone, Copy)]
struct CollidingKey(u64, u64); // (hash_input, identity)

impl Hash for CollidingKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Only the first field feeds the hash, so two keys with the same
        // first field collide while remaining distinct by identity.
        state.write_u64(self.0);
    }
}

arch_test!(map_collision_chain_insert_get_remove, {
    let mut m: Map<CollidingKey, u32> = Map::new();
    // All four hash identically -> they land on one probe chain.
    let key_a = CollidingKey(7, 1);
    let key_b = CollidingKey(7, 2);
    let key_c = CollidingKey(7, 3);
    let key_d = CollidingKey(7, 4);
    m.try_insert(key_a, 1).unwrap();
    m.try_insert(key_b, 2).unwrap();
    m.try_insert(key_c, 3).unwrap();
    m.try_insert(key_d, 4).unwrap();
    testrt::check_eq(m.get(&key_a), Some(&1));
    testrt::check_eq(m.get(&key_b), Some(&2));
    testrt::check_eq(m.get(&key_c), Some(&3));
    testrt::check_eq(m.get(&key_d), Some(&4));
    // Remove a middle entry: backward-shift must keep the tail reachable.
    testrt::check_eq(m.remove(&key_b), Some(2));
    testrt::check_eq(m.get(&key_a), Some(&1));
    testrt::check(m.get(&key_b).is_none(), "removed middle key is gone");
    testrt::check_eq(m.get(&key_c), Some(&3));
    testrt::check_eq(m.get(&key_d), Some(&4));
    // Remove the head of the chain: tail still reachable.
    testrt::check_eq(m.remove(&key_a), Some(1));
    testrt::check_eq(m.get(&key_c), Some(&3));
    testrt::check_eq(m.get(&key_d), Some(&4));
});

arch_test!(map_remove_wrap_branch_shifts_across_array_end, {
    // Deterministically force a probe chain whose head is the LAST bucket,
    // so backward-shift after removing the head wraps past the array end —
    // exercising the wrapping arm of `cyclic_in_range` (hole >= scan).
    //
    // With FIRST_CAPACITY = 8 (mask 7) and three inserts, cap stays 8
    // (`(3+1)*8 = 32 <= 8*7 = 56`). `hash_of(CollidingKey(x))` with a zero
    // initial hash folds one u64 word: `hash = (0.rotate_left(5) ^ x) * FX_SEED`.
    // FX_SEED mod 8 = 5; solving `x*5 ≡ 7 (mod 8)` gives `x ≡ 3 (mod 8)`, so a
    // hash input of 3 maps to bucket 7. Three collisions then occupy slots 7, 0, 1.
    testrt::check_eq(FIRST_CAPACITY, 8); // guard: test depends on initial cap
    let mut m: Map<CollidingKey, u32> = Map::new();
    let keys: [CollidingKey; 3] = [CollidingKey(3, 1), CollidingKey(3, 2), CollidingKey(3, 3)];
    for (i, k) in keys.iter().enumerate() {
        m.try_insert(*k, u32::try_from(i).unwrap()).unwrap();
    }
    // Removing the head at slot 7 leaves a hole there; the shift wraps to
    // slots 0,1 and must keep the others reachable.
    testrt::check_eq(m.remove(&keys[0]), Some(0));
    testrt::check_eq(m.get(&keys[1]), Some(&1));
    testrt::check_eq(m.get(&keys[2]), Some(&2));
});

arch_test!(map_iter_counts_all_entries, {
    let mut m: Map<u32, u32> = Map::new();
    for i in 0..10u32 {
        m.try_insert(i, i).unwrap();
    }
    let mut seen = 0u32;
    let mut sum = 0u32;
    for (k, v) in &m {
        testrt::check_eq(k, v);
        seen += 1;
        sum += *v;
    }
    testrt::check_eq(seen, 10u32);
    testrt::check_eq(sum, (0..10u32).sum());
    // The explicit iter() path too.
    testrt::check_eq(m.iter().count(), 10);
});

arch_test!(map_default_is_new, {
    let m: Map<u32, u32> = Map::default();
    testrt::check(m.is_empty(), "default map is empty");
});

arch_test!(map_hasher_folds_words_and_tail, {
    // A multi-word input (>8 bytes) exercises both the chunk loop and the
    // remainder tail of `FxHasher::write`.
    let mut h1 = FxHasher::default();
    h1.write(b"abcdefghij"); // 10 bytes: one 8-byte word + 2-byte tail
    let mut h2 = FxHasher::default();
    h2.write(b"abcdefghij");
    testrt::check_eq(h1.finish(), h2.finish());
    // A different tail yields a different hash.
    let mut h3 = FxHasher::default();
    h3.write(b"abcdefghXY");
    testrt::check(h1.finish() != h3.finish(), "different input yields different hash");
    // Exactly 8 bytes: no remainder branch.
    let mut h4 = FxHasher::default();
    h4.write(b"abcdefgh");
    testrt::check(h4.finish() != 0, "hash of 8 bytes is non-zero");
});

arch_test!(map_cyclic_in_range_covers_both_arms_and_conditions, {
    // Non-wrapping arm (hole < scan): `x > hole && x <= scan`.
    // Both conditions vary independently for MC/DC.
    testrt::check(cyclic_in_range(2, 5, 4), "in: 4>2 && 4<=5");
    testrt::check(!cyclic_in_range(2, 5, 2), "first false: 2>2 is false");
    testrt::check(!cyclic_in_range(2, 5, 6), "second false: 6<=5 is false");
    testrt::check(cyclic_in_range(2, 5, 5), "boundary: 5<=5 true");
    // Wrapping arm (hole >= scan): `x > hole || x <= scan`.
    testrt::check(cyclic_in_range(6, 2, 7), "first true: 7>6");
    testrt::check(cyclic_in_range(6, 2, 1), "second true: 1<=2");
    testrt::check(!cyclic_in_range(6, 2, 4), "both false: 4>6 no, 4<=2 no");
    // hole == scan is the wrapping arm too (full circle).
    testrt::check(cyclic_in_range(3, 3, 3), "full circle: hole==scan");
});

/// A drop-counting value to prove `Drop` runs each destructor.
struct DropCounter<'a> {
    counter: &'a Cell<usize>,
}

impl Drop for DropCounter<'_> {
    fn drop(&mut self) {
        self.counter.set(self.counter.get() + 1);
    }
}

arch_test!(map_drop_runs_for_each_full_bucket, {
    let counter = Cell::new(0);
    {
        let mut m: Map<u32, DropCounter> = Map::new();
        for i in 0..5u32 {
            m.try_insert(i, DropCounter { counter: &counter }).unwrap();
        }
        testrt::check_eq(counter.get(), 0);
    }
    testrt::check_eq(counter.get(), 5);
});

arch_test!(map_displaced_value_is_dropped_by_caller, {
    let counter = Cell::new(0);
    let mut m: Map<u32, DropCounter> = Map::new();
    m.try_insert(1, DropCounter { counter: &counter }).unwrap();
    // Re-insert displaces the first value; dropping the returned Option
    // runs its destructor.
    let old = m.try_insert(1, DropCounter { counter: &counter }).unwrap();
    testrt::check_eq(counter.get(), 0);
    drop(old);
    testrt::check_eq(counter.get(), 1);
});

arch_test!(map_try_insert_first_grow_oom_propagates, {
    // The first `try_insert` takes `self.grow()?` (cap == 0), which calls
    // `Self::alloc_buckets(new_cap)?` → `alloc(layout)?`. Forcing that alloc
    // to fail drives the `?` arms in `alloc_buckets` (L172), `grow` (L234), and
    // `try_insert` (L196) in one shot.
    let mut m: Map<u32, u32> = Map::new();
    crate::alloc::fault::fail_after(0);
    let r = m.try_insert(1, 10);
    crate::alloc::fault::reset();
    testrt::check(r.is_err(), "try_insert surfaces AllocError when first grow fails");
    testrt::check(m.is_empty(), "map unchanged after the forced grow failure");
    // Usable once disarmed.
    m.try_insert(2, 20).unwrap();
    testrt::check_eq(m.get(&2), Some(&20));
});

arch_test!(map_layout_for_overflow_is_alloc_error, {
    // Drive `alloc_buckets`'s `layout_for(cap)?` overflow arm (map.rs L171)
    // directly: a bucket count near `usize::MAX` overflows `Layout::array`,
    // which `layout_for` maps to `AllocError`. `grow` never reaches such a cap
    // (it doubles from FIRST_CAPACITY, bounded by what `alloc` accepts), so
    // this arm is only reachable by exercising the helper directly.
    let r = Map::<u32, u32>::layout_for(usize::MAX);
    testrt::check_eq(r.err(), Some(AllocError));
    // The success arm is exercised by every allocating insert; assert it here
    // too so both arms of this helper are covered in one place.
    testrt::check(Map::<u32, u32>::layout_for(8).is_ok(), "a sane bucket count yields a layout");
});

arch_test!(map_alloc_buckets_layout_overflow_is_alloc_error, {
    // Drive the `?` Err arm at map.rs L171 (`layout_for(cap)?` inside
    // `alloc_buckets`) by calling `alloc_buckets` directly with a bucket count
    // that overflows `Layout::array`. `grow`'s doubling ladder never reaches
    // such a count, so this is the only reachable path for the `?` arm itself.
    // `layout_for` is exercised separately by the test above; this test
    // specifically exercises the `?` operator's Err branch *inside* `alloc_buckets`.
    let r = Map::<u32, u32>::alloc_buckets(usize::MAX);
    testrt::check_eq(r.err(), Some(AllocError));
});

arch_test!(map_try_insert_rehash_grow_oom_propagates, {
    // A second grow (rehash of an existing array) exercises the same `?` arms
    // with a non-zero old capacity. Fill to just under the load-factor
    // threshold, then force the grow that the next insert triggers.
    let mut m: Map<u32, u32> = Map::new();
    // FIRST_CAPACITY = 8; the grow threshold is `(len + 1) * 8 > cap * 7`.
    // Insert until one more would grow, then force that grow.
    for i in 0..7u32 {
        m.try_insert(i, i).unwrap();
    }
    let len_before = m.len();
    crate::alloc::fault::fail_after(0);
    let r = m.try_insert(100, 100);
    crate::alloc::fault::reset();
    testrt::check(r.is_err(), "rehash grow surfaces AllocError under fault");
    // The existing entries survive (grow leaves the old array on alloc failure).
    testrt::check_eq(m.len(), len_before);
    testrt::check_eq(m.get(&0), Some(&0));
    testrt::check_eq(m.get(&6), Some(&6));
});

arch_test!(map_remove_backward_shift_stay_arm_is_hit, {
    // Drive the `cyclic_in_range(hole, scan, ideal) == true` arm of
    // `remove`'s backward-shift (map.rs L327): an entry in the scan position
    // has its ideal slot strictly within `(hole, scan]`, so it stays put and
    // `hole` is NOT advanced.
    //
    // Setup (FIRST_CAPACITY=8, mask=7; FX_SEED%8=5):
    //   A = CollidingKey(2,1) → bucket 2 (ideal=2); placed at slot 2.
    //   B = CollidingKey(7,1) → bucket 3 (ideal=3); placed at slot 3.
    //   C = CollidingKey(2,2) → bucket 2 (ideal=2, collides with A); placed at slot 4.
    // 3 inserts keep cap=8 ((3+1)*8=32 <= 56).
    //
    // Remove A (slot 2 → hole=2):
    //   scan=3: B's ideal=3. cyclic_in_range(2,3,3)=true → B STAYS (hole=2).
    //   scan=4: C's ideal=2. cyclic_in_range(2,4,2)=false → C shifts to slot2, hole=4.
    //   scan=5: empty → exit.
    // Both arms of the shift branch are exercised.
    testrt::check_eq(FIRST_CAPACITY, 8); // guard: test depends on initial cap
    let mut m: Map<CollidingKey, u32> = Map::new();
    let key_a = CollidingKey(2, 1); // bucket 2
    let key_b = CollidingKey(7, 1); // bucket 3
    let key_c = CollidingKey(2, 2); // bucket 2, displaced to slot 4
    m.try_insert(key_a, 10).unwrap();
    m.try_insert(key_b, 20).unwrap();
    m.try_insert(key_c, 30).unwrap();
    testrt::check_eq(m.len(), 3);
    // Remove A: triggers the backward-shift with B staying and C shifting.
    testrt::check_eq(m.remove(&key_a), Some(10));
    testrt::check_eq(m.len(), 2);
    // B and C must still be reachable after the shift.
    testrt::check_eq(m.get(&key_b), Some(&20));
    testrt::check_eq(m.get(&key_c), Some(&30));
    testrt::check(m.get(&key_a).is_none(), "removed key is gone");
});
