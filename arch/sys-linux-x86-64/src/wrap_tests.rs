//! Tests for `wrap.rs`, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: sibling file declared in `wrap.rs` via
//! `#[cfg(feature = "selftest")] #[path = "wrap_tests.rs"] mod tests;`. Tests
//! access only public items (the crate-root re-exports), so `super::`
//! crate-private access is not needed.

use {
    crate::{
        AT_FDCWD, CLOCK_MONOTONIC, CLOCK_REALTIME, EBADF, EINVAL, ENOENT, ENOMEM, EWOULDBLOCK,
        FUTEX_PRIVATE_FLAG, FUTEX_WAIT, FUTEX_WAKE, MAP_ANONYMOUS, MAP_PRIVATE, O_CLOEXEC, O_CREAT,
        O_RDONLY, O_TRUNC, O_WRONLY, PROT_NONE, PROT_READ, PROT_WRITE, Timespec, clock_gettime,
        close, futex, gettid, mmap, mprotect, munmap, openat, read, write,
    },
    reovim_testrt::{self as testrt, arch_test},
};

arch_test!(write_to_bad_fd_is_ebadf, {
    // fd -1 is never a valid descriptor.
    testrt::check_eq(write(-1, b"x"), Err(EBADF));
});

arch_test!(write_to_stdout_is_ok, {
    // fd 1 (stdout) is open; an empty write succeeds with 0 bytes written.
    testrt::check_eq(write(1, b""), Ok(0));
});

arch_test!(read_from_bad_fd_is_ebadf, {
    // fd -1 is never valid; read must return EBADF.
    let mut buf = [0u8; 4];
    testrt::check_eq(read(-1, &mut buf), Err(EBADF));
});

arch_test!(read_roundtrip_via_pipe_file, {
    // Write a known payload to a temp file, then read it back to verify the
    // `read` wrapper's success path. arch has no libc pipe(), so we use openat
    // O_CREAT|O_TRUNC|O_WRONLY to write, then O_RDONLY to read back.
    //
    // The path must be NUL-terminated for the kernel's C-string read.
    let path: &[u8] = b"/tmp/reovim-arch-selftest-read-roundtrip\0";
    let flags_w = O_CREAT | O_TRUNC | O_WRONLY;
    let wr_raw = openat(AT_FDCWD, path, flags_w, 0o600).expect("open for write");
    let wr = i32::try_from(wr_raw).expect("fd fits i32");
    let payload = b"arch-read-ok";
    let n = write(wr, payload).expect("write payload");
    testrt::check_eq(n, payload.len());
    let _ = close(wr);

    let rd_raw = openat(AT_FDCWD, path, O_RDONLY, 0).expect("open for read");
    let rd = i32::try_from(rd_raw).expect("fd fits i32");
    let mut buf = [0u8; 16];
    let got = read(rd, &mut buf).expect("read back");
    testrt::check_eq(got, payload.len());
    testrt::check(&buf[..got] == payload, "read bytes match written bytes");
    let _ = close(rd);
});

arch_test!(openat_missing_path_is_enoent, {
    let path = b"/no/such/reovim/arch/path\0";
    testrt::check_eq(openat(AT_FDCWD, path, O_RDONLY, 0), Err(ENOENT));
});

arch_test!(openat_dev_null_then_close_succeeds, {
    let path = b"/dev/null\0";
    let raw = openat(AT_FDCWD, path, O_RDONLY | O_CLOEXEC, 0).expect("/dev/null opens");
    let fd = i32::try_from(raw).expect("fd fits in i32");
    testrt::check(fd >= 0, "fd is non-negative");
    testrt::check(close(fd).is_ok(), "close succeeds");
    // Second close of the same fd is EBADF.
    testrt::check_eq(close(fd), Err(EBADF));
});

arch_test!(mmap_anon_page_then_munmap_succeeds, {
    let len = 4096;
    let addr = mmap(0, len, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0)
        .expect("anonymous page maps");
    testrt::check(addr != 0, "mmap returns non-zero address");
    testrt::check(munmap(addr, len).is_ok(), "munmap succeeds");
});

arch_test!(mprotect_guard_page_then_restore_succeeds, {
    let len = 2 * 4096;
    let addr = mmap(0, len, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0)
        .expect("two pages map");
    testrt::check(mprotect(addr, 4096, PROT_NONE).is_ok(), "mark guard page");
    testrt::check(mprotect(addr, 4096, PROT_READ | PROT_WRITE).is_ok(), "restore guard page");
    testrt::check(munmap(addr, len).is_ok(), "munmap succeeds");
});

arch_test!(mprotect_unmapped_range_is_enomem, {
    // A page-aligned address with nothing mapped: mprotect fails.
    let unmapped = 0x0000_4000_0000_0000;
    testrt::check_eq(mprotect(unmapped, 4096, PROT_READ), Err(ENOMEM));
});

arch_test!(mmap_zero_len_is_einval, {
    testrt::check_eq(
        mmap(0, 0, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0),
        Err(EINVAL),
    );
});

arch_test!(clock_gettime_monotonic_is_nonzero, {
    let mut ts = Timespec::default();
    testrt::check(clock_gettime(CLOCK_MONOTONIC, &mut ts).is_ok(), "monotonic ok");
    testrt::check(ts.tv_sec > 0 || ts.tv_nsec > 0, "monotonic nonzero");
});

arch_test!(clock_gettime_realtime_succeeds, {
    let mut ts = Timespec::default();
    testrt::check(clock_gettime(CLOCK_REALTIME, &mut ts).is_ok(), "realtime ok");
    testrt::check(ts.tv_sec > 0, "realtime past epoch");
});

arch_test!(futex_wake_on_local_word_is_ok, {
    let word: u32 = 0;
    let r = futex(core::ptr::from_ref(&word) as usize, FUTEX_WAKE | FUTEX_PRIVATE_FLAG, 1, 0, 0, 0);
    testrt::check_eq(r, Ok(0));
});

arch_test!(futex_wait_mismatch_is_eagain, {
    let word: u32 = 0;
    // `*uaddr` is 0 but we expect 1: FUTEX_WAIT returns EAGAIN immediately.
    let r = futex(core::ptr::from_ref(&word) as usize, FUTEX_WAIT | FUTEX_PRIVATE_FLAG, 1, 0, 0, 0);
    testrt::check_eq(r, Err(EWOULDBLOCK));
});

arch_test!(gettid_returns_positive_tid, {
    // `gettid` always succeeds; exercises `syscall0` (raw.rs L53-69) and the
    // `gettid` wrapper (wrap.rs L312-317). The main thread's tid is >= 1.
    let tid = gettid();
    testrt::check(tid > 0, "gettid returns positive tid");
});
