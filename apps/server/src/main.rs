//! `reovim-server` — the standalone server composition root (AL1, #797 Phase 4).
//!
//! Boots the kernel, registers the text Domain, sets up the single session,
//! starts the UDS listener, and parks the process. The composition root is the
//! only place that wires `ext` crates (`reovim-domain-text`) into the kernel —
//! the kernel crate itself never depends on `ext` (core/ext boundary,
//! `CLAUDE.md §Design Rules`).
//!
//! ## Invocation
//!
//! ```text
//! reovim-server <socket-path>
//! ```
//!
//! `<socket-path>` is a NUL-terminated path used as the UDS listener address.
//! The server parks after `start_listener` returns; the listener accept loop
//! runs on a background thread.
#![no_std]
#![no_main]
#![allow(unsafe_code)] // entry! expands to #[unsafe(no_mangle)]

use {
    reovim_lib_ds::Shared,
    reovim_domain_text::{TextHandler, TextProjector},
    reovim_kernel::{
        Init, LauncherArgs,
        session::{BufferId, DomainAttachmentId, SessionState, WindowId},
    },
    reovim_server_rt::start_listener,
};

/// Static text Domain singletons. Zero-sized structs; no heap needed.
static TEXT_HANDLER: TextHandler = TextHandler;
static TEXT_PROJECTOR: TextProjector = TextProjector;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_floor_linux_x86_64::entry;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_floor_linux_aarch64::entry;

entry!(|argc, argv, _envp| {
    // ── Install the platform vtable (SP02, AB12 write-once) ───────────────────
    // The composition root drives the install: this is the first statement of
    // the `entry!` closure, ahead of every `kabi::handle` read (the kernel boot,
    // the futex/socket syscalls), so the no-read-before-install invariant holds.
    // The result is ignored by construction — this is the sole process entry, so
    // a second install cannot occur here.
    let _ = reovim_platform_linux_native::install_platform();

    // ── Parse the socket path from argv[1] ────────────────────────────────────
    // Expect exactly one argument: the UDS socket path (NUL-terminated).
    // If missing, write a usage note to stderr and exit non-zero.
    let socket_path: &[u8] = if argc >= 2 {
        // SAFETY: argv[1] is a valid C string provided by the kernel; its
        // lifetime is the process lifetime and it is NUL-terminated.
        let ptr = unsafe { *argv.add(1) };
        if ptr.is_null() {
            write_stderr(b"reovim-server: argv[1] is NULL\n");
            return 1;
        }
        // Find the NUL terminator to form a byte slice including the NUL.
        let mut len = 0usize;
        loop {
            // SAFETY: argv[1] is a NUL-terminated C string; scanning to
            // the NUL byte stays within the kernel-provided string.
            if unsafe { *ptr.add(len) } == 0 {
                break;
            }
            len += 1;
        }
        // SAFETY: [ptr, ptr+len+1) is the NUL-terminated C string.
        unsafe { core::slice::from_raw_parts(ptr, len + 1) }
    } else {
        write_stderr(b"usage: reovim-server <socket-path>\n");
        return 1;
    };

    // ── Boot the kernel ───────────────────────────────────────────────────────
    let kernel = match Init::new(LauncherArgs::default()).boot() {
        Ok(k) => k,
        Err(_) => {
            write_stderr(b"reovim-server: kernel boot failed\n");
            return 1;
        }
    };

    // ── Register the text Domain ──────────────────────────────────────────────
    let domain_id = match kernel.register_domain("text", &TEXT_HANDLER, &TEXT_PROJECTOR) {
        Ok(id) => id,
        Err(_) => {
            write_stderr(b"reovim-server: register_domain failed\n");
            return 1;
        }
    };

    // ── Wire the single session ───────────────────────────────────────────────
    let state = SessionState::new(
        domain_id,
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    kernel.setup_session(state);

    // ── Start the UDS listener ────────────────────────────────────────────────
    if start_listener(&kernel, socket_path).is_err() {
        write_stderr(b"reovim-server: start_listener failed\n");
        return 1;
    }

    // ── Park the process ──────────────────────────────────────────────────────
    // The listener accept loop runs on a background thread. The main thread
    // parks here so the process stays alive. A real server would listen for a
    // shutdown signal; the walking skeleton parks indefinitely (kill via signal).
    park_forever(&kernel)
});

/// Parks the process by sleeping in a tight retry loop.
///
/// The walking skeleton has no graceful-shutdown signal yet; the process
/// is terminated externally (SIGTERM/SIGKILL from the launcher or test harness).
#[cold]
fn park_forever(kernel: &Shared<reovim_kernel::Kernel>) -> i32 {
    // Keep a reference to the kernel alive so it is not dropped.
    let _ = Shared::clone(kernel);
    // Sleep in 60-second increments via a FUTEX_WAIT on a local word.
    // The syscall may return early (EINTR from signals); the loop re-parks.
    // futex(uaddr, FUTEX_WAIT | FUTEX_PRIVATE_FLAG, val, timeout, uaddr2, val3):
    //   timeout = 0 → block indefinitely (no timeout pointer in this low-level shim).
    // Use FUTEX_WAIT on a word that never equals the expected value so the futex
    // sleeps until EINTR (signal delivery re-parks us) or forever.
    let word = core::sync::atomic::AtomicU32::new(0);
    let addr = ((&word) as *const core::sync::atomic::AtomicU32).addr();
    loop {
        // FUTEX_WAIT with expected value 0 and no timeout = park until woken.
        let _ = reovim_arch::sys::futex(
            addr,
            reovim_arch::sys::FUTEX_WAIT | reovim_arch::sys::FUTEX_PRIVATE_FLAG,
            0,
            0,
            0,
            0,
        );
    }
}

/// Writes `msg` to fd 2 (stderr). Best-effort; errors ignored.
fn write_stderr(msg: &[u8]) {
    let mut off = 0;
    while off < msg.len() {
        match reovim_arch::sys::write(2, &msg[off..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => off += n,
        }
    }
}
