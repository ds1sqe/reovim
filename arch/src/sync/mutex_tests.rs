//! Tests for `sync/mutex.rs`, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: declared in `sync/mod.rs` as
//! `#[cfg(feature = "selftest")] mod mutex_tests;`.
//!
//! Type substitutions from the original libtest-based tests:
//! - `std::sync::Arc` → `crate::ds::Shared`
//! - `std::thread::spawn` / `h.join().unwrap()` → `crate::thread::spawn` / `h.join()`
//! - `std::thread::sleep(Duration::from_millis(N))` → bounded spin on `crate::time::Instant`
//! - `std::thread::yield_now()` → `core::hint::spin_loop()`

use {
    crate::{arch_test, ds::Shared, sync::testhooks, testrt, thread, time::Instant},
    core::sync::atomic::{AtomicBool, Ordering as O},
};

use super::Mutex;

/// Spin for approximately `millis` milliseconds using `crate::time::Instant`.
/// This replaces `std::thread::sleep` in the selftest context where the
/// time-based delay only needs to be a lower bound.
fn spin_ms(millis: u64) {
    let start = Instant::now();
    let limit_ns = millis * 1_000_000;
    loop {
        let elapsed = Instant::now().elapsed_nanos(start);
        if elapsed >= limit_ns {
            break;
        }
        core::hint::spin_loop();
    }
}

arch_test!(mutex_lock_guards_value_single_thread, {
    let m = Mutex::new(0u32);
    {
        let mut g = m.lock();
        *g += 5;
    }
    testrt::check_eq(*m.lock(), 5);
});

arch_test!(mutex_try_lock_succeeds_when_free_fails_when_held, {
    let m = Mutex::new(7u32);
    let g = m.try_lock().expect("free lock acquires");
    testrt::check(m.try_lock().is_none(), "held lock refuses try_lock");
    drop(g);
    testrt::check(m.try_lock().is_some(), "freed lock acquires again");
});

arch_test!(mutex_eight_threads_ten_thousand_increments_exact_total, {
    // 8 arch threads, each incrementing a Mutex<u64> 10 000 times.
    // Uses `Shared` (arch Arc analog) instead of `std::sync::Arc`.
    let m = Shared::try_new(Mutex::new(0u64)).expect("alloc shared mutex");

    macro_rules! spawn_worker {
        ($m:expr) => {{
            let m2 = $m.clone();
            thread::spawn(move || {
                for _ in 0..10_000u32 {
                    *m2.lock() += 1;
                }
            })
            .expect("thread spawn")
        }};
    }

    let h0 = spawn_worker!(m);
    let h1 = spawn_worker!(m);
    let h2 = spawn_worker!(m);
    let h3 = spawn_worker!(m);
    let h4 = spawn_worker!(m);
    let h5 = spawn_worker!(m);
    let h6 = spawn_worker!(m);
    let h7 = spawn_worker!(m);
    let () = h0.join();
    let () = h1.join();
    let () = h2.join();
    let () = h3.join();
    let () = h4.join();
    let () = h5.join();
    let () = h6.join();
    let () = h7.join();

    testrt::check_eq(*m.lock(), 80_000u64);
});

arch_test!(mutex_forced_futex_slow_path_executes_contended_branch, {
    let before = testhooks::contended_transitions();
    let waits_before = testhooks::waits_entered();

    let m = Shared::try_new(Mutex::new(0u32)).expect("alloc shared mutex");
    // The holder takes the lock and keeps it until the waiter has begun
    // blocking. `waiter_ready` signals the waiter to attempt; the holder
    // then busy-waits for the waiter's FUTEX_WAIT before releasing, so the
    // contended (1->2) transition is forced deterministically.
    let waiter_ready = Shared::try_new(AtomicBool::new(false)).expect("alloc ready flag");
    let waiter_blocking = Shared::try_new(AtomicBool::new(false)).expect("alloc blocking flag");

    let mut g = m.lock();

    let m2 = m.clone();
    let ready2 = waiter_ready.clone();
    let blocking2 = waiter_blocking.clone();
    let waiter = thread::spawn(move || {
        ready2.store(true, O::SeqCst);
        blocking2.store(true, O::SeqCst);
        // This lock() must traverse the slow path (holder still holds it).
        let v = *m2.lock();
        assert!(v == 1);
    })
    .expect("waiter spawn");

    // Spin until the waiter signals it is about to block.
    while !waiter_ready.load(O::SeqCst) {
        core::hint::spin_loop();
    }
    while !waiter_blocking.load(O::SeqCst) {
        core::hint::spin_loop();
    }
    // Give the waiter time to exhaust its spin and enter FUTEX_WAIT.
    spin_ms(50);

    // Mutate under the lock, then release: dropping the guard unlocks and
    // wakes the waiter.
    *g = 1;
    drop(g);
    let () = waiter.join();

    testrt::check(
        testhooks::contended_transitions() > before,
        "the 1->2 contended transition must have executed",
    );
    testrt::check(
        testhooks::waits_entered() > waits_before,
        "the waiter must have entered FUTEX_WAIT",
    );
});

arch_test!(mutex_contended_transitions_counter_is_observable, {
    // The hook counters are plain observation points; reading them must
    // not panic and must be monotone non-decreasing.
    let a = testhooks::contended_transitions();
    let b = testhooks::contended_transitions();
    testrt::check(b >= a, "counter is non-decreasing");
});
