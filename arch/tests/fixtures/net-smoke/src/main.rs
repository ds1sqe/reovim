//! Integration smoke fixture (#797 Phase 1).
//!
//! **Acceptance smoke**: binds a Unix-domain listener, spawns an accept thread,
//! connects from the main thread, echoes a 16-byte frame-sized buffer
//! round-trip, and exits 0. This proves the blocking thread-per-connection
//! carrier primitive composes end-to-end on the real floor.
//!
//! Exit codes:
//! - `0` — all steps completed successfully.
//! - `1` — `UnixListener::bind` failed.
//! - `2` — `spawn` (accept thread) failed.
//! - `3` — `UnixStream::connect` (main thread client) failed.
//! - `4` — client `write_all` failed.
//! - `5` — client `read_exact` failed.
//! - `6` — round-trip bytes did not match.
//! - `7` — accept thread returned an error.
#![no_std]
#![no_main]
// `entry!` uses `#[unsafe(no_mangle)]`; allow is scoped to this fixture bin.
#![allow(unsafe_code)]

use reovim_arch::{
    net::{UnixListener, UnixStream},
    thread::spawn,
};

reovim_arch::entry!(|_argc, _argv, _envp| {
    // Use a fixed path. The socket is unlinked by `UnixListener::Drop`.
    let path: &[u8] = b"/tmp/reovim-arch-net-smoke-fixture\0";

    // 1. Bind the listener.
    let listener = match UnixListener::bind(path) {
        Ok(l) => l,
        Err(_) => return 1,
    };

    // 2. Spawn the accept-side thread.
    let handle = match spawn::<_, i32>(move || {
        let stream = match listener.accept() {
            Ok(s) => s,
            Err(_) => return 7,
        };
        let mut buf = [0u8; 16];
        if stream.read_exact(&mut buf).is_err() {
            return 7;
        }
        if stream.write_all(&buf).is_err() {
            return 7;
        }
        0
    }) {
        Ok(h) => h,
        Err(_) => return 2,
    };

    // 3. Connect from the main thread.
    let client = match UnixStream::connect(path) {
        Ok(c) => c,
        Err(_) => return 3,
    };

    // 4. Write a 16-byte payload.
    let payload: &[u8; 16] = b"reovim-net-smoke";
    if client.write_all(payload).is_err() {
        return 4;
    }

    // 5. Read it back.
    let mut echo = [0u8; 16];
    if client.read_exact(&mut echo).is_err() {
        return 5;
    }

    // 6. Assert byte-for-byte match.
    if &echo != payload {
        return 6;
    }

    // 7. Join the accept thread.
    let accept_code = handle.join();
    accept_code
});
