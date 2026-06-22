//! TUI paint loop: raw terminal entry, pre-exit registration, stdin/server I/O
//! loop.
//!
//! [`run`] is the `pub fn run(args)` platform contract entry (8.2 §1). It:
//!
//! 1. Registers the terminal-restore pre-exit hook via
//!    the supplied `uapi/panic` control table BEFORE entering raw mode (gap-7,
//!    8.2 §2). The bridge below that table is process-global/write-once; if the
//!    hook is already set (e.g. in tests), this step is skipped without error.
//! 2. Enters raw mode on primary input through the supplied
//!    `uapi/terminal` control table.
//! 3. Connects to the server over UDS + completes the Hello/Attach handshake
//!    via [`carrier::connect_and_handshake`].
//! 4. Reads the initial `AttachEvent::Projection` notify (SP12).
//! 5. Composes an ANSI frame via [`frame::compose_ansi_frame`] and writes it
//!    to stdout (fd 1).
//! 6. Loops: select between stdin read and server notify. On a stdin byte,
//!    encode a `SendInput` and send it; on a server notify, compose + paint.
//!
//! ## Non-tty stdin (DEV2 pipe harness)
//!
//! When stdin is not a TTY, the terminal control table returns
//! [`TerminalError::NotTerminal`]. `run` continues without raw mode in that case
//! — the pipe-based DEV2 harness reads bytes from the pipe without needing raw
//! mode.
//!
//! ## Single-thread model (pin-1, rule of three)
//!
//! The walking skeleton uses a simple alternating loop: read one stdin byte,
//! send `SendInput`, wait for one `AttachEvent::Projection`, paint. Real
//! bidirectional muxing (poll/epoll or a second thread) is deferred.

use reovim_uapi::{
    fs::{RawFd, RawFdControl},
    net::{NetControl, UnixStream},
    panic::PanicControl,
    terminal::{RawModeRequest, RawModeToken, TerminalControl},
};

use crate::{
    carrier::{connect_and_handshake, recv_notify_frame, send_input},
    frame::compose_ansi_frame,
    input::{encode_raw_input_list, is_forwardable},
};

// ── RunArgs ───────────────────────────────────────────────────────────────────

/// Arguments for [`run`] (the 8.2 §1 platform entry).
///
/// ```rust
/// use reovim_platform_tui::paint::RunArgs;
/// use reovim_uapi::panic::{
///     Disposition, PanicConfigError, PanicControl, PanicFlushTarget, PanicRecord,
///     PreExitHookFn, RingTailProviderFn, StateRecordHookFn,
/// };
/// use reovim_uapi::terminal::{
///     RawModeRequest, RawModeToken, TerminalControl, TerminalError,
/// };
///
/// fn enter(_: RawModeRequest) -> Result<RawModeToken, TerminalError> {
///     Err(TerminalError::Unsupported)
/// }
/// fn restore(_: RawModeToken) -> Result<(), TerminalError> { Ok(()) }
/// fn restore_primary() {}
/// fn set_disposition(_: Disposition) -> Result<(), PanicConfigError> { Ok(()) }
/// fn set_tail(_: RingTailProviderFn) -> Result<(), PanicConfigError> { Ok(()) }
/// fn set_record(_: StateRecordHookFn) -> Result<(), PanicConfigError> { Ok(()) }
/// fn set_flush(_: PanicFlushTarget) -> Result<(), PanicConfigError> { Ok(()) }
/// fn set_pre_exit(_: PreExitHookFn) -> Result<(), PanicConfigError> { Ok(()) }
///
/// let terminal = TerminalControl::new(enter, restore, restore_primary);
/// let panic = PanicControl::new(
///     set_disposition,
///     set_tail,
///     set_record,
///     set_flush,
///     set_pre_exit,
/// );
/// let net = reovim_uapi::net::NetControl::noop();
/// let stdio = reovim_uapi::fs::RawFdControl::noop();
/// let args = RunArgs { socket_path: b"/tmp/reovim.sock\0", terminal, panic, net, stdio };
/// assert_eq!(args.socket_path, b"/tmp/reovim.sock\0");
/// ```
#[derive(Clone, Copy)]
pub struct RunArgs {
    /// NUL-terminated Unix-domain socket path the server is listening on.
    pub socket_path: &'static [u8],
    /// Product-facing terminal control table supplied by the composition root.
    pub terminal: TerminalControl,
    /// Product-facing panic/log control table supplied by the composition root.
    pub panic: PanicControl,
    /// Product-facing local networking control table supplied by the
    /// composition root.
    pub net: NetControl,
    /// Product-facing borrowed stdio fd control table supplied by the
    /// composition root.
    pub stdio: RawFdControl,
}

// ── RunError ──────────────────────────────────────────────────────────────────

/// Why [`run`] failed.
///
/// ```rust
/// use reovim_platform_tui::paint::RunError;
///
/// assert_ne!(RunError::Connect, RunError::Paint);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunError {
    /// Could not connect or complete the handshake.
    Connect,
    /// A paint frame could not be composed or written.
    Paint,
    /// A frame compose error (projection truncated or alloc).
    Compose,
}

// ── terminal-restore pre-exit hook (8.2 §2) ───────────────────────────

/// Stdin fd — the fd read by the TUI input loop.
const STDIN_FD: RawFd = RawFd::stdin();
/// Stdout fd — the fd we write ANSI frames to.
const STDOUT_FD: RawFd = RawFd::stdout();

pub(crate) struct RawModeGuard {
    terminal: TerminalControl,
    token: RawModeToken,
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = self.terminal.restore_raw_mode(self.token);
    }
}

#[must_use]
pub(crate) fn enter_raw_mode_guard(terminal: TerminalControl) -> Option<RawModeGuard> {
    match terminal.enter_raw_mode(RawModeRequest::primary_input()) {
        Ok(token) => Some(RawModeGuard { terminal, token }),
        Err(e) if e.is_absent_terminal() => None,
        Err(_) => None,
    }
}

// ── paint helpers ─────────────────────────────────────────────────────────────

/// Writes `buf` to `fd`, looping over short writes. Best-effort on error.
fn write_all_fd(stdio: RawFdControl, fd: RawFd, buf: &[u8]) {
    let mut off = 0;
    while off < buf.len() {
        match stdio.write(fd, &buf[off..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => off += n,
        }
    }
}

/// Composes an ANSI frame from `proj_bytes` and writes it to stdout.
fn paint_projection(stdio: RawFdControl, proj_bytes: &[u8]) -> Result<(), RunError> {
    let frame = compose_ansi_frame(proj_bytes).map_err(|_| RunError::Compose)?;
    write_all_fd(stdio, STDOUT_FD, frame.as_slice());
    Ok(())
}

// ── main loop ─────────────────────────────────────────────────────────────────

/// Runs the TUI client: connects, paints, reads stdin, loops.
///
/// This is the 8.2 §1 platform entry dispatched by the launcher.
///
/// # Errors
///
/// Returns [`RunError::Connect`] when the server connection or handshake
/// fails; [`RunError::Compose`] or [`RunError::Paint`] when a frame cannot be
/// rendered.
///
/// ```no_run
/// // no_run: requires a live arch runtime + server.
/// use reovim_platform_tui::paint::{RunArgs, RunError, run};
/// use reovim_uapi::panic::{
///     Disposition, PanicConfigError, PanicControl, PanicFlushTarget, PanicRecord,
///     PreExitHookFn, RingTailProviderFn, StateRecordHookFn,
/// };
/// use reovim_uapi::terminal::{
///     RawModeRequest, RawModeToken, TerminalControl, TerminalError,
/// };
///
/// fn enter(_: RawModeRequest) -> Result<RawModeToken, TerminalError> {
///     Err(TerminalError::Unsupported)
/// }
/// fn restore(_: RawModeToken) -> Result<(), TerminalError> { Ok(()) }
/// fn restore_primary() {}
/// fn set_disposition(_: Disposition) -> Result<(), PanicConfigError> { Ok(()) }
/// fn set_tail(_: RingTailProviderFn) -> Result<(), PanicConfigError> { Ok(()) }
/// fn set_record(_: StateRecordHookFn) -> Result<(), PanicConfigError> { Ok(()) }
/// fn set_flush(_: PanicFlushTarget) -> Result<(), PanicConfigError> { Ok(()) }
/// fn set_pre_exit(_: PreExitHookFn) -> Result<(), PanicConfigError> { Ok(()) }
///
/// let terminal = TerminalControl::new(enter, restore, restore_primary);
/// let panic = PanicControl::new(
///     set_disposition,
///     set_tail,
///     set_record,
///     set_flush,
///     set_pre_exit,
/// );
/// let result = run(RunArgs {
///     socket_path: b"/tmp/reovim.sock\0",
///     terminal,
///     panic,
///     net: reovim_uapi::net::NetControl::noop(),
///     stdio: reovim_uapi::fs::RawFdControl::noop(),
/// });
/// let _ = result; // either Ok(()) or RunError::Connect etc.
/// ```
pub fn run(args: RunArgs) -> Result<(), RunError> {
    // ── 1. Register the pre-exit terminal-restore hook (8.2 §2) ───────
    // Write-once; silently skip if already registered (e.g. during tests).
    let _ = args
        .panic
        .set_pre_exit_hook(args.terminal.restore_primary_input_raw_mode_fn);

    // ── 2. Enter raw mode on stdin (non-tty stdin → skip) ────────────────────
    let _raw_guard = enter_raw_mode_guard(args.terminal);

    // ── 3. Connect + handshake ────────────────────────────────────────────────
    let conn = connect_and_handshake(args.net, args.socket_path).map_err(|_| RunError::Connect)?;

    // ── 4. Read and paint the initial Projection notify (SP12) ───────────────
    let initial_proj = recv_notify_frame(&conn.stream).map_err(|_| RunError::Paint)?;
    paint_projection(args.stdio, initial_proj.as_slice())?;

    // ── 5. Stdin/notify loop ──────────────────────────────────────────────────
    run_loop(args.stdio, &conn.stream, conn.client_id)?;

    Ok(())
}

/// The main alternating loop: read one stdin byte → `SendInput` → wait for
/// `AttachEvent::Projection` → paint.
///
/// Exits cleanly on stdin EOF or server disconnect.
fn run_loop(stdio: RawFdControl, stream: &UnixStream, client_id: u64) -> Result<(), RunError> {
    let mut stdin_buf = [0u8; 16]; // enough for an ANSI escape sequence
    loop {
        // Read stdin bytes (blocks until at least 1 byte). STDIN is a
        // process-owned fd this client must NOT close, so it goes through the
        // injected borrowed-fd control table (never an owning file object).
        let n = match stdio.read(STDIN_FD, &mut stdin_buf) {
            Ok(0) | Err(_) => return Ok(()), // EOF or error → clean exit
            Ok(n) => n,
        };

        // Only forward bytes/sequences the skeleton handles.
        let payload = &stdin_buf[..n];
        if !payload.is_empty()
            && is_forwardable(payload[0])
            && let Some(encoded) = encode_raw_input_list(payload)
        {
            // Send; on error treat as disconnect.
            if send_input(stream, client_id, encoded.as_slice()).is_err() {
                return Ok(());
            }

            // Wait for the resulting Projection notify and paint it.
            let Ok(proj) = recv_notify_frame(stream) else {
                return Ok(()); // server closed → clean exit
            };
            paint_projection(stdio, proj.as_slice())?;
        }
    }
}

// L12: paint.rs has no sibling test file — its logic depends on the real
// runtime (raw mode, socket, stdin). Integration coverage arrives with the
// Phase 5 DEV2 harness.
