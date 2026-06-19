//! `reovim` — the composition root launcher (#797 Phase 4).
//!
//! Boots the server runtime (kernel + text Domain + UDS listener) and the TUI
//! client in a single process (in-process composition). The real UDS carrier is
//! exercised end-to-end: the TUI client connects over the socket the server
//! runtime listens on, exercising the framed protocol over a real kernel UDS.
//!
//! ## Deviation from the plan's subprocess language
//!
//! The plan (Phase 4, pin-2) specifies subprocess UDS composition (fork/exec
//! `reovim-server`, then run the TUI). `execve` is not yet in the arch floor
//! (only `clone`-based threads exist). Rather than adding fork/exec syscalls
//! out of phase, the launcher uses in-process UDS composition: the kernel +
//! listener boot on a background thread, the TUI client runs on the main
//! thread and connects over the same UDS socket. The real carrier is exercised
//! identically; the process boundary is the only difference. The embedded
//! in-memory adapter (AL-INPROC) is NOT used — the carrier path is the real
//! UDS loop. Fork/exec composition lands when `execve` enters the arch floor.
//!
//! ## Invocation
//!
//! ```text
//! reovim [socket-path]
//! ```
//!
//! `socket-path` defaults to `/tmp/reovim-launcher.sock`. The path must fit in
//! 107 bytes. It is removed after the TUI session ends.
#![no_std]
#![no_main]
#![allow(unsafe_code)] // entry! expands to #[unsafe(no_mangle)] shim

use {
    reovim_lib_ds::Shared,
    reovim_domain_text::{TextHandler, TextProjector},
    reovim_kernel::{
        Init, LauncherArgs,
        session::{BufferId, DomainAttachmentId, SessionState, WindowId},
    },
    reovim_platform_tui::paint::{RunArgs, run as tui_run},
    reovim_server_rt::start_listener,
};

/// Static text Domain singletons (zero-sized structs; no heap needed).
static TEXT_HANDLER: TextHandler = TextHandler;
static TEXT_PROJECTOR: TextProjector = TextProjector;

/// Default socket path used when no argument is given.
static DEFAULT_SOCKET: &[u8] = b"/tmp/reovim-launcher.sock\0";

reovim_arch::entry!(|argc, argv, _envp| {
    // ── Install the platform vtable (SP02, AB12 write-once) ───────────────────
    // The composition root drives the install: this is the first statement of
    // the `entry!` closure, ahead of every `kabi::handle` read (the kernel boot,
    // the socket syscalls), so the no-read-before-install invariant holds. The
    // result is ignored by construction — this is the sole process entry, so a
    // second install cannot occur here.
    let _ = reovim_platform_linux_native::install_platform();

    // ── Resolve the socket path ───────────────────────────────────────────────
    let socket_path: &'static [u8] = if argc >= 2 {
        // SAFETY: argv[1] is a valid NUL-terminated C string for the process
        // lifetime. We build a static byte slice from it.
        let ptr = unsafe { *argv.add(1) };
        if ptr.is_null() {
            return 1;
        }
        let mut len = 0usize;
        // SAFETY: scanning a NUL-terminated C string to its terminator.
        while unsafe { *ptr.add(len) } != 0 {
            len += 1;
        }
        // SAFETY: the argv data lives for the process lifetime; re-borrowing
        // as 'static is sound here because we never move past _start's stack.
        unsafe { core::slice::from_raw_parts(ptr, len + 1) }
    } else {
        DEFAULT_SOCKET
    };

    // ── Boot the kernel ───────────────────────────────────────────────────────
    let kernel = match Init::new(LauncherArgs::default()).boot() {
        Ok(k) => k,
        Err(_) => {
            write_stderr(b"reovim: kernel boot failed\n");
            return 1;
        }
    };

    // ── Register the text Domain ──────────────────────────────────────────────
    let domain_id = match kernel.register_domain("text", &TEXT_HANDLER, &TEXT_PROJECTOR) {
        Ok(id) => id,
        Err(_) => {
            write_stderr(b"reovim: register_domain failed\n");
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

    // ── Start the UDS listener on a background thread ────────────────────────
    // Unlink any stale socket from a previous run before binding.
    let _ = reovim_arch::sys::unlinkat(reovim_arch::sys::AT_FDCWD, socket_path, 0);

    if start_listener(&kernel, socket_path).is_err() {
        write_stderr(b"reovim: start_listener failed\n");
        return 1;
    }

    // Keep the kernel alive across the TUI session.
    let _kernel_ref = Shared::clone(&kernel);

    // ── Run the TUI client (blocks until the user quits) ─────────────────────
    // The TUI client connects over the same UDS socket the listener just bound,
    // exercising the real framed carrier end-to-end.
    let result = tui_run(RunArgs { socket_path });

    // ── Clean up the socket path ──────────────────────────────────────────────
    let _ = reovim_arch::sys::unlinkat(reovim_arch::sys::AT_FDCWD, socket_path, 0);

    match result {
        Ok(()) => 0,
        Err(_) => 1,
    }
});

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
