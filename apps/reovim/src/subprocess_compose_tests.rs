//! Unit tests for subprocess-mode orchestration.
//!
//! Two layers of coverage:
//!
//! 1. Pure command-construction matrix: `build_commands` for every
//!    `(launch_mode, transport, client, external_grpc)` combination
//!    that the plan's transport matrix permits. No spawning.
//! 2. Seam-level orchestration: `spawn_all` + `find_client_index` +
//!    `shutdown_servers` + `exit_code_of` driven through a mock
//!    `ProcessSpawn` that returns pre-canned child handles. This
//!    exercises the order-of-operations invariants (server spawned
//!    first, client's exit drives shutdown) without touching real
//!    PIDs or `tokio::signal::ctrl_c()`.

use std::{net::SocketAddr, path::PathBuf};

#[cfg(unix)]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[cfg(unix)]
use async_trait::async_trait;

use {
    super::*,
    crate::transport::{LaunchMode, TransportChoice},
};

// -----------------------------------------------------------------
// build_commands matrix
// -----------------------------------------------------------------

fn tcp_choice(port: u16) -> TransportChoice {
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    TransportChoice::Tcp { addr }
}

fn uds_choice(path: &str) -> TransportChoice {
    TransportChoice::Uds {
        path: PathBuf::from(path),
    }
}

#[test]
fn subprocess_tcp_tui_builds_server_then_client() {
    let specs = build_commands(LaunchMode::Subprocess, &tcp_choice(12540), ClientKind::Tui, None);
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0].bin, "reovim-server");
    assert_eq!(specs[0].args, vec!["--grpc".to_owned(), "12540".to_owned()]);
    assert_eq!(specs[1].bin, "reovim-tui");
    assert_eq!(specs[1].args, vec!["--grpc".to_owned(), "127.0.0.1:12540".to_owned()]);
}

#[test]
fn subprocess_uds_cli_builds_socket_flag() {
    let specs = build_commands(
        LaunchMode::Subprocess,
        &uds_choice("/tmp/reovim.sock"),
        ClientKind::Cli,
        None,
    );
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0].bin, "reovim-server");
    assert_eq!(specs[0].args, vec!["--socket".to_owned(), "/tmp/reovim.sock".to_owned()]);
    assert_eq!(specs[1].bin, "reovim-cli");
    assert_eq!(specs[1].args, vec!["--grpc".to_owned(), "/tmp/reovim.sock".to_owned()]);
}

#[test]
fn subprocess_pipe_tui_emits_transport_pipe_flag() {
    let specs =
        build_commands(LaunchMode::Subprocess, &TransportChoice::Pipe, ClientKind::Tui, None);
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0].bin, "reovim-server");
    assert_eq!(specs[0].args, vec!["--transport".to_owned(), "pipe".to_owned()]);
    // Pipe has no connect string — the client gets no --grpc flag.
    assert_eq!(specs[1].bin, "reovim-tui");
    assert!(specs[1].args.is_empty());
}

#[test]
fn external_grpc_skips_server_and_points_client_at_addr() {
    let specs = build_commands(
        LaunchMode::ExternalGrpc,
        &tcp_choice(12540),
        ClientKind::Tui,
        Some("remote.example:50051"),
    );
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].bin, "reovim-tui");
    assert_eq!(specs[0].args, vec!["--grpc".to_owned(), "remote.example:50051".to_owned()]);
}

#[test]
fn subprocess_with_external_grpc_also_skips_server() {
    // `--no-server` and `--external-grpc` both produce a launch mode
    // that skips the server spawn. Both collapse to the same shape
    // once `build_commands` sees `external_grpc = Some(_)`.
    let specs = build_commands(
        LaunchMode::Subprocess,
        &tcp_choice(0),
        ClientKind::Cli,
        Some("127.0.0.1:7777"),
    );
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].bin, "reovim-cli");
    assert_eq!(specs[0].args, vec!["--grpc".to_owned(), "127.0.0.1:7777".to_owned()]);
}

#[test]
fn subprocess_web_builds_web_bin_without_grpc_flag() {
    // `reovim-web` has no `--grpc` CLI surface today; the launcher
    // passes no connect args until web grows a real CLI.
    let specs = build_commands(LaunchMode::Subprocess, &tcp_choice(12540), ClientKind::Web, None);
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[1].bin, "reovim-web");
    assert!(specs[1].args.is_empty());
}

// -----------------------------------------------------------------
// exit_code_of
// -----------------------------------------------------------------

#[cfg(unix)]
#[test]
fn exit_code_of_handles_normal_exit() {
    use std::os::unix::process::ExitStatusExt;
    let status = ExitStatus::from_raw(0);
    assert_eq!(exit_code_of(status), 0);
}

#[cfg(unix)]
#[test]
fn exit_code_of_propagates_nonzero_code() {
    use std::os::unix::process::ExitStatusExt;
    let status = ExitStatus::from_raw(7 << 8);
    assert_eq!(exit_code_of(status), 7);
}

#[cfg(unix)]
#[test]
fn exit_code_of_maps_signal_to_128_plus_signo() {
    use std::os::unix::process::ExitStatusExt;
    // Low 7 bits = signal number on unix.
    let status = ExitStatus::from_raw(libc::SIGINT);
    assert_eq!(exit_code_of(status), 128 + libc::SIGINT);
}

// -----------------------------------------------------------------
// Seam-level orchestration via MockProcessSpawn / MockChild
// (unix-only because ExitStatus constructors and libc::SIGINT are
// platform-conditional)
// -----------------------------------------------------------------

#[cfg(unix)]
fn ok_status() -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus::from_raw(0)
}

#[cfg(unix)]
struct MockChild {
    role: ChildRole,
    pid: Option<u32>,
    exit_status: ExitStatus,
    kill_count: Arc<AtomicUsize>,
    wait_count: Arc<AtomicUsize>,
}

#[cfg(unix)]
#[async_trait]
impl ChildHandle for MockChild {
    fn pid(&self) -> Option<u32> {
        self.pid
    }

    fn role(&self) -> ChildRole {
        self.role
    }

    async fn force_kill(&mut self) -> io::Result<()> {
        self.kill_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.wait_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.exit_status)
    }
}

#[cfg(unix)]
struct MockSpawnCall {
    spec: CommandSpec,
    role: ChildRole,
}

#[cfg(unix)]
struct MockProcessSpawn {
    calls: Arc<Mutex<Vec<MockSpawnCall>>>,
    next_exit: Mutex<Vec<ExitStatus>>,
    kill_counts: Arc<Mutex<Vec<Arc<AtomicUsize>>>>,
    wait_counts: Arc<Mutex<Vec<Arc<AtomicUsize>>>>,
}

#[cfg(unix)]
impl MockProcessSpawn {
    fn new(exits: Vec<ExitStatus>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            // Reverse so `pop` yields FIFO order.
            next_exit: Mutex::new(exits.into_iter().rev().collect()),
            kill_counts: Arc::new(Mutex::new(Vec::new())),
            wait_counts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[cfg(unix)]
#[async_trait]
impl ProcessSpawn for MockProcessSpawn {
    async fn spawn(&self, spec: &CommandSpec, role: ChildRole) -> io::Result<Box<dyn ChildHandle>> {
        self.calls.lock().unwrap().push(MockSpawnCall {
            spec: spec.clone(),
            role,
        });
        let exit_status = self
            .next_exit
            .lock()
            .unwrap()
            .pop()
            .expect("test supplied enough exit statuses");
        let kill_count = Arc::new(AtomicUsize::new(0));
        let wait_count = Arc::new(AtomicUsize::new(0));
        self.kill_counts
            .lock()
            .unwrap()
            .push(Arc::clone(&kill_count));
        self.wait_counts
            .lock()
            .unwrap()
            .push(Arc::clone(&wait_count));
        // `pid()` returns `None` so the unix signal path is a no-op
        // — the mock never tries to `libc::kill` a real PID during
        // unit tests.
        Ok(Box::new(MockChild {
            role,
            pid: None,
            exit_status,
            kill_count,
            wait_count,
        }))
    }
}

#[cfg(unix)]
#[tokio::test]
async fn spawn_all_records_bin_and_role_for_each_spec() {
    let specs = vec![
        CommandSpec {
            bin: "reovim-server",
            args: vec!["--grpc".into(), "12540".into()],
        },
        CommandSpec {
            bin: "reovim-tui",
            args: vec!["--grpc".into(), "127.0.0.1:12540".into()],
        },
    ];

    let spawner = MockProcessSpawn::new(vec![ok_status(), ok_status()]);
    let children = spawn_all(&spawner, &specs).await.unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].role(), ChildRole::Server);
    assert_eq!(children[1].role(), ChildRole::Client);

    let summary = {
        let calls = spawner.calls.lock().unwrap();
        calls
            .iter()
            .map(|c| (c.spec.bin, c.role))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        summary,
        vec![
            ("reovim-server", ChildRole::Server),
            ("reovim-tui", ChildRole::Client),
        ]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn find_client_index_returns_last_client_position() {
    let specs = vec![
        CommandSpec {
            bin: "reovim-server",
            args: Vec::new(),
        },
        CommandSpec {
            bin: "reovim-cli",
            args: Vec::new(),
        },
    ];
    let spawner = MockProcessSpawn::new(vec![ok_status(), ok_status()]);
    let children = spawn_all(&spawner, &specs).await.unwrap();
    assert_eq!(find_client_index(&children).unwrap(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn find_client_index_errors_when_no_client() {
    let specs = vec![CommandSpec {
        bin: "reovim-server",
        args: Vec::new(),
    }];
    let spawner = MockProcessSpawn::new(vec![ok_status()]);
    let children = spawn_all(&spawner, &specs).await.unwrap();
    let err = find_client_index(&children).expect_err("no client present");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}

#[cfg(unix)]
#[tokio::test]
async fn shutdown_servers_waits_for_each_server_to_exit() {
    let specs = vec![
        CommandSpec {
            bin: "reovim-server",
            args: Vec::new(),
        },
        CommandSpec {
            bin: "reovim-tui",
            args: Vec::new(),
        },
    ];
    let spawner = MockProcessSpawn::new(vec![ok_status(), ok_status()]);
    let mut children = spawn_all(&spawner, &specs).await.unwrap();

    // Simulate the happy path: we never force-kill because the mock
    // `wait()` returns instantly.
    shutdown_servers(&mut children).await.unwrap();

    // Server slot had exactly one wait(); client was untouched here.
    let (waited_server, waited_client) = {
        let waits = spawner.wait_counts.lock().unwrap();
        (waits[0].load(Ordering::SeqCst), waits[1].load(Ordering::SeqCst))
    };
    assert_eq!(waited_server, 1, "server waited once");
    assert_eq!(waited_client, 0, "client untouched by shutdown_servers");

    let kill_server = {
        let kills = spawner.kill_counts.lock().unwrap();
        kills[0].load(Ordering::SeqCst)
    };
    assert_eq!(kill_server, 0, "server exited within grace, no force-kill");
}
