//! AB12 recover-path fixture (#785 Phase 4).
//!
//! Boots via arch `_start`, opens the sink named by `argv[1]`, registers it
//! as the flush fd, registers `recover` disposition and a state-record hook,
//! then panics. The handler fires the hook (which writes a marker carrying
//! the observed `PanicRecord` fields to the sink) and exits the recover code
//! (75, `EX_TEMPFAIL`). An integration test asserts status 75, that the
//! state-hook marker is present, and parses the panic line.
#![no_std]
#![no_main]
// `entry!` expands to `#[unsafe(no_mangle)]` and the argv helpers deref raw
// pointers; the fixture is unsafe by nature. The allow is scoped to this bin.
#![allow(unsafe_code)]

use core::sync::atomic::{AtomicI32, Ordering};

use reovim_arch::panic::{Disposition, PanicRecord};

/// The sink fd, stashed so the state-record hook (which only receives a
/// `PanicRecord`) can write its marker to the same file the flush targets.
static SINK_FD: AtomicI32 = AtomicI32::new(-1);

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_floor_linux_x86_64::entry;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_floor_linux_aarch64::entry;

entry!(|argc, argv, _envp| {
    // SAFETY: `argc`/`argv` are the kernel-supplied startup vectors `_start`
    // forwarded: `argv` has `argc` C-string entries then a NULL terminator.
    let Some(fd) = (unsafe { open_sink_from_argv(argc, argv) }) else {
        return 1;
    };
    SINK_FD.store(fd, Ordering::Release);
    if reovim_arch::panic::set_flush_fd(fd).is_err() {
        return 2;
    }
    if reovim_arch::panic::set_disposition(Disposition::Recover).is_err() {
        return 3;
    }
    if reovim_arch::panic::set_state_record_hook(state_hook).is_err() {
        return 4;
    }
    // Exits 75 via the recover disposition after the hook + flush.
    panic!("recover-path fixture panic");
});

/// The state-record hook: writes a deterministic marker carrying the observed
/// disposition and rollback flag so the integration test can confirm the hook
/// fired with the expected [`PanicRecord`].
fn state_hook(record: PanicRecord) {
    let fd = SINK_FD.load(Ordering::Acquire);
    if fd < 0 {
        return;
    }
    let disp = match record.disposition {
        Disposition::Recover => b"state-hook: disposition=recover rollback=".as_slice(),
        Disposition::Halt => b"state-hook: disposition=halt rollback=".as_slice(),
    };
    write_all(fd, disp);
    write_all(fd, if record.rollback_failed { b"failed\n" } else { b"ok\n" });
}

/// Writes the whole of `buf` to `fd`, looping over short writes (best effort).
fn write_all(fd: i32, buf: &[u8]) {
    use reovim_arch::sys::write;
    let mut off = 0;
    while off < buf.len() {
        match write(fd, &buf[off..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => off += n,
        }
    }
}

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
