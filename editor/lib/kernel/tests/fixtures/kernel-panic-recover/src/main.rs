//! AB12 kernel recover-path fixture (#796 Phase 5).
//!
//! Boots via arch `_start`, opens the LOG7 sink file named by `argv[1]`,
//! registers it as the panic flush fd, boots the kernel (which registers the
//! remaining three arch panic-seam hooks: ring_tail provider, state-record
//! hook, and recover disposition), emits a few events to populate the
//! ring/flush buffer, then panics. The panic handler flushes the ring's LOG2
//! lines followed by the panic line to the sink file and exits the recover
//! code (75, `EX_TEMPFAIL`).
//!
//! The integration test (`tests/fixtures_exec.rs`) execs this fixture, asserts
//! exit code 75, and parses the flushed file content for LOG2 grammar
//! conformance and the presence of the panic line.
//!
//! State-record AC (c) — "the state-record stub observed the matching
//! PanicRecord" — cannot be asserted out-of-process because the kernel's
//! `record_panic_state` hook stores the `PanicRecord` into an `AtomicU32`
//! slot (`STATE_RECORD`) that exits with the process. The in-process
//! selftest suite (`init_tests.rs`) covers (c) by calling `record_panic_state`
//! directly and asserting `last_panic_record()`. This fixture proves (a) exit
//! code + (b) flushed file content end-to-end; (c) is covered by selftest.
#![no_std]
#![no_main]
// `entry!` expands to `#[unsafe(no_mangle)]`; argv helpers dereference raw
// pointers. The allow is scoped to this bin.
#![allow(unsafe_code)]

use reovim_kernel::{Init, LauncherArgs};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_floor_linux_x86_64::entry;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_floor_linux_aarch64::entry;

entry!(|argc, argv, _envp| {
    // Install the platform handle as the closure's first statement, before
    // `Init::boot()` reads it. The fixture is host-only (native-only pin per
    // `fixtures_exec.rs`), so only the real POSIX provider is needed — no
    // bare-metal scaffold branch.
    let _ = reovim_platform_linux_native::install_platform();
    // SAFETY: `argc`/`argv` are the kernel-supplied startup vectors `_start`
    // forwarded: `argv` has `argc` C-string entries then a NULL terminator.
    let Some(fd) = (unsafe { open_sink_from_argv(argc, argv) }) else {
        return 1;
    };

    // Register the flush fd BEFORE booting. `boot()` registers the other
    // three seam hooks (ring_tail_provider, state_record_hook, disposition).
    // `set_flush_fd` is a separate hook slot — no conflict.
    if reovim_arch::panic::set_flush_fd(fd).is_err() {
        return 2;
    }

    // Boot the kernel with `Recover` disposition (75 exit code on panic).
    // `boot()` registers:
    //   1. ring_tail_provider  → flush::ring_tail
    //   2. state_record_hook   → record_panic_state (AtomicU32 slot)
    //   3. disposition         → Recover (exit 75)
    // It also runs stages 1..7 which emit boot.stage.* events into the ring,
    // populating the flush mirror. The ring content is sufficient for the
    // "a few log lines" AC; no additional emits are needed.
    let args = LauncherArgs {
        disposition: reovim_arch::panic::Disposition::Recover,
        ..LauncherArgs::default()
    };
    let _kernel = match Init::new(args).boot() {
        Ok(k) => k,
        Err(_) => return 3,
    };

    // Panic now. The arch panic handler fires:
    //   1. ring_tail provider → pre-rendered LOG2 bytes flushed to fd
    //      (boot stages emitted boot.stage.* events populating the mirror)
    //   2. panic line rendered + flushed to fd
    //   3. state_record_hook(PanicRecord{Recover, rollback_failed:false})
    //      called → stored in AtomicU32 STATE_RECORD slot
    //   4. exit(75) — Disposition::Recover exit code
    //
    // This return is never reached.
    panic!("recover-path kernel fixture panic");
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
        // SAFETY: `ptr` is a NUL-terminated argv string (caller contract).
        let byte = unsafe { *ptr.add(len) };
        if byte == 0 {
            break;
        }
        len += 1;
    }
    // Include the NUL terminator so `openat`'s C-string read sees it.
    // SAFETY: `ptr..ptr+len+1` is within the process-lifetime argv allocation.
    Some(unsafe { core::slice::from_raw_parts(ptr, len + 1) })
}
