//! AB13 cleanup-panic fixture (#785 Phase 4).
//!
//! Boots via arch `_start`, opens the sink named by `argv[1]`, registers it
//! as the flush fd, raises the cleanup-context marker, then panics. A panic
//! in a cleanup context records `rollback = failed` in the [`PanicRecord`]
//! and the flushed LOG2 line carries `rollback=failed` (6.2 §AB13). With no
//! disposition registered the default halt code (70) applies. An integration
//! test asserts status 70 and that the flushed line carries the marker.
#![no_std]
#![no_main]
// `entry!` expands to `#[unsafe(no_mangle)]` and the argv helpers deref raw
// pointers; the fixture is unsafe by nature. The allow is scoped to this bin.
#![allow(unsafe_code)]

reovim_arch::entry!(|argc, argv, _envp| {
    // SAFETY: `argc`/`argv` are the kernel-supplied startup vectors `_start`
    // forwarded: `argv` has `argc` C-string entries then a NULL terminator.
    let Some(fd) = (unsafe { open_sink_from_argv(argc, argv) }) else {
        return 1;
    };
    if reovim_arch::panic::set_flush_fd(fd).is_err() {
        return 2;
    }
    // Simulate a shutdown/drop/unregister cleanup context: a panic here is an
    // AB13 cleanup panic, so the record + flushed line carry rollback=failed.
    reovim_arch::panic::enter_cleanup_context();
    panic!("ab13 cleanup-context panic");
});

/// Opens the sink file named by `argv[1]` (create + truncate) via raw arch
/// syscalls; returns its fd or `None` on failure.
///
/// # Safety
///
/// `argv` must be the kernel-supplied startup vector: `argc` C-string entries
/// then a NULL terminator.
unsafe fn open_sink_from_argv(argc: usize, argv: *const *const u8) -> Option<i32> {
    use reovim_arch::sys::{AT_FDCWD, O_CREAT, O_TRUNC, O_WRONLY, openat};
    if argc < 2 {
        return None;
    }
    // SAFETY: `argc >= 2`, so `argv[1]` is a valid C-string pointer.
    let path_ptr = unsafe { *argv.add(1) };
    // SAFETY: `path_ptr` is a kernel-supplied NUL-terminated argv string.
    let path = unsafe { nul_terminated_slice(path_ptr) }?;
    let raw = openat(AT_FDCWD, path, O_WRONLY | O_CREAT | O_TRUNC, 0o600).ok()?;
    i32::try_from(raw).ok()
}

/// Builds a byte slice spanning a C string plus its NUL terminator, or `None`
/// for a null pointer.
///
/// # Safety
///
/// `ptr`, when non-null, must point at a NUL-terminated C string valid for the
/// process lifetime (an argv entry).
unsafe fn nul_terminated_slice(ptr: *const u8) -> Option<&'static [u8]> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    loop {
        // SAFETY: `ptr` is a NUL-terminated argv string (caller contract);
        // scanning to the NUL stays in bounds.
        let byte = unsafe { *ptr.add(len) };
        if byte == 0 {
            break;
        }
        len += 1;
    }
    // SAFETY: `ptr..ptr+len+1` covers the C string and its terminator within
    // the process-lifetime argv allocation.
    Some(unsafe { core::slice::from_raw_parts(ptr, len + 1) })
}
