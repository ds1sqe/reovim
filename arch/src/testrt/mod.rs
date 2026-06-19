//! The `arch` facade over the core-only `reovim-testrt` runtime (#785 Phase 5,
//! SP07).
//!
//! The libtest-free test-collection + reporting mechanism itself lives in the
//! `reovim-testrt` leaf — it is pure `#![no_std]` `core` code (the `arch_test!`
//! macro, the `__start_/__stop_reovim_arch_tests` section walk, the fail-fast
//! reporter, `check`/`check_eq`). This module is the thin arch-side facade that
//! re-exports that surface and wires the two injected edges onto arch's `sys`
//! floor, so every existing consumer (`reovim_arch::testrt::*` and
//! `reovim_arch::arch_test!`) compiles unchanged:
//!
//! - [`run`] (0-arg) drives the leaf's reporter with an arch fd-1 output sink;
//! - [`unique_path`] (2-arg) supplies the leaf with `sys::gettid` for the
//!   TID-suffixed temp path.
//!
//! See the leaf's module docs for the registration model, exit-code contract,
//! and the fail-fast failure model.

// The pure surface lives in the leaf; re-export it so `reovim_arch::testrt::*`
// keeps resolving for every external consumer (tui/kernel/server/e2e + arch's
// own net tests + the panic handler's `current_test()` read).
pub use reovim_testrt::{TestCase, check, check_eq, current_test};

/// Writes all of `buf` to fd 1, looping over short writes; stops on error.
///
/// The arch-side output edge injected into [`reovim_testrt::run`]: the leaf
/// renders progress/summary bytes and hands each chunk here, which drives them
/// through arch's `sys::write` floor.
fn arch_stdout_sink(buf: &[u8]) {
    let mut off = 0;
    while off < buf.len() {
        match crate::sys::write(1, &buf[off..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => off += n,
        }
    }
}

/// Runs every registered test, reporting progress to fd 1, and returns the
/// process exit code: `0` when all pass.
///
/// The 0-arg arch facade over [`reovim_testrt::run`]: it supplies the fd-1
/// output sink so the selftest bin's `entry!` body calls `testrt::run()`
/// unchanged. On the fail-fast path a failing test panics before this returns;
/// the panic handler terminates with its disposition code, so this returns only
/// on a full pass (`0`).
///
/// ```ignore
/// // run() requires the selftest runner binary with registered TestCase
/// // statics in the reovim_arch_tests link section — not the doctest harness.
/// let code = reovim_arch::testrt::run();
/// assert_eq!(code, 0);
/// ```
#[must_use]
pub fn run() -> i32 {
    reovim_testrt::run(arch_stdout_sink)
}

/// Fills `buf` with `prefix`, the calling thread's TID in decimal, and a
/// terminating NUL, returning the filled slice (NUL included).
///
/// The 2-arg arch facade over [`reovim_testrt::unique_path`]: it supplies the
/// calling thread's TID (`sys::gettid`) so consumers keep the original
/// `unique_path(prefix, buf)` call shape. A TID-suffixed path is unique per
/// running test process, which avoids `/tmp` collisions across the concurrent
/// selftest binaries one `cargo test` invocation launches.
///
/// # Panics
///
/// Panics when `buf` is too small for `prefix` + digits + NUL.
///
/// ```ignore
/// // unique_path() reads the calling thread's TID via sys::gettid — meaningful
/// // only inside a running selftest binary, not the doctest harness.
/// let mut buf = [0u8; 64];
/// let path = reovim_arch::testrt::unique_path(b"/tmp/reovim-", &mut buf);
/// assert_eq!(*path.last().unwrap(), 0);
/// ```
pub fn unique_path<'a>(prefix: &[u8], buf: &'a mut [u8; 64]) -> &'a [u8] {
    reovim_testrt::unique_path(prefix, buf, crate::sys::gettid().unsigned_abs() as usize)
}
