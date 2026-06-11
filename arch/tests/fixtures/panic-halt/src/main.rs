//! AB12 halt-path fixture (#785 Phase 4).
//!
//! Boots via arch `_start`, opens the LOG7-sink file named by `argv[1]`,
//! registers it as the panic flush fd, then panics. With no disposition
//! registered the default is `halt` (6.2 §5.2), so the process exits the
//! halt code (70) after flushing the LOG2 panic line to the file. An
//! integration test execs this, asserts status 70, and parses the flushed
//! line against the LOG2 grammar.
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
    // No disposition registered → default halt (exit 70). Panic now; the
    // handler renders + flushes the line and exits the halt code. This return
    // is never reached.
    panic!("halt-path fixture panic");
});

/// Opens the sink file named by `argv[1]` for writing (create + truncate) via
/// raw arch syscalls and returns its fd, or `None` on any failure.
///
/// The path arrives as a C string in `argv[1]`; arch `openat` requires a
/// NUL-terminated slice, so the helper measures the string (including its
/// terminator) and hands the whole slice to the syscall.
///
/// # Safety
///
/// `argv` must be the kernel-supplied startup vector: `argc` C-string entries
/// followed by a NULL terminator. The caller (the `entry!` body) forwards
/// exactly that.
unsafe fn open_sink_from_argv(argc: usize, argv: *const *const u8) -> Option<i32> {
    use reovim_arch::sys::{AT_FDCWD, O_CREAT, O_TRUNC, O_WRONLY, openat};
    if argc < 2 {
        return None;
    }
    // SAFETY: `argc >= 2`, so `argv[1]` is a valid C-string pointer per the
    // caller's startup-vector contract.
    let path_ptr = unsafe { *argv.add(1) };
    // SAFETY: `path_ptr` is a kernel-supplied NUL-terminated argv string.
    let path = unsafe { nul_terminated_slice(path_ptr) }?;
    // Mode 0o600: owner read/write. The sink is a test artifact.
    let raw = openat(AT_FDCWD, path, O_WRONLY | O_CREAT | O_TRUNC, 0o600).ok()?;
    i32::try_from(raw).ok()
}

/// Builds a byte slice spanning a C string and its NUL terminator from a raw
/// pointer, or `None` if the pointer is null.
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
        // SAFETY: `ptr` points at a NUL-terminated argv string (caller
        // contract); reading forward until the NUL stays in bounds.
        let byte = unsafe { *ptr.add(len) };
        if byte == 0 {
            break;
        }
        len += 1;
    }
    // Include the terminator so `openat`'s C-string read sees the NUL.
    // SAFETY: `ptr..ptr+len+1` is the C string plus its terminator, within the
    // process-lifetime argv allocation.
    Some(unsafe { core::slice::from_raw_parts(ptr, len + 1) })
}
