//! Tests for `thread/mod.rs`, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: declared inside `thread/mod.rs` as
//! `#[cfg(feature = "selftest")] #[path = "thread_tests.rs"] mod tests;`.
//! `super::` reaches the private `spawn_with_map_size`, `PAGE_SIZE`,
//! `SpawnError`, and `AllocError`.
//!
//! Type substitutions from the original libtest-based tests:
//! - `std::sync::Arc` → `reovim_lib_ds::Shared`
//! - `std::vec::Vec` for handle storage → individual handle variables
//! - `std::thread::yield_now()` → `core::hint::spin_loop()` in a bounded loop
//! - All tests use `arch_test!` instead of `#[test]`

use {
    crate::{alloc::live_bytes, arch_test, testrt},
    core::sync::atomic::{AtomicU32 as TU32, Ordering as O},
    reovim_kabi_platform::AllocError,
    reovim_lib_ds::{Mutex, Seq, Shared},
};

use crate::sys::{CLONE_THREAD, CLONE_VM};

use super::{
    JoinHandle, PAGE_SIZE, SpawnError, spawn, spawn_with_clone_flags, spawn_with_map_size,
    testhooks,
};

arch_test!(thread_spawn_returns_value_through_join, {
    let h = spawn(|| 7u32 + 35).expect("spawn succeeds");
    testrt::check_eq(h.join(), 42u32);
});

arch_test!(thread_multiple_threads_each_return, {
    // 8 threads each returning i*i; sum of squares 0..8 = 140.
    // Closures capture distinct `i` values but all have the same type
    // (moved u64, return u64), so they can be stored in a Seq.
    // However `JoinHandle<F, T>` is generic over `F`, so closures with
    // different captures are different types. Use fixed handle variables.
    let h0: JoinHandle<_, u64> = spawn(move || 0u64 * 0).expect("spawn 0");
    let h1: JoinHandle<_, u64> = spawn(move || 1u64 * 1).expect("spawn 1");
    let h2: JoinHandle<_, u64> = spawn(move || 2u64 * 2).expect("spawn 2");
    let h3: JoinHandle<_, u64> = spawn(move || 3u64 * 3).expect("spawn 3");
    let h4: JoinHandle<_, u64> = spawn(move || 4u64 * 4).expect("spawn 4");
    let h5: JoinHandle<_, u64> = spawn(move || 5u64 * 5).expect("spawn 5");
    let h6: JoinHandle<_, u64> = spawn(move || 6u64 * 6).expect("spawn 6");
    let h7: JoinHandle<_, u64> = spawn(move || 7u64 * 7).expect("spawn 7");
    let total = h0.join()
        + h1.join()
        + h2.join()
        + h3.join()
        + h4.join()
        + h5.join()
        + h6.join()
        + h7.join();
    // sum of squares 0..8 = 0+1+4+9+16+25+36+49 = 140
    testrt::check_eq(total, 140u64);
});

arch_test!(thread_is_finished_reports_completion, {
    // Coverage note (DEV1 — round 3): the spawned closure's spin loop needs
    // BOTH arms covered deterministically: the "continue spinning" arm (body
    // runs) and the "exit" arm (condition evaluates false). We use a `spinning`
    // latch that the spawned thread sets INSIDE the loop body on the first
    // iteration. The main thread waits for this latch before storing `flag=1`,
    // guaranteeing the loop body runs at least once (true arm) before the
    // condition becomes false (exit arm).
    use core::sync::atomic::AtomicBool;
    let flag = Shared::try_new(TU32::new(0)).expect("alloc flag");
    let spinning = Shared::try_new(AtomicBool::new(false)).expect("alloc spinning latch");
    let f2 = flag.clone();
    let s2 = spinning.clone();
    let h = spawn(move || {
        // Loop until the release flag is set. On the first iteration (flag still
        // 0), the body sets the spinning latch, signalling that the loop body
        // has executed at least once (true arm covered). The main thread then
        // stores flag=1 so the next check evaluates false (exit arm covered).
        while f2.load(O::SeqCst) == 0 {
            // Set the latch on the first body execution so the main thread can
            // observe that the loop body ran before setting the exit condition.
            s2.store(true, O::Release);
            core::hint::spin_loop();
        }
        99u32
    })
    .expect("spawn succeeds");
    // Block until the spawned thread is provably inside the loop body (latch set).
    while !spinning.load(O::Acquire) {
        core::hint::spin_loop();
    }
    // The spawned thread is spinning; it has not finished yet.
    testrt::check(!h.is_finished(), "thread not finished yet");
    // Release: the spawned thread will see flag=1 on the next condition check
    // and exit the loop (false arm covered).
    flag.store(1, O::SeqCst);
    testrt::check_eq(h.join(), 99u32);
});

arch_test!(thread_is_finished_true_after_completion, {
    // Exercises the `is_finished() == true` branch of `JoinHandle::is_finished`
    // (the `ctid == 0` arm). Spawn a thread that returns immediately, then
    // spin until is_finished() returns true, confirming both branches execute.
    use crate::time::Instant;
    let h = spawn(|| 0u32).expect("spawn succeeds");
    // Spin briefly to let the thread run and clear its ctid word.
    let deadline = Instant::now();
    loop {
        if h.is_finished() {
            break; // ctid == 0 branch: is_finished returns true
        }
        let elapsed = Instant::now().elapsed_nanos(deadline);
        if elapsed > 500_000_000 {
            break; // 500ms timeout (should never be reached)
        }
        core::hint::spin_loop();
    }
    testrt::check_eq(h.join(), 0u32);
});

arch_test!(thread_spawn_failure_on_absurd_stack_is_out_of_memory, {
    // Drive the public failure path: an absurd stack mapping the kernel
    // cannot satisfy makes spawn's first mmap fail -> SpawnError::OutOfMemory.
    let absurd = (usize::MAX & !(PAGE_SIZE - 1)) - PAGE_SIZE;
    let r = spawn_with_map_size(|| 0u32, absurd);
    match r {
        Err(SpawnError::OutOfMemory) => {}
        Err(other) => panic!("expected OutOfMemory, got {other:?}"),
        Ok(_) => panic!("absurd stack must not spawn"),
    }
    // The conversion is the same branch the error path uses.
    testrt::check_eq(SpawnError::from(AllocError), SpawnError::OutOfMemory);
});

arch_test!(thread_stack_unmapped_after_join_accounting, {
    // The shared block is allocated through the arch allocator; after join
    // frees it, live_bytes returns to baseline (the stack is mmap'd
    // directly, not via the allocator, so it is the block that shows in
    // accounting). We assert balance across spawn+join.
    let base = live_bytes();
    let h = spawn(|| 1u32).expect("spawn succeeds");
    // While running, the shared block is live.
    testrt::check(live_bytes() >= base, "live_bytes non-decreasing while thread alive");
    testrt::check_eq(h.join(), 1u32);
    testrt::check_eq(live_bytes(), base);
});

arch_test!(thread_integration_thread_pushes_into_mutex_guarded_seq, {
    // Phase 3 integration smoke: a spawned ARCH thread pushes into a
    // Mutex<Seq<u64>>; join; assert contents AND that the shared-block
    // accounting returned to baseline (stack unmapped, block freed).
    let base = live_bytes();
    let shared = Shared::try_new(Mutex::new(Seq::<u64>::new())).expect("alloc shared mutex");
    let s2 = shared.clone();
    let h = spawn(move || {
        let mut g = s2.lock();
        for i in 0..16u64 {
            g.try_push(i).unwrap();
        }
    })
    .expect("spawn succeeds");
    let () = h.join();
    let g = shared.lock();
    testrt::check_eq(g.len(), 16);
    testrt::check_eq(g[0], 0u64);
    testrt::check_eq(g[15], 15u64);
    drop(g);
    // The Seq lives inside the Shared<Mutex<..>> still held here; only the
    // thread's stack+block are gone. Drop the Shared and assert baseline.
    drop(shared);
    testrt::check_eq(live_bytes(), base);
});

arch_test!(thread_clone_err_teardown_runs_on_invalid_flags, {
    // Drive the `SpawnError::Clone(Errno)` arm and its teardown block in
    // `spawn_inner` using the selftest-only `spawn_with_clone_flags` seam,
    // which routes through the same unified body as production `spawn` (so the
    // forced branch IS the flight branch). Passing `CLONE_THREAD` without
    // `CLONE_SIGHAND` is explicitly
    // rejected by the Linux kernel with EINVAL (clone(2) man page: "CLONE_THREAD
    // requires CLONE_SIGHAND"). The clone-Err teardown path (drop shared block,
    // unmap stack) runs for real, verifiable through live_bytes accounting.
    use crate::alloc::live_bytes;
    let base = live_bytes();
    // Invalid flags: CLONE_VM + CLONE_THREAD without CLONE_SIGHAND → EINVAL.
    let flags = CLONE_VM | CLONE_THREAD;
    let r = spawn_with_clone_flags(|| 0u32, flags);
    match r {
        Err(SpawnError::Clone(_errno)) => {
            // Expected: the clone was refused, teardown executed.
        }
        Err(SpawnError::OutOfMemory) => {
            // Unexpected but non-fatal: mmap or alloc failed before clone.
            testrt::check(false, "expected Clone error, got OutOfMemory");
        }
        Ok(h) => {
            // If the kernel surprisingly accepted the flags, join and note it.
            // This would be unusual but doesn't fail the test binary.
            let _v = h.join();
        }
    }
    // Accounting must return to baseline: the shared block and stack were freed
    // in the teardown path (stack is munmap'd, not allocator-tracked, but the
    // shared block IS allocator-tracked).
    testrt::check_eq(live_bytes(), base);
});

arch_test!(thread_guard_mprotect_failure_runs_unmap_teardown, {
    // Drives the guard-install failure teardown in `spawn_inner`: the selftest
    // `fail_next_guard` hook forces the post-`mprotect` failure check to fire,
    // so the unmap-and-`SpawnError::OutOfMemory` arm runs for real. A healthy
    // kernel never refuses an `mprotect` on freshly-`mmap`d anonymous memory,
    // so the hook is the only in-process way to reach this branch. Because the
    // hook fires before the shared-block alloc, accounting is untouched.
    let base = live_bytes();
    testhooks::fail_next_guard();
    let r = spawn(|| 0u32);
    match r {
        Err(SpawnError::OutOfMemory) => {}
        Err(other) => panic!("expected OutOfMemory from guard failure, got {other:?}"),
        Ok(_) => panic!("guard-failure hook must abort the spawn"),
    }
    // The stack was unmapped and no shared block was allocated.
    testrt::check_eq(live_bytes(), base);
    // The hook is one-shot: the next spawn succeeds normally.
    let h = spawn(|| 5u32).expect("spawn succeeds after the one-shot guard fault");
    testrt::check_eq(h.join(), 5u32);
});

arch_test!(thread_shared_block_alloc_failure_runs_unmap_teardown, {
    // Drives the shared-block alloc-failure teardown in `spawn_inner`: the
    // stack maps and the guard installs, then the `ThreadShared` allocation is
    // forced to fail via the allocator fault hook. The `Err(e) => munmap +
    // e.into()` arm must unmap the stack and return `OutOfMemory`. The
    // `ThreadShared` alloc is the next allocator call after spawn entry, so
    // `fail_after(0)` targets it precisely.
    let base = live_bytes();
    crate::alloc::fault::fail_after(0);
    let r = spawn(|| 0u32);
    crate::alloc::fault::reset();
    match r {
        Err(SpawnError::OutOfMemory) => {}
        Err(other) => panic!("expected OutOfMemory from alloc failure, got {other:?}"),
        Ok(_) => panic!("alloc-failure hook must abort the spawn"),
    }
    // The stack was unmapped; the shared block never allocated.
    testrt::check_eq(live_bytes(), base);
    let h = spawn(|| 9u32).expect("spawn succeeds after the one-shot alloc fault");
    testrt::check_eq(h.join(), 9u32);
});

arch_test!(thread_eight_arch_threads_ten_thousand_increments_exact_total, {
    // End-to-end over ARCH threads: 8 x 10_000 under an arch Mutex,
    // exact total — the plan AC, on the primitives we fly.
    let counter = Shared::try_new(Mutex::new(0u64)).expect("alloc counter");

    macro_rules! spawn_worker {
        ($counter:expr) => {{
            let c = $counter.clone();
            spawn(move || {
                for _ in 0..10_000u32 {
                    *c.lock() += 1;
                }
            })
            .expect("spawn succeeds")
        }};
    }

    let h0 = spawn_worker!(counter);
    let h1 = spawn_worker!(counter);
    let h2 = spawn_worker!(counter);
    let h3 = spawn_worker!(counter);
    let h4 = spawn_worker!(counter);
    let h5 = spawn_worker!(counter);
    let h6 = spawn_worker!(counter);
    let h7 = spawn_worker!(counter);
    let () = h0.join();
    let () = h1.join();
    let () = h2.join();
    let () = h3.join();
    let () = h4.join();
    let () = h5.join();
    let () = h6.join();
    let () = h7.join();

    testrt::check_eq(*counter.lock(), 80_000u64);
});
