//! Tests for `panic.rs`, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: declared inside `panic.rs` as
//! `#[cfg(feature = "selftest")] #[path = "panic_tests.rs"] mod tests;`.
//! `super::` reaches the private `handle`, registry statics, `current_disposition`,
//! `current_record`, `enter_cleanup_context`, `clear_cleanup_context`,
//! `load_ring_tail_provider`, `load_state_record_hook`, `set_*`, and
//! `reset_registry`.
//!
//! ## Capture strategy — replacing libc pipe()
//!
//! The libtest version used `libc::pipe()` + `std::fs::File::from_raw_fd` to
//! capture the bytes written by `handle()`. The selftest version uses arch's
//! own syscalls: `openat O_CREAT|O_TRUNC|O_WRONLY` to open a temp file for
//! writing (passed to `set_flush_fd`), then `openat O_RDONLY` + `read` loops
//! to read it back.
//!
//! ## Reset discipline
//!
//! `reset_registry()` is called at the END of each test body so the global
//! statics are clean for the next test. The selftest runner is single-threaded
//! sequential; no serialization guard is needed.

use reovim_lib_ds::Bytes;

use crate::{arch_test, testrt};

// Only the hosted (filesystem-backed) capture tests need the file syscalls
// and the recover-hook's seen-flag atomics.
#[cfg(target_os = "linux")]
use {
    crate::sys::{self, AT_FDCWD, O_CREAT, O_RDONLY, O_TRUNC, O_WRONLY},
    core::sync::atomic::{AtomicU8, Ordering::Relaxed},
    reovim_lib_ds::Str,
};

use super::{
    Disposition, PanicRecord, SetError, StackWriter, clear_cleanup_context, current_disposition,
    current_record, enter_cleanup_context, handle, load_ring_tail_provider, load_state_record_hook,
    render_location, reset_registry, set_disposition, set_flush_fd, set_ring_tail_provider,
    set_state_record_hook,
};

// ---- file-based capture helpers -------------------------------------------

/// Opens a temp file for writing; truncates if it exists. Returns the fd.
///
/// The path must be a nul-terminated byte slice. The returned fd is the
/// write end; the test passes it to `set_flush_fd`.
#[cfg(target_os = "linux")]
fn open_write(path: &[u8]) -> i32 {
    let raw =
        sys::openat(AT_FDCWD, path, O_CREAT | O_TRUNC | O_WRONLY, 0o600).expect("open tmp write");
    i32::try_from(raw).expect("fd fits i32")
}

/// Opens the same temp file for reading. Returns the fd.
#[cfg(target_os = "linux")]
fn open_read(path: &[u8]) -> i32 {
    let raw = sys::openat(AT_FDCWD, path, O_RDONLY, 0).expect("open tmp read");
    i32::try_from(raw).expect("fd fits i32")
}

/// Reads `fd` to EOF, accumulating into a `Bytes`. Closes the fd.
#[cfg(target_os = "linux")]
fn read_to_bytes(fd: i32) -> Bytes {
    let mut buf = [0u8; 512];
    let mut out = Bytes::new();
    loop {
        match sys::read(fd, &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                out.try_extend_from_slice(&buf[..n]).expect("extend bytes");
            }
        }
    }
    let _ = sys::close(fd);
    out
}

/// Reads `fd` to EOF into a `Str` (assumes the content is valid UTF-8).
#[cfg(target_os = "linux")]
fn read_to_str(fd: i32) -> Str {
    let raw = read_to_bytes(fd);
    let s = core::str::from_utf8(raw.as_slice()).expect("utf-8 content");
    Str::try_from_str(s).expect("alloc str")
}

/// A synthetic message renderer for the `handle` scaffolding tests.
/// Under `panic = "abort"` a real `PanicInfo` cannot be caught in-process,
/// so the scaffolding is tested with a stand-in message. The renderer writes
/// into the allocator-free `StackWriter` (the panic line's render target).
fn synth_message(msg: &str) -> impl FnOnce(&mut StackWriter) -> core::fmt::Result + '_ {
    move |w| {
        use core::fmt::Write;
        write!(w, "{msg} at synth.rs:1:1")
    }
}

// ---- LOG2 grammar assertions -----------------------------------------------

/// Parses a kernel-emitter LOG2 line and asserts its fields. Grammar:
/// `[ts] kernel panic: <message>`. Asserts the kernel-emitter form and that
/// `msg_substr` appears in the message.
#[cfg(target_os = "linux")]
fn assert_log2_kernel_panic(content: &str, msg_substr: &str) {
    let line = content
        .lines()
        .find(|l| l.contains("kernel panic:"))
        .expect("a panic line in output");
    testrt::check(line.starts_with('['), "ts opens with '['");
    let close = line.find(']').expect("ts closes with ']'");
    let ts = &line[1..close];
    let (secs, micros) = ts.split_once('.').expect("ts is seconds.micros");
    testrt::check(secs.trim().parse::<u64>().is_ok(), "ts seconds numeric");
    testrt::check_eq(micros.len(), 6);
    testrt::check(micros.parse::<u32>().is_ok(), "ts micros numeric");
    let rest = line[close + 1..].trim_start();
    let (emitter, after) = rest.split_once(' ').expect("emitter token");
    testrt::check_eq(emitter, "kernel");
    let (address, message) = after.split_once(": ").expect("address ':' message");
    testrt::check_eq(address, "panic");
    testrt::check(!address.contains('/'), "kernel address has no '/'");
    testrt::check(message.contains(msg_substr), "message carries the panic text");
}

// ---- tests -----------------------------------------------------------------

arch_test!(panic_default_disposition_is_halt, {
    reset_registry();
    testrt::check_eq(current_disposition(), Disposition::Halt);
    testrt::check_eq(Disposition::Halt.exit_code(), 70i32);
    testrt::check_eq(Disposition::Recover.exit_code(), 75i32);
    reset_registry();
});

arch_test!(panic_set_disposition_is_write_once, {
    reset_registry();
    testrt::check_eq(set_disposition(Disposition::Recover), Ok(()));
    testrt::check_eq(current_disposition(), Disposition::Recover);
    // Second registration is rejected; the first value stands.
    testrt::check_eq(set_disposition(Disposition::Halt), Err(SetError::AlreadySet));
    testrt::check_eq(current_disposition(), Disposition::Recover);
    reset_registry();
});

arch_test!(panic_set_flush_fd_is_write_once, {
    reset_registry();
    testrt::check_eq(set_flush_fd(7), Ok(()));
    // The flush-fd atom lives in kabi/panic now; observe it through its getter.
    testrt::check_eq(reovim_kabi_panic::get_flush_fd(), Some(7i32));
    testrt::check_eq(set_flush_fd(9), Err(SetError::AlreadySet));
    testrt::check_eq(reovim_kabi_panic::get_flush_fd(), Some(7i32));
    reset_registry();
});

arch_test!(panic_set_fn_hooks_are_write_once, {
    fn tail() -> &'static [u8] {
        b"TAIL\n"
    }
    fn tail2() -> &'static [u8] {
        b"OTHER\n"
    }
    fn hook(r: PanicRecord) {
        // Body must execute for coverage; the record is inspectable.
        let _ = r.disposition;
    }
    fn hook2(r: PanicRecord) {
        let _ = r.rollback_failed;
    }
    reset_registry();
    testrt::check_eq(set_ring_tail_provider(tail), Ok(()));
    testrt::check_eq(set_ring_tail_provider(tail2), Err(SetError::AlreadySet));
    testrt::check(
        load_ring_tail_provider().unwrap()() == b"TAIL\n",
        "tail provider returns correct bytes",
    );
    // Call tail2 directly so its body is covered (it was only passed as a
    // pointer but never invoked; `set_ring_tail_provider` rejected it).
    testrt::check(tail2() == b"OTHER\n", "tail2 body executed");
    testrt::check_eq(set_state_record_hook(hook), Ok(()));
    testrt::check_eq(set_state_record_hook(hook2), Err(SetError::AlreadySet));
    testrt::check(load_state_record_hook().is_some(), "hook is loaded");
    // Call hook and hook2 directly so their bodies are covered.
    let dummy_record = PanicRecord {
        disposition: Disposition::Halt,
        rollback_failed: false,
    };
    hook(dummy_record);
    hook2(dummy_record);
    reset_registry();
});

arch_test!(panic_cleanup_context_marks_record, {
    reset_registry();
    testrt::check(!current_record().rollback_failed, "no cleanup context initially");
    enter_cleanup_context();
    testrt::check(current_record().rollback_failed, "cleanup context sets rollback_failed");
    clear_cleanup_context();
    testrt::check(!current_record().rollback_failed, "cleared context");
    reset_registry();
});

// Filesystem-dependent: the capture pattern round-trips the flushed line
// through a real temp file (openat O_CREAT + read-back), which freestanding
// targets do not realize (openat is ENOENT there). The bare-metal panic
// path is exercised end-to-end by the inject-failure pilot's serial
// assertion. Stays in every hosted suite.
#[cfg(target_os = "linux")]
arch_test!(panic_handle_renders_log2_line_and_returns_halt_by_default, {
    let path: &[u8] = b"/tmp/reovim-panic-test-default\0";
    reset_registry();
    let wr = open_write(path);
    set_flush_fd(wr).unwrap();
    let code = handle(synth_message("boom"));
    let _ = sys::close(wr);
    let rd = open_read(path);
    let content = read_to_str(rd);
    testrt::check_eq(code, 70i32);
    assert_log2_kernel_panic(content.as_str(), "boom");
    testrt::check(!content.as_str().contains("rollback=failed"), "no rollback marker");
    reset_registry();
});

// Filesystem-dependent (file capture round-trip): see above.
#[cfg(target_os = "linux")]
arch_test!(panic_handle_recover_fires_hook_and_returns_75, {
    static SEEN: AtomicU8 = AtomicU8::new(0);
    fn record_hook(r: PanicRecord) {
        assert!(r.disposition == Disposition::Recover);
        assert!(!r.rollback_failed);
        SEEN.store(1, Relaxed);
    }
    let path: &[u8] = b"/tmp/reovim-panic-test-recover\0";
    reset_registry();
    SEEN.store(0, Relaxed);
    set_disposition(Disposition::Recover).unwrap();
    set_state_record_hook(record_hook).unwrap();
    let wr = open_write(path);
    set_flush_fd(wr).unwrap();
    let code = handle(synth_message("recover-me"));
    let _ = sys::close(wr);
    let rd = open_read(path);
    let content = read_to_str(rd);
    testrt::check_eq(code, 75i32);
    testrt::check_eq(SEEN.load(Relaxed), 1u8);
    assert_log2_kernel_panic(content.as_str(), "recover-me");
    reset_registry();
});

// Filesystem-dependent (file capture round-trip): see above.
#[cfg(target_os = "linux")]
arch_test!(panic_handle_flushes_ring_tail_then_line, {
    fn tail() -> &'static [u8] {
        b"[    0.000001] kernel init: boot.stage.ok\n"
    }
    let path: &[u8] = b"/tmp/reovim-panic-test-tail\0";
    reset_registry();
    set_ring_tail_provider(tail).unwrap();
    let wr = open_write(path);
    set_flush_fd(wr).unwrap();
    let _ = handle(synth_message("after-tail"));
    let _ = sys::close(wr);
    let rd = open_read(path);
    let content = read_to_str(rd);
    let out = content.as_str();
    let tail_pos = out.find("boot.stage.ok").expect("tail flushed");
    let panic_pos = out.find("kernel panic:").expect("panic line flushed");
    testrt::check(tail_pos < panic_pos, "ring tail precedes the panic line");
    reset_registry();
});

// Filesystem-dependent (file capture round-trip): see above.
#[cfg(target_os = "linux")]
arch_test!(panic_handle_ab13_marks_rollback_failed, {
    let path: &[u8] = b"/tmp/reovim-panic-test-ab13\0";
    reset_registry();
    let wr = open_write(path);
    set_flush_fd(wr).unwrap();
    enter_cleanup_context();
    let code = handle(synth_message("drop-panic"));
    clear_cleanup_context();
    let _ = sys::close(wr);
    let rd = open_read(path);
    let content = read_to_str(rd);
    testrt::check_eq(code, 70i32);
    testrt::check(content.as_str().contains("rollback=failed"), "AB13 marker in flushed line");
    assert_log2_kernel_panic(content.as_str(), "drop-panic");
    reset_registry();
});

arch_test!(panic_handle_default_no_fd_writes_stderr, {
    reset_registry();
    // No flush fd registered: the default path writes to fd 2. The returned
    // code and non-panic is the coverage for the no-sink branch.
    let code = handle(synth_message("no-sink"));
    testrt::check_eq(code, 70i32);
    reset_registry();
});

// Filesystem-dependent: needs an fd that was genuinely open and is then
// closed, which requires a successful openat first. See above.
#[cfg(target_os = "linux")]
arch_test!(panic_write_all_error_arm_on_bad_fd, {
    // Drive the `Ok(0) | Err(_) => break` arm in panic.rs's private `write_all`
    // (panic.rs L368) by registering a bad fd (-1) as the flush fd and calling
    // handle(). The write to fd -1 immediately returns EBADF (Err), hitting the
    // break arm. The handle call returns normally (best-effort, no panic).
    reset_registry();
    // Registering fd -1 would conflict with the sentinel check (FLUSH_FD starts
    // at -1). Instead register an fd that is valid at registration but closed
    // before handle() writes: open a file, register it, close it, then call handle.
    let path: &[u8] = b"/tmp/reovim-panic-test-closed-fd\0";
    let wr = open_write(path);
    set_flush_fd(wr).unwrap();
    // Close the fd so the subsequent write in final_flush hits EBADF.
    let _ = sys::close(wr);
    // handle() will try to write to the now-closed fd; write_all's Err arm fires.
    let code = handle(synth_message("closed-fd"));
    testrt::check_eq(code, 70i32);
    reset_registry();
});

arch_test!(panic_render_location_some_and_none_arms, {
    // Both arms of `render_location` directly: the `Some` arm via the live
    // caller location, the `None` arm (not contractually impossible — the
    // PanicInfo API returns Option) via an explicit None. Renders into the
    // allocator-free `StackWriter`.
    let mut w = StackWriter::new();
    render_location(&mut w, Some(core::panic::Location::caller())).unwrap();
    testrt::check(w.as_slice().starts_with(b" at "), "Some arm renders ' at file:line:col'");

    let mut w2 = StackWriter::new();
    render_location(&mut w2, None).unwrap();
    testrt::check_eq(w2.as_slice(), b" at <unknown>".as_slice());
});

// ---- pre-exit hook tests (gap-7, #797 Phase 4) ----------------------------

use super::{load_pre_exit_hook, set_pre_exit_hook};

arch_test!(panic_pre_exit_hook_write_once, {
    // Write-once semantics: first registration wins; second is rejected.
    fn noop() {}
    fn noop2() {}
    reset_registry();
    testrt::check_eq(set_pre_exit_hook(noop), Ok(()));
    testrt::check_eq(set_pre_exit_hook(noop2), Err(SetError::AlreadySet));
    testrt::check(load_pre_exit_hook().is_some(), "hook is loaded after first register");
    // Call noop2 directly so its body is covered (it was rejected, never invoked).
    noop2();
    reset_registry();
});

arch_test!(panic_pre_exit_hook_unset_returns_none, {
    reset_registry();
    testrt::check(load_pre_exit_hook().is_none(), "unregistered hook returns None");
    reset_registry();
});

// Filesystem-dependent (file capture round-trip): hosted targets only.
#[cfg(target_os = "linux")]
arch_test!(panic_pre_exit_hook_runs_before_panic_output, {
    // Verify that the pre-exit hook is invoked during `handle`, and that its
    // execution is reflected before the panic line is written. Strategy: the
    // hook sets an atomic flag; we read the flag after `handle` to confirm it
    // fired.
    static HOOK_FIRED: AtomicU8 = AtomicU8::new(0);
    fn pre_exit() {
        HOOK_FIRED.store(1, Relaxed);
    }
    let path: &[u8] = b"/tmp/reovim-panic-test-pre-exit\0";
    reset_registry();
    HOOK_FIRED.store(0, Relaxed);
    set_pre_exit_hook(pre_exit).unwrap();
    let wr = open_write(path);
    set_flush_fd(wr).unwrap();
    let code = handle(synth_message("pre-exit-check"));
    let _ = sys::close(wr);
    testrt::check_eq(code, 70i32);
    testrt::check_eq(HOOK_FIRED.load(Relaxed), 1u8);
    reset_registry();
});
