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

use super::{HANDLE, InstallError, PlatformVtable, handle, install};

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

/// The synthetic provider's `static` vtable (zero heap to build), the shape an
/// arch-side `static` takes.
static FIXTURE_VTABLE: PlatformVtable = PlatformVtable {
    clock: fixture_clock,
    alloc: fixture_alloc,
    dealloc: fixture_dealloc,
    park: fixture_park,
    unpark: fixture_unpark,
    unpark_all: fixture_unpark_all,
};

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
}
