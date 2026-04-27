//! Subprocess composition: launcher spawns `reovim-server` and one
//! chosen client bin (`reovim-tui` / `reovim-cli` / `reovim-web`) as
//! separate OS processes wired together by the resolved transport.
//!
//! Layout of concerns, separated for test coverage:
//!
//! - [`CommandSpec`] — pure `(bin, args)` data describing a child to
//!   spawn. Built by [`build_commands`] from the validated
//!   [`LaunchMode`] + [`TransportChoice`] + [`ClientKind`] trio.
//! - [`ProcessSpawn`] — async trait seam that returns a
//!   [`ChildHandle`] (boxed), so orchestration code never touches
//!   real PIDs during unit tests.
//! - [`run_subprocess`] — the orchestration root. Spawns children,
//!   installs the `ctrl_c` handler, and returns the client's exit
//!   code after the server has been wound down. Marked
//!   `coverage(off)` because it drives real OS signals and real
//!   child processes; coverage is provided by the `build_commands`
//!   matrix tests and the seam-based orchestration tests.
//!
//! Windows signal handling is the [O-P2b-2] decision: escalate to
//! `child.kill()` after the grace period rather than mapping SIGINT
//! to `CTRL_C_EVENT` via `windows-sys`. Deeper Windows signal plumbing
//! can follow in a later issue.

use std::{io, process::ExitStatus, time::Duration};

use {
    async_trait::async_trait,
    tokio::process::{Child, Command},
};

use crate::{
    subprocess::ClientKind,
    transport::{LaunchMode, TransportChoice},
};

/// Grace period between SIGINT and SIGTERM escalation for a stuck
/// child. Matches the 5-second window documented in the 2b.E plan.
const SIGINT_GRACE: Duration = Duration::from_secs(5);

/// Describes one child process to spawn.
///
/// Pure data so [`build_commands`] can be unit-tested without
/// touching the OS. [`ProcessSpawn`] consumes this to produce a real
/// or mock child.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    /// Executable name — resolved against `$PATH` at spawn time.
    pub bin: &'static str,
    /// Arguments passed verbatim (no shell interpretation).
    pub args: Vec<String>,
}

impl CommandSpec {
    /// Build a real [`tokio::process::Command`] for this spec with
    /// inherited stdio.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_tokio_command(&self) -> Command {
        let mut cmd = Command::new(self.bin);
        cmd.args(&self.args);
        cmd
    }
}

/// Which child process this handle refers to. Used by the signal-
/// forwarding loop to log and escalate in the correct order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildRole {
    /// The `reovim-server` child.
    Server,
    /// The client child (`reovim-tui` / `reovim-cli` / `reovim-web`).
    Client,
}

/// Async trait seam over a spawned child process.
///
/// Production uses [`TokioChild`] wrapping a `tokio::process::Child`.
/// Tests use [`MockChild`] (in `subprocess_compose_tests.rs`) to
/// exercise orchestration without touching real PIDs.
#[async_trait]
pub trait ChildHandle: Send + Sync {
    /// OS process id, for signal forwarding. `None` when the child
    /// has already exited.
    fn pid(&self) -> Option<u32>;

    /// Which role this child plays in the composition.
    fn role(&self) -> ChildRole;

    /// Ask the OS to terminate the child immediately (SIGKILL /
    /// `TerminateProcess`). Used when the SIGINT grace period expires.
    ///
    /// # Errors
    ///
    /// Propagates any I/O error from the underlying kill call.
    async fn force_kill(&mut self) -> io::Result<()>;

    /// Wait for the child to exit and return its status.
    ///
    /// # Errors
    ///
    /// Propagates any I/O error raised while waiting.
    async fn wait(&mut self) -> io::Result<ExitStatus>;
}

/// Real-world spawner: uses `tokio::process::Command` with inherited
/// stdio (children share stdin/stdout/stderr with the launcher).
pub struct RealProcessSpawn;

#[async_trait]
impl ProcessSpawn for RealProcessSpawn {
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn spawn(&self, spec: &CommandSpec, role: ChildRole) -> io::Result<Box<dyn ChildHandle>> {
        let mut cmd = spec.to_tokio_command();
        let child = cmd.spawn()?;
        Ok(Box::new(TokioChild { child, role }))
    }
}

/// Trait seam for spawning children. Production wires
/// [`RealProcessSpawn`]; tests wire a recording mock that returns
/// pre-canned exit statuses.
#[async_trait]
pub trait ProcessSpawn: Send + Sync {
    /// Spawn the child described by `spec` in the `role` slot.
    ///
    /// # Errors
    ///
    /// Propagates any I/O error raised while spawning the child.
    async fn spawn(&self, spec: &CommandSpec, role: ChildRole) -> io::Result<Box<dyn ChildHandle>>;
}

/// Production [`ChildHandle`] wrapping a [`tokio::process::Child`].
struct TokioChild {
    child: Child,
    role: ChildRole,
}

#[async_trait]
impl ChildHandle for TokioChild {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    fn role(&self) -> ChildRole {
        self.role
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn force_kill(&mut self) -> io::Result<()> {
        self.child.kill().await
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait().await
    }
}

/// Build the list of [`CommandSpec`]s the launcher spawns for the
/// given `launch_mode` + `transport` + `client` + `external_grpc`
/// combination.
///
/// Order matters: the returned vector is server-first, client-second
/// on the happy path. For [`LaunchMode::ExternalGrpc`] and `--no-server`
/// (expressed as [`LaunchMode::Subprocess`] + `external_grpc = Some`
/// or equivalent; see [`run_subprocess`] for the dispatch rules) the
/// vector contains only the client.
#[must_use]
pub fn build_commands(
    launch_mode: LaunchMode,
    transport: &TransportChoice,
    client: ClientKind,
    external_grpc: Option<&str>,
) -> Vec<CommandSpec> {
    let mut specs = Vec::with_capacity(2);

    if launch_mode == LaunchMode::Subprocess && external_grpc.is_none() {
        specs.push(server_spec(transport));
    }

    specs.push(client_spec(client, transport, external_grpc));
    specs
}

/// Build the `reovim-server` command arguments for the resolved
/// transport. The server never speaks `Inproc` (rejected by
/// `TransportChoice::resolve`), but the arm is kept for exhaustiveness
/// and mirrors `TransportMode::to_server_mode`.
fn server_spec(transport: &TransportChoice) -> CommandSpec {
    let args = match transport {
        TransportChoice::Inproc => Vec::new(),
        TransportChoice::Pipe => vec!["--transport".into(), "pipe".into()],
        TransportChoice::Uds { path } => {
            vec!["--socket".into(), path.display().to_string()]
        }
        TransportChoice::Tcp { addr } => {
            vec!["--grpc".into(), addr.port().to_string()]
        }
    };
    CommandSpec {
        bin: "reovim-server",
        args,
    }
}

/// Build the client command for the given `client` + `transport` +
/// optional external gRPC target.
///
/// When `external_grpc` is `Some(addr)`, the TUI/CLI client is
/// pointed at that address (overrides the transport-derived connect
/// string). When it is `None`, the connect string is derived from
/// [`TransportChoice::to_client_connect_string`] for TCP and UDS;
/// Inproc/Pipe transports carry no connect string.
///
/// `reovim-web` does not accept a `--grpc` flag today (its runtime
/// is scaffold-only pending the SSR strategy), so no connect args
/// are passed for it; the launcher's contract evolves when web grows
/// a real CLI surface.
fn client_spec(
    client: ClientKind,
    transport: &TransportChoice,
    external_grpc: Option<&str>,
) -> CommandSpec {
    let bin = match client {
        ClientKind::Tui => "reovim-tui",
        ClientKind::Cli => "reovim-cli",
        ClientKind::Web => "reovim-web",
    };

    if matches!(client, ClientKind::Web) {
        return CommandSpec {
            bin,
            args: Vec::new(),
        };
    }

    let connect = external_grpc
        .map(str::to_owned)
        .or_else(|| transport.to_client_connect_string());

    let args = connect.map_or_else(Vec::new, |addr| vec!["--grpc".into(), addr]);

    CommandSpec { bin, args }
}

/// Orchestrate the subprocess-mode run.
///
/// Happy path:
///   1. Build command specs for `(launch_mode, transport, client,
///      external_grpc)`.
///   2. Spawn server (if applicable) and then the client, in that
///      order.
///   3. Install a `ctrl_c()` handler that forwards SIGINT to each
///      child in reverse spawn order (client first, server second).
///   4. Await the client. Return its exit code after the server has
///      been awaited (or force-killed after the grace period).
///
/// # Errors
///
/// Propagates any I/O error from spawning, signal forwarding, or
/// waiting on children.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn run_subprocess(
    launch_mode: LaunchMode,
    transport: TransportChoice,
    client: ClientKind,
    external_grpc: Option<String>,
) -> io::Result<i32> {
    let specs = build_commands(launch_mode, &transport, client, external_grpc.as_deref());
    run_subprocess_with(&RealProcessSpawn, &specs).await
}

/// Test-seamed orchestration core: exercise the spawn + wait + SIGINT
/// forwarding loop against an injectable [`ProcessSpawn`].
///
/// # Errors
///
/// Propagates any I/O error from spawning or waiting on children.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn run_subprocess_with(
    spawner: &dyn ProcessSpawn,
    specs: &[CommandSpec],
) -> io::Result<i32> {
    let mut children = spawn_all(spawner, specs).await?;
    let client_idx = find_client_index(&children)?;
    let client_status = wait_with_signal_forwarding(&mut children, client_idx).await?;
    shutdown_servers(&mut children).await?;
    Ok(exit_code_of(client_status))
}

/// Spawn every child described by `specs`, in order. The role is
/// derived from the bin name: `reovim-server` is the server slot,
/// anything else is a client.
///
/// # Errors
///
/// Propagates any I/O error from the underlying [`ProcessSpawn`].
pub async fn spawn_all(
    spawner: &dyn ProcessSpawn,
    specs: &[CommandSpec],
) -> io::Result<Vec<Box<dyn ChildHandle>>> {
    let mut children: Vec<Box<dyn ChildHandle>> = Vec::with_capacity(specs.len());
    for spec in specs {
        let role = if spec.bin == "reovim-server" {
            ChildRole::Server
        } else {
            ChildRole::Client
        };
        children.push(spawner.spawn(spec, role).await?);
    }
    Ok(children)
}

/// Locate the client child. The client is the one whose exit drives
/// the application's exit; failing to find one is a build-time
/// programming error that surfaces at runtime.
///
/// # Errors
///
/// Returns [`io::ErrorKind::InvalidInput`] when no child has the
/// [`ChildRole::Client`] role.
pub fn find_client_index(children: &[Box<dyn ChildHandle>]) -> io::Result<usize> {
    children
        .iter()
        .position(|c| c.role() == ChildRole::Client)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "subprocess composition requires a client child",
            )
        })
}

/// Wait for the client to exit; if SIGINT arrives first, forward it
/// to every child in reverse spawn order and escalate to SIGKILL
/// after [`SIGINT_GRACE`].
#[cfg_attr(coverage_nightly, coverage(off))]
async fn wait_with_signal_forwarding(
    children: &mut [Box<dyn ChildHandle>],
    client_idx: usize,
) -> io::Result<ExitStatus> {
    let (others, client_slice) = split_client_out(children, client_idx);
    let client = &mut client_slice[0];

    tokio::select! {
        res = client.wait() => res,
        sig = tokio::signal::ctrl_c() => {
            sig?;
            // Reverse spawn order: client first, then servers. On
            // unix we PID-signal; on windows force-kill drives the
            // escalation.
            #[cfg(unix)]
            if let Some(pid) = client.pid() {
                send_sigint(pid);
            }
            forward_sigint(others);
            await_with_escalation(client, others).await
        }
    }
}

/// Split `children` into the non-client children (everything before
/// `client_idx`) and a 1-element slice containing the client itself.
///
/// Relies on [`build_commands`] placing the client last; the client's
/// role is validated separately, and the caller treats anything after
/// the client as "pending non-client tail" that is merged back into
/// the others slice below.
type ChildSlice<'a> = &'a mut [Box<dyn ChildHandle>];

#[cfg_attr(coverage_nightly, coverage(off))]
fn split_client_out(
    children: &mut [Box<dyn ChildHandle>],
    client_idx: usize,
) -> (ChildSlice<'_>, ChildSlice<'_>) {
    let (before, after) = children.split_at_mut(client_idx);
    let (client_slice, _post) = after.split_at_mut(1);
    (before, client_slice)
}

/// Forward SIGINT to each non-client child. Unix only; on Windows the
/// escalation path is the only signalling primitive used (O-P2b-2).
#[cfg(unix)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn forward_sigint(children: &[Box<dyn ChildHandle>]) {
    for child in children.iter().rev() {
        if let Some(pid) = child.pid() {
            send_sigint(pid);
        }
    }
}

/// Windows: no SIGINT primitive; escalation handles termination.
#[cfg(not(unix))]
#[cfg_attr(coverage_nightly, coverage(off))]
fn forward_sigint(_children: &[Box<dyn ChildHandle>]) {}

/// Unix: send SIGINT via `libc::kill`.
///
/// `libc::kill` is an `unsafe` extern fn for historical reasons; the
/// call itself has no Rust memory-safety implications (it only sends
/// a signal to a PID). We ignore the return code: a missing child is
/// a benign race, not an orchestration error.
#[cfg(unix)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(unsafe_code)] // see module doc and fn doc: libc::kill is always safe
fn send_sigint(pid: u32) {
    #[allow(clippy::cast_possible_wrap)]
    let pid = pid as i32;
    // SAFETY: `libc::kill` is FFI; the call sends a signal to a pid
    // and returns an integer status. No Rust aliasing or lifetime
    // guarantees are involved.
    unsafe {
        libc::kill(pid, libc::SIGINT);
    }
}

/// After SIGINT has been forwarded, wait for the client to exit; if
/// it is still alive after [`SIGINT_GRACE`], escalate to
/// `force_kill()` on every non-exited child.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn await_with_escalation(
    client: &mut Box<dyn ChildHandle>,
    others: &mut [Box<dyn ChildHandle>],
) -> io::Result<ExitStatus> {
    if let Ok(res) = tokio::time::timeout(SIGINT_GRACE, client.wait()).await {
        res
    } else {
        // Grace window elapsed; force-kill everything.
        for child in others.iter_mut() {
            force_kill_if_alive(child.as_mut()).await?;
        }
        force_kill_if_alive(client.as_mut()).await?;
        client.wait().await
    }
}

/// After the client has exited, signal every server child and wait
/// for it to drain. Escalates to force-kill after [`SIGINT_GRACE`].
#[cfg_attr(coverage_nightly, coverage(off))]
async fn shutdown_servers(children: &mut [Box<dyn ChildHandle>]) -> io::Result<()> {
    for child in children.iter_mut() {
        if child.role() != ChildRole::Server {
            continue;
        }
        #[cfg(unix)]
        if let Some(pid) = child.pid() {
            send_sigint(pid);
        }
        if let Ok(res) = tokio::time::timeout(SIGINT_GRACE, child.wait()).await {
            res?;
        } else {
            force_kill_if_alive(child.as_mut()).await?;
            let _ = child.wait().await;
        }
    }
    Ok(())
}

/// Best-effort kill: ignore "already exited" races.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn force_kill_if_alive(child: &mut dyn ChildHandle) -> io::Result<()> {
    match child.force_kill().await {
        Ok(()) => Ok(()),
        // Already exited between `pid()` read and `kill()` syscall —
        // not an error from the orchestration's standpoint.
        Err(e) if e.kind() == io::ErrorKind::InvalidInput => Ok(()),
        Err(e) => Err(e),
    }
}

/// Map an `ExitStatus` to the integer exit code the launcher propagates
/// to its own caller. On Unix, signal-terminated processes return
/// `128 + signo` (the shell convention).
fn exit_code_of(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signo) = status.signal() {
            return 128 + signo;
        }
    }
    1
}

#[cfg(test)]
#[path = "subprocess_compose_tests.rs"]
mod subprocess_compose_tests;
