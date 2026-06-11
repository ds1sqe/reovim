//! Tests for `profiler.rs`, compiled into the lib under
//! `selftest` + `runtime` + `arch_coverage` (#785 Phase 5).
//!
//! L12 layout: declared inside `profiler.rs` as
//! `#[cfg(feature = "selftest")] #[path = "profiler_tests.rs"] mod tests;`.
//! The outer `profiler` module is already gated on
//! `cfg(all(feature = "runtime", arch_coverage))`, so these tests only
//! compile (and run) when the arch-selftest binary is built under
//! `-C instrument-coverage --cfg arch_coverage`.
//!
//! `super::` reaches the private helpers `write_all`, `write_padding`,
//! `profile_path`, `num_data_entries`, and the section walk `write_profile`.
//! Because the tests run inside the instrumented selftest binary
//! (`-C instrument-coverage --cfg arch_coverage`), the `__llvm_prf_*` sections
//! are live, so `write_profile` can be driven directly to reach its `openat`
//! and `write_all`-chain failure arms.
//!
//! Coverage targets (profiler.rs):
//! - `write_all` success path (all bytes written), error path (Err from
//!   write), and the empty-buffer no-iteration arm.
//! - `write_padding` success path (len > 0) and the loop-exit path (len == 0).
//! - `profile_path` — None (no `LLVM_PROFILE_FILE` entry), Some (entry found),
//!   short-entry skip, and the wrong-prefix skip.
//! - `num_data_entries` — the exact-multiple `Some` arm and the non-multiple
//!   `None` arm (the version-stale guard, otherwise structurally unreachable).
//! - `write_profile` `openat`-Err arm (bad path) and the `write_all`-chain
//!   failure arm (a sink that opens but refuses writes).

use crate::{arch_test, sys, testrt};

use super::{num_data_entries, profile_path, write_all, write_padding, write_profile};

// Helper: open a temp file for writing and return the fd. The path must be
// NUL-terminated. Uses AT_FDCWD + O_WRONLY | O_CREAT | O_TRUNC.
fn open_writable(path: &[u8]) -> i32 {
    i32::try_from(
        sys::openat(
            sys::AT_FDCWD,
            path,
            sys::O_WRONLY | sys::O_CREAT | sys::O_TRUNC | sys::O_CLOEXEC,
            0o644,
        )
        .expect("open temp file for profiler test"),
    )
    .expect("fd fits i32")
}

arch_test!(profiler_write_all_success_returns_true, {
    // Drive the `Ok(n) => off += n` arm of `write_all` (profiler.rs L133):
    // a normal write to a writable fd advances the offset and returns true.
    let path = b"/tmp/reovim-profiler-write-all-ok\0";
    let fd = open_writable(path);
    let result = write_all(fd, b"hello profiler test");
    let _ = sys::close(fd);
    testrt::check(result, "write_all returns true on success");
});

arch_test!(profiler_write_all_bad_fd_returns_false, {
    // Drive the `Err(_) => return false` arm of `write_all` (profiler.rs L132):
    // writing to fd = -1 gives EBADF immediately.
    let result = write_all(-1, b"should fail");
    testrt::check(!result, "write_all returns false on bad fd");
});

arch_test!(profiler_write_all_closed_fd_returns_false, {
    // Open and immediately close a fd, then write to it — drives the Err arm
    // via EBADF on a previously-valid fd (distinct from the fd=-1 case to
    // exercise the Err arm through a real write attempt rather than a trivially
    // invalid fd, confirming the branch is taken in the realistic failure mode).
    let path = b"/tmp/reovim-profiler-write-all-closed\0";
    let fd = open_writable(path);
    let _ = sys::close(fd);
    // fd is now closed; write returns EBADF.
    let result = write_all(fd, b"after close");
    testrt::check(!result, "write_all on closed fd returns false");
});

arch_test!(profiler_write_all_empty_buf_is_true, {
    // An empty buffer: the while loop never runs (off=0, buf.len()=0),
    // `write` is never called, returns true immediately. Covers the
    // `while off < buf.len()` false arm (no iterations).
    let path = b"/tmp/reovim-profiler-write-all-empty\0";
    let fd = open_writable(path);
    let result = write_all(fd, b"");
    let _ = sys::close(fd);
    testrt::check(result, "write_all with empty buf returns true");
});

arch_test!(profiler_write_padding_zero_len_is_true, {
    // Drive the `while remaining > 0` false arm of `write_padding`
    // (profiler.rs L202): when len=0 the loop never runs.
    let path = b"/tmp/reovim-profiler-padding-zero\0";
    let fd = open_writable(path);
    let result = write_padding(fd, 0);
    let _ = sys::close(fd);
    testrt::check(result, "write_padding(0) returns true without writing");
});

arch_test!(profiler_write_padding_nonzero_len_runs_loop, {
    // Drive the `while remaining > 0` true arm and the inner `!write_all`
    // false arm (all writes succeed): `write_padding` with len=9 runs the
    // loop twice (8 bytes + 1 byte), each iteration writing zeros.
    let path = b"/tmp/reovim-profiler-padding-nine\0";
    let fd = open_writable(path);
    // 9 bytes: two iterations (min(9,8)=8, then min(1,8)=1).
    let result = write_padding(fd, 9);
    let _ = sys::close(fd);
    testrt::check(result, "write_padding(9) returns true after writing 9 zero bytes");
});

arch_test!(profiler_write_padding_bad_fd_returns_false, {
    // Drive the `!write_all` true arm of `write_padding` (profiler.rs L204):
    // write_padding with a bad fd calls write_all which returns false, and
    // write_padding propagates false.
    let result = write_padding(-1, 4);
    testrt::check(!result, "write_padding on bad fd returns false");
});

arch_test!(profiler_profile_path_returns_none_when_unset, {
    // Drive the end-of-loop `None` return (profiler.rs L226): no entry
    // matches the LLVM_PROFILE_FILE= key.
    let env: &[&[u8]] = &[b"HOME=/root\0", b"PATH=/usr/bin\0"];
    let result = profile_path(env);
    testrt::check(result.is_none(), "profile_path returns None when key absent");
});

arch_test!(profiler_profile_path_returns_some_when_set, {
    // Drive the `return Some(&entry[KEY.len()..])` arm (profiler.rs L223):
    // an entry starting with LLVM_PROFILE_FILE= returns the value tail.
    let env: &[&[u8]] = &[b"HOME=/root\0", b"LLVM_PROFILE_FILE=/tmp/out.profraw\0"];
    let result = profile_path(env);
    testrt::check(result.is_some(), "profile_path returns Some when key is set");
    testrt::check(
        result.unwrap() == b"/tmp/out.profraw\0",
        "profile_path returns the value including NUL terminator",
    );
});

arch_test!(profiler_profile_path_skips_short_entry, {
    // Drive the `entry.len() > KEY.len()` false arm (profiler.rs L219):
    // an entry shorter than or equal to KEY.len() is skipped.
    const KEY_LEN: usize = b"LLVM_PROFILE_FILE=".len();
    // Entry with exactly KEY_LEN bytes (no value) — len is NOT > KEY.len().
    let short: &[u8] = b"LLVM_PROFILE_FILE=";
    testrt::check_eq(short.len(), KEY_LEN);
    let env: &[&[u8]] = &[short];
    // Even though the prefix matches, the entry is too short (not > KEY.len()).
    let result = profile_path(env);
    testrt::check(result.is_none(), "profile_path skips entry with no value");
});

arch_test!(profiler_profile_path_skips_wrong_prefix, {
    // Drive the `&entry[..KEY.len()] == KEY` false arm (profiler.rs L219):
    // an entry longer than KEY.len() but with a different prefix is skipped.
    let env: &[&[u8]] = &[b"LLVM_PROFILE_PATH=/different/key\0"];
    let result = profile_path(env);
    testrt::check(result.is_none(), "profile_path skips entry with wrong prefix");
});

arch_test!(profiler_num_data_entries_exact_multiple_is_some, {
    // The exact-multiple arm: a span that is a whole number of entries yields
    // Some(count).
    testrt::check_eq(num_data_entries(0), Some(0u64));
    testrt::check_eq(num_data_entries(64), Some(1u64));
    testrt::check_eq(num_data_entries(6400), Some(100u64));
});

arch_test!(profiler_num_data_entries_non_multiple_is_none, {
    // The version-stale guard arm: a span that is NOT a whole number of
    // entries yields None. This is the arm that is otherwise structurally
    // unreachable under correct instrumentation; the factored helper makes it
    // directly testable (no dead branch in `write_profile`).
    testrt::check(num_data_entries(63).is_none(), "63 is not a multiple of 64");
    testrt::check(num_data_entries(65).is_none(), "65 is not a multiple of 64");
    testrt::check(num_data_entries(100).is_none(), "100 is not a multiple of 64");
});

arch_test!(profiler_write_profile_openat_err_returns_false, {
    // Drive `write_profile`'s `openat`-Err arm: a path inside a directory that
    // does not exist makes `openat` fail (ENOENT). The section walk runs first
    // (the `__llvm_prf_*` sections are live in this instrumented binary), then
    // the open fails and `write_profile` returns false without writing.
    let bad_path = b"/nonexistent-reovim-dir-zzz/profile.profraw\0";
    // SAFETY: the `arch_coverage` selftest binary has live `__llvm_prf_*`
    // sections; `write_profile`'s section-walk contract is met. The open fails,
    // so no file is produced.
    let ok = unsafe { write_profile(bad_path) };
    testrt::check(!ok, "write_profile returns false when openat fails");
});

arch_test!(profiler_write_profile_write_failure_returns_false, {
    // Drive the `write_all`-chain failure arm: `/dev/full` opens successfully
    // but every write fails with ENOSPC, so the first `write_all` in the chain
    // (the header) returns false and `write_profile` short-circuits to false.
    let full = b"/dev/full\0";
    // SAFETY: as above; the open succeeds but writes fail, exercising the
    // chain's false arm. No durable file is produced.
    let ok = unsafe { write_profile(full) };
    testrt::check(!ok, "write_profile returns false when a section write fails");
});

arch_test!(profiler_write_profile_good_path_writes_full_chain, {
    // The full success chain (header + data + cnts + bits + names + padding +
    // close + ok=true), executed MID-RUN so its own counter increments land
    // BEFORE the exit-time profile serialization. The exit-time write cannot
    // count its own tail (the counters are read section-by-section while
    // writing — the self-measurement horizon); this mid-run call is what
    // makes the success arms measurable.
    let path = b"/tmp/reovim-arch-selftest-midrun.profraw\0";
    // SAFETY: the `arch_coverage` selftest binary has live `__llvm_prf_*`
    // sections; the path is writable. A throwaway profraw is produced and
    // removed by the host (tmp).
    let ok = unsafe { write_profile(path) };
    testrt::check(ok, "mid-run write_profile succeeds end-to-end");
});
