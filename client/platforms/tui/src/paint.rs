//! TUI paint loop: raw-`termios` entry, pre-exit registration, stdin/server
//! I/O loop.
//!
//! [`run`] is the `pub fn run(args)` platform contract entry (8.2 §1). It:
//!
//! 1. Registers the terminal-restore pre-exit hook via
//!    [`arch::panic::set_pre_exit_hook`] BEFORE entering raw mode (gap-7,
//!    8.2 §2). The pre-exit hook is a process-global write-once static; if
//!    already set (e.g. in tests), this step is skipped without error.
//! 2. Enters raw mode on stdin (`arch::term::RawMode::enter(0)`).
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
//! When stdin is not a TTY, `RawMode::enter(0)` returns `ENOTTY`. `run`
//! continues without raw mode in that case — the pipe-based DEV2 harness
//! reads bytes from the pipe without needing raw mode.
//!
//! ## Single-thread model (pin-1, rule of three)
//!
//! The walking skeleton uses a simple alternating loop: read one stdin byte,
//! send `SendInput`, wait for one `AttachEvent::Projection`, paint. Real
//! bidirectional muxing (poll/epoll or a second thread) is deferred.

use {
    reovim_arch::{
        panic::set_pre_exit_hook,
        sys::write,
        term::{ENOTTY, RawMode},
    },
    reovim_lib_ds::net::UnixStream,
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
///
/// let args = RunArgs { socket_path: b"/tmp/reovim.sock\0" };
/// assert_eq!(args.socket_path, b"/tmp/reovim.sock\0");
/// ```
#[derive(Clone, Copy)]
pub struct RunArgs {
    /// NUL-terminated Unix-domain socket path the server is listening on.
    pub socket_path: &'static [u8],
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

/// Stdin fd — the fd whose termios was placed into raw mode.
const STDIN_FD: i32 = 0;
/// Stdout fd — the fd we write ANSI frames to.
const STDOUT_FD: i32 = 1;

/// The pre-exit terminal-restore hook registered via
/// [`arch::panic::set_pre_exit_hook`]. Restores stdin to the saved
/// cooked-mode termios using a `TCSETS` ioctl via `arch`.
///
/// This function is registered ONCE before entering raw mode. When the
/// panic handler fires it calls this hook before writing its output so the
/// panic line appears in cooked mode.
///
/// The function is safe to call multiple times (each call re-issues the
/// restore ioctl with the saved settings; the second call is a no-op if the
/// terminal is already cooked).
fn restore_terminal_on_panic() {
    // Re-enter+drop a RawMode to get access to saved termios is not available
    // here (we don't have the saved state). Instead we use arch's ioctl to
    // request a SANE termios via TCSETS with the cooked defaults. The canonical
    // approach: read current termios and re-enable ICANON+ECHO.
    //
    // Since we have no saved reference here, we call tcgetattr + set sane
    // flags. This is best-effort — the hook is called in a terminal panic
    // context. Use the arch ioctl directly.
    use reovim_arch::sys::{
        ioctl,
        term::{ECHO, ICANON, TCGETS, TCSETS},
    };
    let mut t = reovim_arch::term::Termios::zeroed();
    // Read current (raw) termios.
    if ioctl(STDIN_FD, TCGETS, core::ptr::from_mut(&mut t).addr()).is_ok() {
        // Re-enable canonical mode and echo.
        t.c_lflag |= ICANON | ECHO;
        // Apply; ignore errors (best-effort at panic time).
        let _ = ioctl(STDIN_FD, TCSETS, core::ptr::from_ref(&t).addr());
    }
}

// ── paint helpers ─────────────────────────────────────────────────────────────

/// Writes `buf` to `fd`, looping over short writes. Best-effort on error.
fn write_all_fd(fd: i32, buf: &[u8]) {
    let mut off = 0;
    while off < buf.len() {
        match write(fd, &buf[off..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => off += n,
        }
    }
}

/// Composes an ANSI frame from `proj_bytes` and writes it to stdout.
fn paint_projection(proj_bytes: &[u8]) -> Result<(), RunError> {
    let frame = compose_ansi_frame(proj_bytes).map_err(|_| RunError::Compose)?;
    write_all_fd(STDOUT_FD, frame.as_slice());
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
///
/// let result = run(RunArgs { socket_path: b"/tmp/reovim.sock\0" });
/// let _ = result; // either Ok(()) or RunError::Connect etc.
/// ```
pub fn run(args: RunArgs) -> Result<(), RunError> {
    // ── 1. Register the pre-exit terminal-restore hook (8.2 §2) ───────
    // Write-once; silently skip if already registered (e.g. during tests).
    let _ = set_pre_exit_hook(restore_terminal_on_panic);

    // ── 2. Enter raw mode on stdin (non-tty stdin → ENOTTY → skip) ───────────
    let _raw_guard = match RawMode::enter(STDIN_FD) {
        Ok(guard) => Some(guard),
        Err(e) if e == ENOTTY => None, // pipe-based harness — continue without raw mode
        Err(_) => None,                // other error — best-effort, continue
    };

    // ── 3. Connect + handshake ────────────────────────────────────────────────
    let conn = connect_and_handshake(args.socket_path).map_err(|_| RunError::Connect)?;

    // ── 4. Read and paint the initial Projection notify (SP12) ───────────────
    let initial_proj = recv_notify_frame(&conn.stream).map_err(|_| RunError::Paint)?;
    paint_projection(initial_proj.as_slice())?;

    // ── 5. Stdin/notify loop ──────────────────────────────────────────────────
    run_loop(&conn.stream, conn.client_id)?;

    Ok(())
}

/// The main alternating loop: read one stdin byte → `SendInput` → wait for
/// `AttachEvent::Projection` → paint.
///
/// Exits cleanly on stdin EOF or server disconnect.
fn run_loop(stream: &UnixStream, client_id: u64) -> Result<(), RunError> {
    let mut stdin_buf = [0u8; 16]; // enough for an ANSI escape sequence
    loop {
        // Read stdin bytes (blocks until at least 1 byte).
        let n = match reovim_arch::sys::read(STDIN_FD, &mut stdin_buf) {
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
            paint_projection(proj.as_slice())?;
        }
    }
}

// L12: paint.rs has no sibling test file — its logic depends on the real
// runtime (raw mode, socket, stdin). Integration coverage arrives with the
// Phase 5 DEV2 harness.
