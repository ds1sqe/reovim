//! Tests for `sync/condvar.rs`, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: declared in `sync/mod.rs` as
//! `#[cfg(feature = "selftest")] mod condvar_tests;`.
//!
//! Type substitutions from the original libtest-based tests:
//! - `std::sync::Arc<(Mutex<bool>, Condvar)>` → `Shared<(Mutex<bool>, Condvar)>`
//! - `std::sync::Arc<AtomicU32>` → `Shared<AtomicU32>`
//! - `std::thread::spawn` / `h.join().unwrap()` → `crate::thread::spawn` / `h.join()`
//! - `std::thread::sleep(Duration::from_millis(N))` → bounded spin on `crate::time::Instant`
//!
//! The `missed_wake_window_does_not_deadlock` test originally ran 20 rounds.
//! Each round spawns a thread and joins it. This stresses the missed-wake
//! sequence-counter mechanism without timing dependencies.

use crate::arch_test;
// Only the spawn-dependent (Linux-gated) cases consume these.
#[cfg(target_os = "linux")]
use {
    crate::{testrt, thread, time::Instant},
    core::sync::atomic::{AtomicBool, AtomicU32 as TU32, Ordering as O},
    reovim_lib_ds::Shared,
};

use reovim_lib_ds::Condvar;
#[cfg(target_os = "linux")]
use reovim_lib_ds::Mutex;

/// Spin for approximately `millis` milliseconds using `crate::time::Instant`.
#[cfg(target_os = "linux")]
fn spin_ms(millis: u64) {
    let start = Instant::now();
    let limit_ns = millis * 1_000_000;
    loop {
        if Instant::now().elapsed_nanos(start) >= limit_ns {
            break;
        }
        core::hint::spin_loop();
    }
}

arch_test!(condvar_default_constructs, {
    let _c = Condvar::default();
});

// Spawn-dependent cases: they need a thread floor, which freestanding
// targets do not realize. They stay in every hosted suite.
#[cfg(target_os = "linux")]
arch_test!(condvar_notify_one_wakes_a_waiter, {
    // A waiter blocks on the condvar guarding a bool predicate; the main
    // thread sets the predicate and notifies. The waiter must return.
    let pair = Shared::try_new((Mutex::new(false), Condvar::new())).expect("alloc pair");
    let pair2 = pair.clone();
    let waiter = thread::spawn(move || {
        let (m, c) = &*pair2;
        let mut g = m.lock();
        while !*g {
            g = c.wait(g);
        }
        assert!(*g);
    })
    .expect("waiter spawn");

    // Let the waiter reach wait().
    spin_ms(40);
    {
        let (m, c) = &*pair;
        *m.lock() = true;
        c.notify_one();
    }
    let () = waiter.join();
});

#[cfg(target_os = "linux")]
arch_test!(condvar_notify_all_wakes_several, {
    let pair = Shared::try_new((Mutex::new(false), Condvar::new())).expect("alloc pair");
    let woken = Shared::try_new(TU32::new(0)).expect("alloc woken counter");

    macro_rules! spawn_waiter {
        ($pair:expr, $woken:expr) => {{
            let pair2 = $pair.clone();
            let woken2 = $woken.clone();
            thread::spawn(move || {
                let (m, c) = &*pair2;
                let mut g = m.lock();
                while !*g {
                    g = c.wait(g);
                }
                woken2.fetch_add(1, O::SeqCst);
            })
            .expect("waiter spawn")
        }};
    }

    let h0 = spawn_waiter!(pair, woken);
    let h1 = spawn_waiter!(pair, woken);
    let h2 = spawn_waiter!(pair, woken);
    let h3 = spawn_waiter!(pair, woken);

    spin_ms(40);
    {
        let (m, c) = &*pair;
        *m.lock() = true;
        c.notify_all();
    }
    let () = h0.join();
    let () = h1.join();
    let () = h2.join();
    let () = h3.join();
    testrt::check_eq(woken.load(O::SeqCst), 4u32);
});

#[cfg(target_os = "linux")]
arch_test!(condvar_missed_wake_window_does_not_deadlock, {
    // Force the race: the notifier sets the predicate and notifies in the
    // window where the waiter has decided to wait but may not have slept.
    // The sequence re-check must prevent a permanent block. Run several
    // rounds to probe the timing window.
    //
    // Coverage note (DEV1 — round 3): the `while !*g { g = c.wait(g); }` loop
    // at lines 113-114 needs BOTH arms covered deterministically. We use a
    // `ready` flag: the waiter sets it after acquiring the lock (just before
    // calling `c.wait`), and the notifier spins on it before setting the
    // predicate. This guarantees `c.wait(g)` executes in at least one round
    // (the true arm of the while) while later rounds still probe the missed-wake
    // window (the notifier may fire before the waiter re-enters the condvar).
    // The original opportunistic timing is preserved for rounds 1-19; round 0
    // uses the synchronized path.
    for round in 0..20u32 {
        let pair = Shared::try_new((Mutex::new(false), Condvar::new())).expect("alloc pair");
        let ready = Shared::try_new(AtomicBool::new(false)).expect("alloc ready flag");
        let pair2 = pair.clone();
        let ready2 = ready.clone();
        let waiter = thread::spawn(move || {
            let (m, c) = &*pair2;
            let mut g = m.lock();
            // Signal readiness before the first `c.wait(g)` so the notifier
            // in round 0 can synchronise and guarantee this line executes.
            ready2.store(true, O::Release);
            while !*g {
                g = c.wait(g);
            }
        })
        .expect("waiter spawn");
        let (m, c) = &*pair;
        if round == 0 {
            // Synchronised round: wait until the waiter is definitely inside
            // the while body before notifying, guaranteeing L114 executes.
            while !ready.load(O::Acquire) {
                core::hint::spin_loop();
            }
        }
        // Notify — in later rounds this may fire before the waiter reaches wait
        // (the missed-wake window), but the sequence counter prevents deadlock.
        *m.lock() = true;
        c.notify_all();
        // Must terminate; if the window were open this would hang.
        let () = waiter.join();
    }
    testrt::check(true, "missed-wake window test completed all 20 rounds");
});

#[cfg(target_os = "linux")]
arch_test!(condvar_wait_loops_when_predicate_spuriously_false, {
    // Forces the while-loop body (L113-114 of condvar_tests.rs) to iterate
    // at least twice: the waiter calls `c.wait(g)` even though the condition
    // may already be true from the previous iteration (the spin-poll variant).
    //
    // We use a counter predicate: the waiter loops until the count reaches 3,
    // notifying between increments, so the `while !*g` body executes multiple
    // times for the same waiter thread.

    let pair = Shared::try_new((Mutex::new(0u32), Condvar::new())).expect("alloc pair");
    let pair2 = pair.clone();
    let target: u32 = 3;

    let waiter = thread::spawn(move || {
        let (m, c) = &*pair2;
        let mut g = m.lock();
        // Waits until the count reaches `target`; the while body executes
        // multiple times, exercising the loop-back branch.
        while *g < target {
            g = c.wait(g);
        }
        *g
    })
    .expect("waiter spawn");

    // Increment the count step by step, notifying after each step.
    let (m, c) = &*pair;
    for step in 1..=target {
        spin_ms(10);
        *m.lock() = step;
        c.notify_one();
    }

    let result = waiter.join();
    testrt::check_eq(result, target);
});
