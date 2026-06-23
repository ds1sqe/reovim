//! `reovim-server` — the standalone server composition root (AL1, #797 Phase 4).
//!
//! Boots the editor core, registers the text Domain, sets up the single session,
//! starts the UDS listener, and parks the process. The composition root is the
//! only place that wires `ext` crates (`reovim-domain-text`) into the editor core —
//! the editor-core crate itself never depends on `ext` (core/ext boundary).
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
    reovim_domain_text::{TextHandler, TextProjector},
    reovim_editor_core::{
        EditorInit, LauncherArgs,
        session::{BufferId, DomainAttachmentId, SessionState, WindowId},
    },
    reovim_lib_ds::Shared,
    reovim_server_rt::start_listener,
    reovim_system_kernel::{
        fs::raw_fd_control,
        log::log_sink_control,
        mm::install_lib_ds_alloc_backend,
        net::net_control,
        panic::panic_control,
        sched::{
            clock_control, install_lib_ds_sync_backend, sync_control, thread_control,
            thread_spawner,
        },
    },
    reovim_uapi::fs::RawFd,
};

/// Static text Domain singletons. Zero-sized structs; no heap needed.
static TEXT_HANDLER: TextHandler = TextHandler;
static TEXT_PROJECTOR: TextProjector = TextProjector;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_floor_linux_aarch64::entry;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_floor_linux_x86_64::entry;

entry!(|argc, argv, _envp| {
    // ── Install the platform vtable (SP02, AB12 write-once) ───────────────────
    // The composition root drives the install: this is the first statement of
    // the `entry!` closure, ahead of every `kabi::handle` read (the editor-core
    // boot, scheduler bridge, socket bridge, and fs bridge), so the no-read-before-install
    // invariant holds.
    // The result is ignored by construction — this is the sole process entry, so
    // a second install cannot occur here.
    let _ = reovim_platform_linux_native::install_platform();
    let _ = install_lib_ds_alloc_backend();
    let _ = install_lib_ds_sync_backend();

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

    // ── Boot the editor core ──────────────────────────────────────────────────
    let editor_core_args = LauncherArgs {
        clock: clock_control(),
        log: log_sink_control(),
        panic: panic_control(),
        thread: thread_control(),
        ..LauncherArgs::default()
    };
    let editor_core = match EditorInit::new(editor_core_args).boot() {
        Ok(k) => k,
        Err(_) => {
            write_stderr(b"reovim-server: editor core boot failed\n");
            return 1;
        }
    };

    // ── Register the text Domain ──────────────────────────────────────────────
    let domain_id = match editor_core.register_domain("text", &TEXT_HANDLER, &TEXT_PROJECTOR) {
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
    editor_core.setup_session(state);

    // ── Start the UDS listener ────────────────────────────────────────────────
    if start_listener(&editor_core, socket_path, net_control(), thread_spawner()).is_err() {
        write_stderr(b"reovim-server: start_listener failed\n");
        return 1;
    }

    // ── Park the process ──────────────────────────────────────────────────────
    // The listener accept loop runs on a background thread. The main thread
    // parks here so the process stays alive. A real server would listen for a
    // shutdown signal; the walking skeleton parks indefinitely (kill via signal).
    park_forever(&editor_core)
});

/// Parks the process by sleeping in a tight retry loop.
///
/// The walking skeleton has no graceful-shutdown signal yet; the process
/// is terminated externally (SIGTERM/SIGKILL from the launcher or test harness).
#[cold]
fn park_forever(editor_core: &Shared<reovim_editor_core::EditorCore>) -> i32 {
    // Keep a reference to the editor core alive so it is not dropped.
    let _ = Shared::clone(editor_core);
    // Park on a local sync word through the system-kernel scheduler bridge. The
    // lower provider may return early (for example signal delivery), so the
    // loop re-parks indefinitely.
    let word = core::sync::atomic::AtomicU32::new(0);
    let sync = sync_control();
    loop {
        sync.park(&word, 0);
    }
}

/// Writes `msg` to fd 2 (stderr). Best-effort; errors ignored.
fn write_stderr(msg: &[u8]) {
    let mut off = 0;
    while off < msg.len() {
        match raw_fd_control().write(RawFd::stderr(), &msg[off..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => off += n,
        }
    }
}
