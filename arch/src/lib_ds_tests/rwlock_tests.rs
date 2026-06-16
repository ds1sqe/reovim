//! Tests for `sync/rwlock.rs`, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: declared in `sync/mod.rs` as
//! `#[cfg(feature = "selftest")] mod rwlock_tests;`.
//!
//! Type substitutions from the original libtest-based tests:
//! - `std::sync::Arc` → `reovim_lib_ds::Shared`
//! - `std::thread::spawn` / `h.join().unwrap()` → `crate::thread::spawn` / `h.join()`
//! - `std::thread::sleep(Duration::from_millis(N))` → bounded spin on `crate::time::Instant`
//!
//! `reader_count()` and `READERS` are now gated
//! `#[cfg(feature = "selftest")]` in `rwlock.rs`, making them
//! accessible here.

use crate::{arch_test, testrt};
// Only the spawn-dependent (Linux-gated) cases consume these.
#[cfg(target_os = "linux")]
use {
    crate::{thread, time::Instant},
    core::sync::atomic::{AtomicBool, AtomicU32 as TU32, Ordering as O},
    reovim_lib_ds::{Shared, testhooks},
};

use reovim_lib_ds::RwLock;

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

arch_test!(rwlock_read_and_write_single_thread, {
    let l = RwLock::new(10u32);
    testrt::check_eq(*l.read(), 10);
    {
        let mut w = l.write();
        *w += 5;
    }
    testrt::check_eq(*l.read(), 15);
});

// Spawn-dependent cases: they need a thread floor, which freestanding
// targets do not realize. They stay in every hosted suite.
#[cfg(target_os = "linux")]
arch_test!(rwlock_concurrent_readers_overlap, {
    // Two readers must hold simultaneously: each bumps an overlap counter
    // on entry, asserts the peak reached 2, decrements on exit.
    let l = Shared::try_new(RwLock::new(0u32)).expect("alloc shared rwlock");
    let overlap = Shared::try_new(TU32::new(0)).expect("alloc overlap counter");
    let peak = Shared::try_new(TU32::new(0)).expect("alloc peak counter");

    macro_rules! spawn_reader {
        ($l:expr, $overlap:expr, $peak:expr) => {{
            let l2 = $l.clone();
            let overlap2 = $overlap.clone();
            let peak2 = $peak.clone();
            thread::spawn(move || {
                let g = l2.read();
                let now = overlap2.fetch_add(1, O::SeqCst) + 1;
                peak2.fetch_max(now, O::SeqCst);
                // Hold long enough for the sibling reader to overlap.
                spin_ms(30);
                assert!(*g == 0);
                overlap2.fetch_sub(1, O::SeqCst);
            })
            .expect("reader spawn")
        }};
    }

    let h0 = spawn_reader!(l, overlap, peak);
    let h1 = spawn_reader!(l, overlap, peak);
    let () = h0.join();
    let () = h1.join();

    testrt::check_eq(peak.load(O::SeqCst), 2u32);
    testrt::check_eq(l.reader_count(), 0u32);
});

#[cfg(target_os = "linux")]
arch_test!(rwlock_writer_excludes_readers, {
    let l = Shared::try_new(RwLock::new(0u32)).expect("alloc shared rwlock");
    let reader_saw = Shared::try_new(TU32::new(0)).expect("alloc reader_saw");
    let writer_done = Shared::try_new(AtomicBool::new(false)).expect("alloc writer_done");

    let w = l.write();

    let l2 = l.clone();
    let saw2 = reader_saw.clone();
    let done2 = writer_done.clone();
    let reader = thread::spawn(move || {
        // This read() must block until the writer releases.
        let g = l2.read();
        // By the time the reader proceeds, the writer must have finished.
        assert!(done2.load(O::SeqCst), "reader observed writer's release");
        saw2.store(*g, O::SeqCst);
    })
    .expect("reader spawn");

    spin_ms(40);
    {
        let mut w = w;
        *w = 99;
        writer_done.store(true, O::SeqCst);
    }
    let () = reader.join();
    testrt::check_eq(reader_saw.load(O::SeqCst), 99u32);
});

#[cfg(target_os = "linux")]
arch_test!(rwlock_readers_exclude_writer, {
    let l = Shared::try_new(RwLock::new(0u32)).expect("alloc shared rwlock");
    let writer_ran = Shared::try_new(AtomicBool::new(false)).expect("alloc writer_ran");

    let r = l.read();

    let l2 = l.clone();
    let ran2 = writer_ran.clone();
    let writer = thread::spawn(move || {
        let mut g = l2.write();
        *g = 1;
        ran2.store(true, O::SeqCst);
    })
    .expect("writer spawn");

    // Hold the read lock briefly; the writer must not have run yet.
    spin_ms(40);
    testrt::check(!writer_ran.load(O::SeqCst), "writer blocked behind reader");
    drop(r);
    let () = writer.join();
    testrt::check(writer_ran.load(O::SeqCst), "writer ran after reader dropped");
    testrt::check_eq(*l.read(), 1u32);
});

#[cfg(target_os = "linux")]
arch_test!(rwlock_writer_blocks_reader_past_spin_limit_exercises_futex_wait, {
    // Forces the `FUTEX_WAIT` path in `read()` deterministically: the writer
    // holds the lock until it OBSERVES the reader's wait counter increment
    // (proving the reader actually issued `FUTEX_WAIT`), then releases. No
    // reliance on a timing window — the counter is the synchronization point,
    // mirroring the mutex forced-slow-path test.
    let reader_waits_before = testhooks::rwlock_reader_waits();

    let l = Shared::try_new(RwLock::new(0u32)).expect("alloc shared rwlock");
    let l2 = l.clone();
    let reader_done = Shared::try_new(AtomicBool::new(false)).expect("alloc reader_done");
    let reader_done2 = reader_done.clone();

    // Acquire the write lock in the main thread.
    let mut g = l.write();

    // Spawn a reader: it spins past SPIN_LIMIT and enters FUTEX_WAIT because
    // the writer holds the lock.
    let reader = thread::spawn(move || {
        let v = *l2.read();
        reader_done2.store(true, O::SeqCst);
        v
    })
    .expect("reader spawn");

    // Hold the write lock until the reader's FUTEX_WAIT counter advances: this
    // proves the reader exhausted its spin budget and slept on the word.
    while testhooks::rwlock_reader_waits() == reader_waits_before {
        core::hint::spin_loop();
    }

    // Mutate and release; write_unlock()'s FUTEX_WAKE wakes the blocked reader.
    *g = 42;
    drop(g);

    let val = reader.join();
    testrt::check_eq(val, 42u32);
    testrt::check(reader_done.load(O::SeqCst), "reader completed after writer released");
    testrt::check(
        testhooks::rwlock_reader_waits() > reader_waits_before,
        "the reader must have entered FUTEX_WAIT",
    );
});

#[cfg(target_os = "linux")]
arch_test!(rwlock_concurrent_readers_cas_retry_exercises_nowriter_retry_path, {
    // Forces the "CAS lost a race; retry without sleeping" arm of `read()`
    // (rwlock.rs lines 117-120): fires when `WRITER == 0` (no writer holds)
    // but `compare_exchange_weak(s, s+1, ..)` fails because another concurrent
    // reader incremented the count between the `load` and the `CAS`. Many
    // readers hammering the same lock in tight loops makes this collision
    // statistically certain; the `RWLOCK_READER_CAS_RETRY` testhook counter
    // is the deterministic proof that the retry arm executed.
    let retry_before = testhooks::rwlock_reader_cas_retry();

    let l = Shared::try_new(RwLock::new(0u32)).expect("alloc shared rwlock");
    let done = Shared::try_new(TU32::new(0)).expect("alloc done counter");

    // Each worker takes many short read critical sections so siblings collide
    // on the `s -> s+1` CAS while no writer is present.
    const ITERS: u32 = 2_000;

    macro_rules! spawn_reader_hammer {
        ($l:expr, $done:expr) => {{
            let l2 = $l.clone();
            let d2 = $done.clone();
            thread::spawn(move || {
                for _ in 0..ITERS {
                    let _g = l2.read();
                    // Short critical section: drop guard immediately so the
                    // reader count returns to 0, maximising CAS collision
                    // probability for the next iteration.
                }
                d2.fetch_add(1, O::SeqCst);
            })
            .expect("reader hammer spawn")
        }};
    }

    let h0 = spawn_reader_hammer!(l, done);
    let h1 = spawn_reader_hammer!(l, done);
    let h2 = spawn_reader_hammer!(l, done);
    let h3 = spawn_reader_hammer!(l, done);
    let h4 = spawn_reader_hammer!(l, done);
    let h5 = spawn_reader_hammer!(l, done);
    let () = h0.join();
    let () = h1.join();
    let () = h2.join();
    let () = h3.join();
    let () = h4.join();
    let () = h5.join();

    testrt::check_eq(done.load(O::SeqCst), 6u32);
    testrt::check(
        testhooks::rwlock_reader_cas_retry() > retry_before,
        "the reader CAS-retry arm (rwlock.rs L117-120) must have fired under contention",
    );
});

#[cfg(target_os = "linux")]
arch_test!(rwlock_multiple_writers_contend_exercises_lost_cas_spin, {
    // Forces the `s == 0` lost-CAS-race spin arm of `write()`: that arm fires
    // when a writer's `compare_exchange_weak(0, WRITER, ..)` fails while the
    // word reads `0` — either a genuine lost race against a sibling writer that
    // grabbed-and-released between the CAS and the reload, or a spurious weak-CAS
    // failure. Several writers each hammer the write lock in a loop so the arm
    // is taken; the lost-CAS counter is the deterministic proof, and the loop
    // budget makes a clean run (zero contention) astronomically unlikely.
    let lost_before = testhooks::rwlock_writer_lost_cas();

    let l = Shared::try_new(RwLock::new(0u32)).expect("alloc shared rwlock");
    let done = Shared::try_new(TU32::new(0)).expect("alloc done counter");

    // Each worker performs many short write critical sections so siblings keep
    // colliding on the `0 -> WRITER` CAS.
    const ITERS: u32 = 2_000;

    macro_rules! spawn_writer {
        ($l:expr, $done:expr) => {{
            let l2 = $l.clone();
            let d2 = $done.clone();
            thread::spawn(move || {
                for _ in 0..ITERS {
                    *l2.write() += 1;
                }
                d2.fetch_add(1, O::SeqCst);
            })
            .expect("writer spawn")
        }};
    }

    let h0 = spawn_writer!(l, done);
    let h1 = spawn_writer!(l, done);
    let h2 = spawn_writer!(l, done);
    let h3 = spawn_writer!(l, done);
    let () = h0.join();
    let () = h1.join();
    let () = h2.join();
    let () = h3.join();

    testrt::check_eq(*l.read(), 4 * ITERS);
    testrt::check_eq(done.load(O::SeqCst), 4u32);
    testrt::check(
        testhooks::rwlock_writer_lost_cas() > lost_before,
        "the writer lost-CAS spin arm must have executed under contention",
    );
});
