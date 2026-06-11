//! Charter smoke fixture (#785 Phase 4).
//!
//! A `#![no_std] #![no_main]` binary that boots through the arch `_start`,
//! exercises the platform floor end-to-end — allocator + heap DS + threads +
//! sync — and exits `0`. This is the phase's end-to-end proof that the entry
//! path composes the floor; an integration test execs the built binary and
//! asserts status `0`.
#![no_std]
#![no_main]
// The `entry!` macro expands to `#[unsafe(no_mangle)]` (the entry-shim symbol),
// which the `unsafe_code` lint flags; the fixture is unsafe by nature (it
// declares the process entry symbol). The allow is scoped to this fixture bin.
#![allow(unsafe_code)]

use reovim_arch::{
    ds::Seq,
    sync::Mutex,
    thread::spawn,
};

reovim_arch::entry!(|_argc, _argv, _envp| {
    // Allocate an arch DS through the live allocator.
    let mut seq: Seq<u64> = Seq::new();
    for i in 0..8 {
        if seq.try_push(i).is_err() {
            return 1;
        }
    }

    // Spawn a thread that pushes into a Mutex-guarded Seq, then join it —
    // threads + sync + DS + allocator composing under the entry path.
    let shared = match reovim_arch::ds::Shared::try_new(Mutex::new(Seq::<u64>::new())) {
        Ok(s) => s,
        Err(_) => return 2,
    };
    let worker_view = shared.clone();
    let handle = match spawn(move || {
        let mut guard = worker_view.lock();
        for i in 0..16u64 {
            let _ = guard.try_push(i);
        }
    }) {
        Ok(h) => h,
        Err(_) => return 3,
    };
    let () = handle.join();

    let guard = shared.lock();
    if guard.len() != 16 || seq.len() != 8 {
        return 4;
    }
    drop(guard);

    // The boot path touched allocator + DS + thread + sync and reached here:
    // exit success.
    0
});
