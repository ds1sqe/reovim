//! Tests for the platform handle install/read contract.
//!
//! ## One-test lifecycle (process-global write-once)
//!
//! `HANDLE` is a process-global, write-once static (AB12). Ordinary
//! `cargo test` runs the tests of one crate as threads in a single process, so
//! the install can happen exactly once across the whole run. The full
//! lifecycle — install, read-through, reject-second-install — is therefore
//! exercised in a single `#[test]` rather than split across functions that
//! would race over the one-shot static. A test fixture provides the vtable; no
//! arch implementor is named (the contract has no impl-side edge).

use super::{HANDLE, InstallError, NetError, PlatformVtable, handle, install};

// ── fixture primitives (a synthetic provider, no arch edge) ──────────────────

/// A clock that returns a fixed, non-zero, sane nanosecond value. The test
/// reads it THROUGH the installed handle to prove the dispatch path, not to
/// assert a real time source (kabi has no clock of its own — it is the
/// contract, not an implementor).
unsafe extern "C" fn fixture_clock() -> i64 {
    1_234_567_890
}

unsafe extern "C" fn fixture_alloc(_size: usize, _align: usize) -> *mut u8 {
    core::ptr::null_mut()
}

unsafe extern "C" fn fixture_dealloc(_ptr: *mut u8, _size: usize, _align: usize) {}

unsafe extern "C" fn fixture_park(_word: *const u32, _expected: u32) {}

unsafe extern "C" fn fixture_unpark(_word: *const u32) {}

unsafe extern "C" fn fixture_unpark_all(_word: *const u32) {}

unsafe extern "C" fn fixture_unix_connect(_path: *const u8, _len: usize) -> i64 {
    // A synthetic provider: report a fixed fd so the dispatch path is proven.
    7
}

unsafe extern "C" fn fixture_unix_listen(_path: *const u8, _len: usize) -> i64 {
    8
}

unsafe extern "C" fn fixture_unix_accept(_fd: i32) -> i64 {
    9
}

unsafe extern "C" fn fixture_fd_read(_fd: i32, _buf: *mut u8, _len: usize) -> i64 {
    // End-of-stream: zero bytes.
    0
}

unsafe extern "C" fn fixture_fd_write(_fd: i32, _buf: *const u8, len: usize) -> i64 {
    // A full write of `len` bytes (the count narrows back to i64).
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    {
        len as i64
    }
}

unsafe extern "C" fn fixture_fd_close(_fd: i32) -> i64 {
    0
}

unsafe extern "C" fn fixture_thread_spawn(
    _entry: unsafe extern "C" fn(*mut u8),
    _arg: *mut u8,
) -> i64 {
    // A synthetic provider returns a fixed tid without starting a real thread.
    42
}

unsafe extern "C" fn fixture_realtime() -> i64 {
    // A fixed, sane wall-clock nanos value (well after the Unix epoch), read
    // through the handle to prove the dispatch path.
    1_700_000_000_000_000_000
}

unsafe extern "C" fn fixture_file_open(
    _path: *const u8,
    _path_len: usize,
    flags: i32,
    _mode: u32,
) -> i64 {
    // A synthetic provider: a non-negative `flags` "opens" successfully and
    // returns a fixed fd; a negative `flags` reports `-ENOENT` (errno 2) so the
    // error branch of the safe wrapper is exercised through the handle.
    if flags < 0 { -2 } else { 11 }
}

unsafe extern "C" fn fixture_thread_id() -> i64 {
    // A synthetic provider returns a fixed tid.
    1234
}

#[allow(clippy::cast_possible_wrap)]
unsafe extern "C" fn fixture_file_write(_fd: i32, _buf: *const u8, len: usize) -> i64 {
    // A synthetic provider "writes" the whole buffer: echo the byte count so the
    // plain-write dispatch path (distinct from the socket `fd_write`) is proven.
    // A test buffer's byte count never exceeds `isize::MAX`, so the cast cannot
    // wrap.
    len as i64
}

/// The synthetic provider's `static` vtable (zero heap to build), the shape an
/// arch-side `static` takes.
static FIXTURE_VTABLE: PlatformVtable = PlatformVtable {
    clock: fixture_clock,
    alloc: fixture_alloc,
    dealloc: fixture_dealloc,
    park: fixture_park,
    unpark: fixture_unpark,
    unpark_all: fixture_unpark_all,
    unix_connect: fixture_unix_connect,
    unix_listen: fixture_unix_listen,
    unix_accept: fixture_unix_accept,
    fd_read: fixture_fd_read,
    fd_write: fixture_fd_write,
    fd_close: fixture_fd_close,
    thread_spawn: fixture_thread_spawn,
    realtime: fixture_realtime,
    file_open: fixture_file_open,
    thread_id: fixture_thread_id,
    file_write: fixture_file_write,
};

/// A no-op C-ABI thread entry for the `thread_spawn` fixture: the synthetic
/// provider never invokes it, so the body is unreachable and trivially sound.
unsafe extern "C" fn noop_entry(_arg: *mut u8) {}

#[test]
fn install_is_write_once_and_handle_reads_through() {
    // Pre-install the handle is unset (the null sentinel).
    assert!(
        HANDLE.load(core::sync::atomic::Ordering::Acquire).is_null(),
        "handle must be unset before the first install",
    );

    // First install wins (AB12).
    assert_eq!(install(&FIXTURE_VTABLE), Ok(()));

    // Read the clock THROUGH the installed handle: handle → fn pointer →
    // primitive. The value flows end-to-end, not via a bypass.
    // SAFETY: the handle is installed; the fixture clock has no precondition.
    let nanos = unsafe { (handle().clock)() };
    assert_eq!(nanos, 1_234_567_890, "clock value must flow through the handle");
    assert!(nanos > 0, "a sane monotonic reading is positive");

    // Second install is rejected; the first table stands (write-once).
    assert_eq!(
        install(&FIXTURE_VTABLE),
        Err(InstallError::AlreadyInstalled),
        "a second install must be rejected (AB12 write-once)",
    );

    // The handle still resolves to the first-installed table.
    // SAFETY: as above — the handle remains installed.
    let nanos_again = unsafe { (handle().clock)() };
    assert_eq!(nanos_again, 1_234_567_890, "the first table still stands");

    // The SP05 net/thread safe wrappers dispatch through the handle too: the
    // synthetic provider's fixed returns flow back as typed results, proving
    // the new slots are wired and the negative-errno mapping is correct.
    assert_eq!(handle().unix_connect(b"/x\0"), Ok(7), "connect fd flows through");
    assert_eq!(handle().unix_listen(b"/x\0"), Ok(8), "listen fd flows through");
    assert_eq!(handle().unix_accept(8), Ok(9), "accept fd flows through");
    let mut buf = [0u8; 4];
    assert_eq!(handle().fd_read(9, &mut buf), Ok(0), "read EOF flows through");
    assert_eq!(handle().fd_write(9, b"abcd"), Ok(4), "write count flows through");
    assert_eq!(handle().fd_close(9), Ok(()), "close ok flows through");

    // thread_spawn is unsafe (the caller owns entry/arg validity); the fixture
    // never touches `entry`/`arg`, so a null arg + no-op entry is sound here.
    // SAFETY: the fixture spawn ignores `entry`/`arg`, so a no-op entry and a
    // null arg uphold the contract trivially (nothing dereferences them).
    let tid = unsafe { handle().thread_spawn(noop_entry, core::ptr::null_mut()) };
    assert_eq!(tid, Ok(42), "thread tid flows through");

    // The SP05 time + sys safe wrappers dispatch through the handle too.
    assert_eq!(handle().realtime(), 1_700_000_000_000_000_000, "wall-clock nanos flow through");
    assert_eq!(handle().thread_id(), 1234, "thread id flows through");
    assert_eq!(handle().clock(), 1_234_567_890, "monotonic nanos flow through the safe wrapper");

    // file_open SUCCESS: a non-negative `flags` "opens" and returns fd 11.
    assert_eq!(handle().file_open(b"/log\0", 1, 0), Ok(11), "file_open fd flows through");
    // file_open ERROR branch: a negative `flags` makes the fixture return
    // `-2` (-ENOENT), which the wrapper maps to the typed positive-errno error.
    assert_eq!(
        handle().file_open(b"/log\0", -1, 0),
        Err(NetError::from_code(2)),
        "file_open negative errno maps to the typed error",
    );

    // file_write (plain `write(2)`, distinct from the socket `fd_write`): the
    // fixture echoes the byte count, proving the separate dispatch path.
    assert_eq!(handle().file_write(11, b"abcd"), Ok(4), "file_write count flows through");
}
