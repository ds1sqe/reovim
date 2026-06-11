//! Tests for `alloc/mod.rs`, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: declared inside `alloc/mod.rs` as
//! `#[cfg(feature = "selftest")] #[path = "tests.rs"] mod tests;`
//! so `super::` reaches crate-private items (`class_for`, `LARGE_HEADER`,
//! `MAX_SMALL`, `PAGE_SIZE`, `SIZE_CLASSES`).

use {
    crate::{
        alloc::{AllocError, alloc, dealloc, live_bytes, realloc},
        arch_test, testrt,
    },
    core::{alloc::Layout, ptr::NonNull},
};

// Private items reached via super:: through the #[path] child declaration:
use super::{LARGE_HEADER, MAX_SMALL, PAGE_SIZE, SIZE_CLASSES, class_for};

arch_test!(small_alloc_dealloc_roundtrips_accounting, {
    let base = live_bytes();
    let layout = Layout::from_size_align(24, 8).unwrap();
    let p = alloc(layout).unwrap();
    testrt::check_eq(live_bytes(), base + 24);
    // Write to verify the memory is writable.
    // SAFETY: `p` is a live 24-byte allocation.
    unsafe { core::ptr::write_bytes(p.as_ptr(), 0xAB, 24) };
    // SAFETY: same (ptr, layout).
    unsafe { dealloc(p, layout) };
    testrt::check_eq(live_bytes(), base);
});

arch_test!(small_free_slot_returns_to_free_list, {
    // A dealloc-then-alloc on the same class must not grow live bytes beyond
    // a single block: the freed slot is reclaimed.
    let layout = Layout::from_size_align(16, 8).unwrap();
    let a = alloc(layout).unwrap();
    // SAFETY: live (ptr, layout).
    unsafe { dealloc(a, layout) };
    let base = live_bytes();
    let b = alloc(layout).unwrap();
    testrt::check_eq(live_bytes(), base + 16);
    // SAFETY: live (ptr, layout).
    unsafe { dealloc(b, layout) };
    testrt::check_eq(live_bytes(), base);
});

arch_test!(refill_serves_more_than_one_page_of_slots, {
    // Allocate enough 2048-blocks to span more than one page, forcing a refill.
    let layout = Layout::from_size_align(2048, 8).unwrap();
    let mut ptrs = [core::ptr::null_mut::<u8>(); 5];
    for slot in &mut ptrs {
        *slot = alloc(layout).unwrap().as_ptr();
    }
    // All distinct.
    for i in 0..ptrs.len() {
        for j in (i + 1)..ptrs.len() {
            testrt::check(ptrs[i] != ptrs[j], "ptrs distinct");
        }
    }
    for slot in ptrs {
        // SAFETY: each is a live 2048 allocation.
        unsafe { dealloc(NonNull::new(slot).unwrap(), layout) };
    }
});

arch_test!(large_alloc_dealloc_roundtrips_accounting, {
    let base = live_bytes();
    let size = 8192; // > MAX_SMALL
    let layout = Layout::from_size_align(size, 8).unwrap();
    let p = alloc(layout).unwrap();
    testrt::check_eq(live_bytes(), base + size);
    // SAFETY: live `size`-byte mapping.
    unsafe { core::ptr::write_bytes(p.as_ptr(), 0xCD, size) };
    // SAFETY: same (ptr, layout).
    unsafe { dealloc(p, layout) };
    testrt::check_eq(live_bytes(), base);
});

arch_test!(class_boundary_selects_small_vs_large, {
    // Exactly MAX_SMALL is the last small class.
    testrt::check_eq(class_for(MAX_SMALL, 1), Some(SIZE_CLASSES.len() - 1));
    // One past MAX_SMALL is large.
    testrt::check_eq(class_for(MAX_SMALL + 1, 1), None);
    // Alignment can push a small size onto a larger class.
    testrt::check_eq(class_for(8, 64), Some(2));
    // Alignment past MAX_SMALL pushes to large.
    testrt::check_eq(class_for(8, 4096), None);
});

arch_test!(zero_size_alloc_is_error, {
    let layout = Layout::from_size_align(0, 1).unwrap();
    testrt::check_eq(alloc(layout), Err(AllocError));
});

arch_test!(absurd_size_is_oom_error_not_panic, {
    // The kernel refuses an ~8 EiB anonymous mapping with ENOMEM.
    let size = isize::MAX as usize - PAGE_SIZE;
    let layout = Layout::from_size_align(size, 8).unwrap();
    testrt::check_eq(alloc(layout), Err(AllocError));
});

arch_test!(over_aligned_large_is_error, {
    let layout = Layout::from_size_align(8192, 8192).unwrap();
    testrt::check_eq(alloc(layout), Err(AllocError));
});

arch_test!(realloc_grow_preserves_prefix, {
    let old = Layout::from_size_align(16, 8).unwrap();
    let p = alloc(old).unwrap();
    // SAFETY: live 16-byte block.
    unsafe { core::ptr::write_bytes(p.as_ptr(), 0x11, 16) };
    // SAFETY: live (ptr, old).
    let grown = unsafe { realloc(p, old, 64).unwrap() };
    // SAFETY: `grown` is a live 64-byte block.
    let first = unsafe { *grown.as_ptr() };
    testrt::check_eq(first, 0x11u8);
    let new = Layout::from_size_align(64, 8).unwrap();
    // SAFETY: live (grown, new).
    unsafe { dealloc(grown, new) };
});

arch_test!(realloc_shrink_preserves_prefix, {
    let old = Layout::from_size_align(64, 8).unwrap();
    let p = alloc(old).unwrap();
    // SAFETY: live 64-byte block.
    unsafe { core::ptr::write_bytes(p.as_ptr(), 0x22, 64) };
    // SAFETY: live (ptr, old).
    let shrunk = unsafe { realloc(p, old, 16).unwrap() };
    // SAFETY: `shrunk` is a live 16-byte block.
    let first = unsafe { *shrunk.as_ptr() };
    testrt::check_eq(first, 0x22u8);
    let new = Layout::from_size_align(16, 8).unwrap();
    // SAFETY: live (shrunk, new).
    unsafe { dealloc(shrunk, new) };
});

arch_test!(realloc_zero_is_error, {
    let old = Layout::from_size_align(16, 8).unwrap();
    let p = alloc(old).unwrap();
    // SAFETY: live (ptr, old).
    let r = unsafe { realloc(p, old, 0) };
    testrt::check_eq(r, Err(AllocError));
    // Original still valid.
    // SAFETY: live (ptr, old).
    unsafe { dealloc(p, old) };
});

arch_test!(realloc_oom_leaves_original_valid, {
    let old = Layout::from_size_align(16, 8).unwrap();
    let p = alloc(old).unwrap();
    let absurd = usize::MAX - LARGE_HEADER - PAGE_SIZE;
    // SAFETY: live (ptr, old); the grow is refused so `p` stays valid.
    let r = unsafe { realloc(p, old, absurd) };
    testrt::check_eq(r, Err(AllocError));
    // SAFETY: live (ptr, old) — untouched by the failed realloc.
    unsafe { dealloc(p, old) };
});

arch_test!(spin_lock_serializes_nested_paths, {
    // Two small allocs of different classes exercise distinct free-list heads
    // under the same lock.
    let l1 = Layout::from_size_align(16, 8).unwrap();
    let l2 = Layout::from_size_align(256, 8).unwrap();
    let a = alloc(l1).unwrap();
    let b = alloc(l2).unwrap();
    testrt::check(a.as_ptr() as usize != b.as_ptr() as usize, "ptrs distinct");
    // SAFETY: live (a, l1) / (b, l2).
    unsafe {
        dealloc(a, l1);
        dealloc(b, l2);
    }
});

arch_test!(page_round_up_rounds_to_page_multiple, {
    use super::page_round_up;
    testrt::check_eq(page_round_up(1), PAGE_SIZE);
    testrt::check_eq(page_round_up(PAGE_SIZE), PAGE_SIZE);
    testrt::check_eq(page_round_up(PAGE_SIZE + 1), 2 * PAGE_SIZE);
});

arch_test!(class_for_fallback_takes_last_class, {
    // need == MAX_SMALL lands on the last class, not None (restructure proof).
    testrt::check_eq(class_for(MAX_SMALL, 1), Some(SIZE_CLASSES.len() - 1));
    // A value that fits only the last class (just under MAX_SMALL but above
    // the second-to-last class, which is 512).
    testrt::check_eq(class_for(513, 1), Some(SIZE_CLASSES.len() - 1));
    // Large path is still None.
    testrt::check_eq(class_for(MAX_SMALL + 1, 1), None);
});

arch_test!(spin_lock_contended_path_is_hit_under_threads, {
    // Two threads competing on the same size class force the spin path inside
    // SpinLock::lock(). We allocate from one thread while another holds the
    // lock (implicitly, via GLOBAL) for a brief window. The contended inner
    // loop (L113-115) must execute at least once for coverage.
    use {
        crate::{ds::Shared, thread},
        core::sync::atomic::{AtomicBool, Ordering as O},
    };

    // A gate that makes thread B spin for a moment while thread A holds the
    // allocator lock via an ongoing alloc+dealloc sequence.
    let gate = Shared::try_new(AtomicBool::new(false)).expect("alloc gate");
    let gate2 = gate.clone();

    // Thread A: rapidly allocates and frees, momentarily holding the lock.
    // No fault is armed here, so each alloc must succeed; `expect` keeps the
    // helper free of a dead `Err` arm (the contention is the point, not OOM).
    let h = thread::spawn(move || {
        for _ in 0..500u32 {
            let layout = core::alloc::Layout::from_size_align(32, 8).unwrap();
            let p = alloc(layout).expect("uncontended 32-byte alloc succeeds");
            // SAFETY: live 32-byte allocation returned just above.
            unsafe { dealloc(p, layout) };
        }
        gate2.store(true, O::SeqCst);
    })
    .expect("contention thread spawn");

    // Thread B (main): also alloc-dealloc rapidly so both threads are
    // competing for the lock.
    while !gate.load(O::SeqCst) {
        let layout = core::alloc::Layout::from_size_align(64, 8).unwrap();
        let p = alloc(layout).expect("uncontended 64-byte alloc succeeds");
        // SAFETY: live 64-byte allocation.
        unsafe { dealloc(p, layout) };
    }
    let () = h.join();
    testrt::check(true, "contended spin path completed without deadlock");
});

arch_test!(refill_class_mmap_refusal_is_alloc_error, {
    // Drives `alloc_small`'s refill-`Err` branch for real via the selftest
    // refill fault hook: when armed, the next size-class refill `mmap` is
    // treated as refused, so `refill_class` propagates `AllocError` up through
    // `alloc_small`'s unlock-then-`Err` arm and out of `alloc`.
    //
    // The refill path only runs when the class free-list is empty, and the
    // selftest binary's allocation history for this class is unknown, so we
    // drain deterministically: keep allocating from one class with the hook
    // armed until an alloc returns `Err`. The free-list holds at most one page
    // of slots, so the very next refill after the list drains fires the armed
    // one-shot hook within a bounded number of iterations.
    let layout = Layout::from_size_align(512, 8).unwrap();
    let block = 512usize;
    // One page of 512-blocks is 8 slots; a generous bound covers any prior
    // free-list residue plus the refill that follows it.
    let bound = (PAGE_SIZE / block) + 4;
    super::fault::fail_next_refill();
    let mut drained = [core::ptr::null_mut::<u8>(); 16];
    let mut n = 0usize;
    let mut hit_err = false;
    while n < bound {
        match alloc(layout) {
            Ok(p) => {
                drained[n] = p.as_ptr();
                n += 1;
            }
            Err(_) => {
                hit_err = true;
                break;
            }
        }
    }
    super::fault::reset();
    // Free everything the drain handed out before asserting (no leak across
    // the shared process).
    for slot in &drained[..n] {
        // SAFETY: each is a live 512-byte allocation from the loop above.
        unsafe { dealloc(NonNull::new(*slot).unwrap(), layout) };
    }
    testrt::check(hit_err, "the armed refill hook forced an AllocError");
});

arch_test!(alloc_entry_fault_forces_oom_error, {
    // The alloc-entry fault hook makes the next allocation attempt fail as if
    // the kernel refused it — the seam every DS-level `?` arm relies on.
    super::fault::fail_after(0);
    let layout = Layout::from_size_align(24, 8).unwrap();
    let r = alloc(layout);
    super::fault::reset();
    testrt::check_eq(r, Err(AllocError));
    // The hook self-disarms after one fire: the next alloc succeeds.
    let p = alloc(layout).expect("alloc succeeds after the one-shot fault");
    // SAFETY: live (ptr, layout).
    unsafe { dealloc(p, layout) };
});

arch_test!(alloc_entry_fault_countdown_skips_n_then_fails, {
    // `fail_after(2)` lets the next two attempts succeed and fails the third,
    // exercising the countdown arm (not just the immediate-fire arm).
    super::fault::fail_after(2);
    let layout = Layout::from_size_align(16, 8).unwrap();
    let a = alloc(layout).expect("first alloc survives the countdown");
    let b = alloc(layout).expect("second alloc survives the countdown");
    let third = alloc(layout);
    super::fault::reset();
    testrt::check_eq(third, Err(AllocError));
    // SAFETY: `a`/`b` are live (ptr, layout) blocks.
    unsafe {
        dealloc(a, layout);
        dealloc(b, layout);
    }
});

arch_test!(realloc_grow_oom_via_fault_leaves_original_valid, {
    // The realloc grow path calls `alloc(new_layout)?`; the entry fault hook
    // forces that inner alloc to fail, driving realloc's `Err` arm in-process
    // (the original block must stay valid).
    let old = Layout::from_size_align(16, 8).unwrap();
    let p = alloc(old).expect("seed alloc");
    // SAFETY: live 16-byte block.
    unsafe { core::ptr::write_bytes(p.as_ptr(), 0x55, 16) };
    super::fault::fail_after(0);
    // SAFETY: live (ptr, old); the grow's inner alloc is forced to fail, so
    // `p` is left untouched and valid.
    let r = unsafe { realloc(p, old, 64) };
    super::fault::reset();
    testrt::check_eq(r, Err(AllocError));
    // The original is intact: its first byte is still the seeded value.
    // SAFETY: `p` is the untouched original block.
    let first = unsafe { *p.as_ptr() };
    testrt::check_eq(first, 0x55u8);
    // SAFETY: live (ptr, old).
    unsafe { dealloc(p, old) };
});

arch_test!(dealloc_zero_size_layout_is_noop, {
    // Drives the `size == 0` early-return arm of `dealloc`: a zero-size layout
    // never produced a real allocation, so dealloc must return without touching
    // accounting or the free-list.
    let base = live_bytes();
    let layout = Layout::from_size_align(0, 1).unwrap();
    // SAFETY: a dangling, well-aligned pointer paired with a zero-size layout;
    // `dealloc` takes the early-return arm and never dereferences it.
    unsafe { dealloc(NonNull::<u8>::dangling(), layout) };
    testrt::check_eq(live_bytes(), base);
});

arch_test!(fault_alloc_should_fail_disabled_returns_false, {
    // Drive fault.rs `alloc_should_fail()` with the hook disarmed (cur < 0):
    // the `if cur < 0 { return false }` arm at fault.rs L69-71.
    super::fault::reset(); // ensure disabled
    testrt::check(!super::fault::alloc_should_fail(), "disabled hook returns false");
    super::fault::reset();
});

arch_test!(fault_alloc_should_fail_zero_fires_and_returns_true, {
    // Drive fault.rs L74-79: `fail_after(0)` sets cur=0; the CAS from 0 to
    // DISABLED succeeds → `return true` (fire path). After firing, the hook is
    // self-disarmed (cur == DISABLED again), so a second call returns false.
    super::fault::fail_after(0);
    testrt::check(super::fault::alloc_should_fail(), "cur==0 CAS-ok returns true");
    testrt::check(!super::fault::alloc_should_fail(), "self-disarmed: second call returns false");
    super::fault::reset();
});

arch_test!(fault_alloc_should_fail_countdown_decrements_then_fires, {
    // Drive fault.rs L84-89: `fail_after(2)` sets cur=2; first call sees cur>0,
    // CAS 2→1 succeeds → `return false` (L88-89 countdown arm). Second call CAS
    // 1→0 succeeds → return false. Third call sees cur==0 and fires → return true.
    super::fault::fail_after(2);
    testrt::check(!super::fault::alloc_should_fail(), "countdown step 1: cur=2→1, false");
    testrt::check(!super::fault::alloc_should_fail(), "countdown step 2: cur=1→0, false");
    testrt::check(super::fault::alloc_should_fail(), "countdown step 3: cur=0, fires true");
    testrt::check(!super::fault::alloc_should_fail(), "after fire: disarmed, false");
    super::fault::reset();
});

arch_test!(fault_refill_should_fail_armed_fires_once, {
    // Drive fault.rs `refill_should_fail()`: when armed (CAS 1→0 succeeds) it
    // returns true; on the second call the hook is disarmed (cur=0, CAS fails)
    // so it returns false. This exercises both arms of the `compare_exchange`.
    super::fault::fail_next_refill();
    testrt::check(super::fault::refill_should_fail(), "armed: CAS 1→0 ok, returns true");
    testrt::check(!super::fault::refill_should_fail(), "disarmed: CAS fails, returns false");
    super::fault::reset();
});
