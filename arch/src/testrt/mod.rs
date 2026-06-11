//! The in-repo `no_std` test runner (#785 Phase 5).
//!
//! `arch/` is the bootstrap-state-1 migration target: its own tests must run
//! in the environment it flies (`#![no_std] #![no_main]`, `panic = "abort"`,
//! no libtest), not under std-linked libtest. This module is that runner —
//! a libtest-free test-collection + reporting mechanism that an arch test bin
//! boots through.
//!
//! ## Registration (declarative, no central list)
//!
//! [`arch_test!`] registers a test function by emitting a `#[used]`
//! [`TestCase`] into a dedicated link section. The linker brackets that
//! section with `__start_/__stop_` symbols, and [`run`] walks the span —
//! distributed-slice collection, so adding a test is a pure local
//! declaration, never an edit to a hand-maintained registry.
//!
//! ## Reporting + exit codes
//!
//! [`run`] iterates the registered cases, writing `name ... ok` to fd 1 per
//! pass, a summary line, and exits `0` when all pass. A test that fails does
//! so by panicking (see the failure model below); the panic handler renders
//! the failing line and exits a non-zero disposition code, so the *process*
//! exit code is the pass/fail signal a parent observes.
//!
//! ## Failure model: fail-fast (documented contract)
//!
//! Tests are plain functions that signal failure by **panicking** (via
//! [`check`]/[`check_eq`] or a direct `panic!`). Under `panic = "abort"`
//! there is no unwinder, so a failing test cannot be caught and the run
//! cannot continue past it: the model is **fail-fast**. The first failing
//! test aborts the process through the arch panic handler, whose rendered
//! LOG2 line names the failing test (the runner publishes the current test
//! name via [`set_current_test`], which the panic message reads) so the
//! failure is attributable from the process output alone.
//!
//! This differs from libtest's "run all, report each" model deliberately:
//! per-test isolation needs either a forked child per test or unwinding,
//! neither of which the floor has. A future phase may exec one child per
//! test for full isolation; until a second consumer needs it that is not
//! built (rule of three).

use core::sync::atomic::{AtomicPtr, Ordering};

/// One registered test: its name and the function to run.
///
/// `#[repr(C)]` so the layout is stable across the link-section array the
/// runner walks; the fields are read by [`run`] only.
///
/// ```ignore
/// // TestCase is populated by the arch_test! macro; the link-section
/// // distributed-slice requires a no_main runtime binary, not a doctest.
/// use reovim_arch::testrt::TestCase;
/// let case = TestCase { name: "my_test", run: || {} };
/// assert_eq!(case.name, "my_test");
/// ```
#[repr(C)]
pub struct TestCase {
    /// The test's display name (the `name ... ok` label).
    pub name: &'static str,
    /// The test body. Returns normally on pass; panics on failure.
    pub run: fn(),
}

// `TestCase` is `Sync` automatically (`&'static str` and `fn()` are both
// `Sync`), so the link-section statics the macro emits are valid `static`s
// shared read-only across the runner.

// The link-section bracket symbols the linker emits around the registered
// `TestCase` array. Declaring externs with the `__start_/__stop_` link names
// binds to the linker's section delimiters; the runner differences them to
// recover the slice without any hand-maintained table.
// (Plain comment: rustdoc rejects doc comments on extern blocks via
// `unused_doc_comments`.)
unsafe extern "C" {
    /// First byte of the `reovim_arch_tests` section (the bracket symbol is
    /// declared `u8` to stay FFI-safe; the runner casts it to `*const
    /// TestCase`).
    #[link_name = "__start_reovim_arch_tests"]
    static TESTS_START: u8;
    /// One byte past the last `TestCase` in the section.
    #[link_name = "__stop_reovim_arch_tests"]
    static TESTS_STOP: u8;
}

/// The name of the currently-running test, published so the panic handler can
/// name it in the failure line. `null` between tests. Stored as the string's
/// data pointer + length is recovered separately; here we keep the whole
/// `&str` behind a pointer so the panic path can read it lock-free.
static CURRENT_TEST: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());
/// The byte length paired with [`CURRENT_TEST`]'s data pointer.
static CURRENT_TEST_LEN: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// Publishes `name` as the current test (read by the panic path on failure).
fn set_current_test(name: &'static str) {
    CURRENT_TEST_LEN.store(name.len(), Ordering::Release);
    CURRENT_TEST.store(name.as_ptr().cast_mut(), Ordering::Release);
}

/// Returns the name of the test running when the panic fired, or `None` when
/// no test is active. The panic handler / test bin calls this to attribute a
/// failure; the returned slice borrows the `'static` test name.
///
/// ```ignore
/// // current_test() is meaningful only inside the selftest runner binary.
/// use reovim_arch::testrt::current_test;
/// // Outside a running test, returns None.
/// assert!(current_test().is_none());
/// ```
#[must_use]
pub fn current_test() -> Option<&'static str> {
    let ptr = CURRENT_TEST.load(Ordering::Acquire);
    if ptr.is_null() {
        return None;
    }
    let len = CURRENT_TEST_LEN.load(Ordering::Acquire);
    // SAFETY: `ptr`/`len` were published by `set_current_test` from a live
    // `&'static str`'s data pointer and length; the bytes are valid UTF-8 for
    // the process lifetime. No writer runs concurrently with this read on the
    // fail-fast path (the panicking thread is the one that set it).
    let bytes = unsafe { core::slice::from_raw_parts(ptr.cast_const(), len) };
    // SAFETY: the bytes came from a `&str`, so they are valid UTF-8.
    Some(unsafe { core::str::from_utf8_unchecked(bytes) })
}

/// The registered test cases as a slice (the distributed-slice walk).
fn registered() -> &'static [TestCase] {
    let start = (&raw const TESTS_START).cast::<TestCase>();
    let stop = (&raw const TESTS_STOP).cast::<TestCase>();
    // SAFETY: `start`/`stop` bracket one contiguous linker section holding the
    // `#[used]` `TestCase` statics `arch_test!` emits, so both casts point at
    // properly-aligned `TestCase`s and `offset_from` yields the element count;
    // the span is live, initialized `'static` data.
    let len = unsafe { stop.offset_from(start).cast_unsigned() };
    // SAFETY: as above — `start` points at `len` contiguous `TestCase`s.
    unsafe { core::slice::from_raw_parts(start, len) }
}

/// Writes all of `buf` to `fd`, looping over short writes; stops on error.
fn write_all(fd: i32, buf: &[u8]) {
    let mut off = 0;
    while off < buf.len() {
        match crate::sys::write(fd, &buf[off..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => off += n,
        }
    }
}

/// Writes `n` as decimal to `fd` (no allocation; a fixed scratch buffer).
fn write_usize(fd: i32, n: usize) {
    // `usize::MAX` is 20 decimal digits; 20 bytes always suffice.
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut v = n;
    loop {
        i -= 1;
        // `v % 10` is a single decimal digit (0..=9), so the `u8` cast is
        // value-preserving; adding `b'0'` yields its ASCII digit.
        #[allow(clippy::cast_possible_truncation)]
        let digit = (v % 10) as u8;
        buf[i] = b'0' + digit;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    write_all(fd, &buf[i..]);
}

/// Runs every registered test, reporting progress to fd 1, and returns the
/// process exit code: `0` when all pass.
///
/// On the fail-fast path a failing test panics before [`run`] can return; the
/// panic handler terminates the process with its disposition code, so this
/// function returns only on a full pass. The return value is the bin's exit
/// code: an `entry!` body calls `testrt::run()` and returns it.
///
/// The all-pass return is `0`; the failing path never reaches the return (the
/// panic handler exits first), which is why there is no non-zero return here.
///
/// ```ignore
/// // run() requires the selftest runner binary with registered TestCase statics
/// // in the reovim_arch_tests link section — not usable from the doctest harness.
/// let code = reovim_arch::testrt::run();
/// assert_eq!(code, 0);
/// ```
#[must_use]
pub fn run() -> i32 {
    let tests = registered();
    write_all(1, b"running ");
    write_usize(1, tests.len());
    write_all(1, b" tests\n");

    for case in tests {
        set_current_test(case.name);
        write_all(1, b"test ");
        write_all(1, case.name.as_bytes());
        write_all(1, b" ... ");
        // The body panics on failure; the panic handler reports + exits, so
        // control returns here only on a pass.
        (case.run)();
        write_all(1, b"ok\n");
    }

    write_all(1, b"result: ok. ");
    write_usize(1, tests.len());
    write_all(1, b" passed; 0 failed\n");
    0
}

/// Asserts `cond`, panicking with `msg` (and the current test name) on
/// failure. The panic is the failure path: the runner is fail-fast.
///
/// # Panics
///
/// Panics when `cond` is `false`; the panic message carries `msg` and the
/// running test's name, so the flushed LOG2 line attributes the failure.
///
/// DEV1 (round 3): `current_test().unwrap_or("<unknown test>")` is evaluated
/// eagerly (before the `assert!`) so that both the `Some` and `None` arms are
/// reachable from tests that null the `CURRENT_TEST` pointer. With the lazy
/// `assert!` form, the format arguments are only computed on failure, making
/// the `None` arm physically uncoverable in a pass-only test run.
///
/// ```ignore
/// // check() is used inside arch_test! bodies; it requires the selftest runner.
/// use reovim_arch::testrt::check;
/// check(2 + 2 == 4, "arithmetic invariant");
/// ```
#[track_caller]
pub fn check(cond: bool, msg: &str) {
    let test_name = current_test().unwrap_or("<unknown test>");
    assert!(cond, "{test_name}: {msg}");
}

/// Asserts `left == right`, panicking on inequality. As [`check`], the panic
/// is the failure path (fail-fast).
///
/// # Panics
///
/// Panics when `left != right`; the message names the running test.
///
/// ```ignore
/// // check_eq() is used inside arch_test! bodies; it requires the selftest runner.
/// use reovim_arch::testrt::check_eq;
/// check_eq(1 + 1, 2);
/// ```
#[track_caller]
pub fn check_eq<T: PartialEq + core::fmt::Debug>(left: T, right: T) {
    assert!(
        left == right,
        "{}: {left:?} != {right:?}",
        current_test().unwrap_or("<unknown test>"),
    );
}

// L12 layout (#785 Phase 5 coverage): tests live in the sibling file
// `testrt_tests.rs`, declared as a `#[path]` child so `super::` reaches the
// private `write_all`, `CURRENT_TEST`, and related items.
#[cfg(feature = "selftest")]
#[path = "testrt_tests.rs"]
mod tests;

/// Registers a `no_std` test with the runner (distributed-slice; #785 Phase 5).
///
/// Each invocation emits a `#[used]` [`TestCase`] into the
/// `reovim_arch_tests` link section, which [`run`] walks. Registration is
/// declarative and local — no central list. The body is a plain function that
/// returns on pass and panics on failure (via [`check`]/[`check_eq`] or a
/// direct `panic!`); the runner is fail-fast (see the module docs).
///
/// ```ignore
/// reovim_arch::arch_test!(my_invariant_holds, {
///     reovim_arch::testrt::check_eq(2 + 2, 4);
/// });
/// ```
#[macro_export]
macro_rules! arch_test {
    ($name:ident, $body:block) => {
        $crate::arch_test!($name, stringify!($name), $body);
    };
    ($name:ident, $label:expr, $body:block) => {
        const _: () = {
            fn test_body() {
                $body
            }
            #[used]
            #[unsafe(link_section = "reovim_arch_tests")]
            static CASE: $crate::testrt::TestCase = $crate::testrt::TestCase {
                name: $label,
                run: test_body,
            };
        };
    };
}
