//! UDS listener: `start_listener` binds the Unix-domain socket and spawns the
//! accept loop (SP9, §5.3).
//!
//! ## Thread model (pin-1, §5.3)
//!
//! One OS thread per accepted connection (blocking I/O, no poll/epoll). The
//! accept loop lives on its own thread so the calling thread is never blocked.
//! Per-connection threads are not joined — they run until the client
//! disconnects. Unjoinable threads are fine here because the connection loop is
//! terminal: it exits cleanly on any I/O error or protocol violation, and the
//! thread tears down naturally with the process.
//!
//! ## Boot stage 6 wiring (gap-2)
//!
//! Stage 6 in `editor/lib/core/src/boot.rs` is a structural stub that emits
//! `boot.stage.{start,ok}` and performs no policy work. The composition root
//! fills the stage-6 body by calling [`start_listener`] AFTER `EditorInit::boot`
//! returns, keeping the `EditorCore → ServerRuntime` dependency direction
//! illegal (the editor-core crate does not import this crate; only the composition
//! root imports both and wires them). Gap-2 is confirmed; the stub rule permits
//! a later phase to fill the body.

use {
    reovim_editor_core::EditorCore,
    reovim_lib_ds::Shared,
    reovim_uapi::{
        net::{NetControl, UnixListener},
        sched::{DetachedThreadSpawner, SpawnError},
    },
};

use crate::{carrier::run_connection, error::RuntimeError};

/// Maximum byte length of a UDS path on Linux (107 usable bytes + NUL = 108).
pub(crate) const UNIX_PATH_MAX: usize = 107;

/// Binds a Unix-domain socket at `path`, spawns the accept loop on a
/// background thread, and returns immediately.
///
/// Each accepted connection spawns its own thread running
/// [`run_connection`]. Connections are not joined; the thread exits on I/O
/// error or protocol violation.
///
/// # Errors
///
/// Returns [`RuntimeError::InvalidPath`] when `path` exceeds OS limits
/// and [`RuntimeError::Io`] when `bind` or the accept-loop thread spawn fails.
///
/// ```rust,no_run
/// // no_run: requires a live arch runtime + editor core.
/// ```
pub fn start_listener<S>(
    editor_core: &Shared<EditorCore>,
    path: &[u8],
    net: NetControl,
    spawner: S,
) -> Result<(), RuntimeError>
where
    S: DetachedThreadSpawner,
{
    // Unix abstract/pathname sockets cap the path at UNIX_PATH_MAX bytes on
    // Linux. Reject over-long paths before the syscall rather than
    // propagating an opaque errno.
    if path.len() > UNIX_PATH_MAX {
        return Err(RuntimeError::InvalidPath);
    }

    // Bind the UDS listener at `path`.
    let listener = UnixListener::bind(net, path).map_err(|e| RuntimeError::Io(e.code()))?;

    // Clone the editor-core handle before moving into the accept-loop closure.
    // Shared::clone is a refcount bump — it never fails.
    let editor_core_accept = Shared::clone(editor_core);

    // Spawn the accept loop on a background thread. The loop runs for the
    // lifetime of the server process.
    spawner
        .spawn_detached(move || {
            accept_loop(&listener, &editor_core_accept, spawner);
        })
        .map_err(map_spawn_error)?;

    Ok(())
}

const fn map_spawn_error(error: SpawnError) -> RuntimeError {
    match error {
        SpawnError::OutOfMemory => RuntimeError::Alloc,
        SpawnError::Refused(code) => RuntimeError::Io(code),
    }
}

/// Runs the accept loop: blocks on `UnixListener::accept` and spawns one
/// connection thread per client.
fn accept_loop<S>(listener: &UnixListener, editor_core: &Shared<EditorCore>, spawner: S)
where
    S: DetachedThreadSpawner,
{
    loop {
        let Ok(stream) = listener.accept() else {
            return; // listener closed or OS error → exit loop
        };
        // Clone the editor core for this connection's thread.
        let core = Shared::clone(editor_core);
        // Spawn; ignore spawn failures (out-of-memory or scheduler refusal) —
        // the connection is dropped and the loop continues. The closure owns
        // the stream so the fd closes when the connection thread exits.
        let _ = spawner.spawn_detached(move || {
            run_connection(&stream, &core);
        });
    }
}

// L12 layout: tests in sibling listener_tests.rs, declared in lib.rs.
