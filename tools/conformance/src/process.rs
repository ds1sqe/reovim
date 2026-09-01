//! Bounded, concurrent child-process capture without a shell.

use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::{self, Read},
    path::PathBuf,
    process::{Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

/// Independent hard limits for one child invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessLimits {
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub deadline: Duration,
    pub supervision_deadline: Duration,
}

/// An exact executable invocation. No shell is involved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub arguments: Vec<OsString>,
    pub current_dir: PathBuf,
    pub environment: BTreeMap<OsString, OsString>,
    pub clear_environment: bool,
}

/// Why a bounded process stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerminationReason {
    Exited,
    StdoutLimit,
    StderrLimit,
    Deadline,
}

impl TerminationReason {
    /// Stable report spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exited => "exited",
            Self::StdoutLimit => "stdout_limit",
            Self::StderrLimit => "stderr_limit",
            Self::Deadline => "deadline",
        }
    }
}

/// Fully reaped result from a bounded process.
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct ProcessResult {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub stdout_overflow: bool,
    pub stderr_overflow: bool,
    pub stdout_read_error: Option<String>,
    pub stderr_read_error: Option<String>,
    pub termination_reason: TerminationReason,
    pub kill_attempted: bool,
    pub kill_raced_with_exit: bool,
    pub limit_observed_after_exit: bool,
    pub reaped: bool,
    pub readers_joined: bool,
}

#[derive(Debug)]
struct Capture {
    retained: Vec<u8>,
    total: u64,
    overflow: bool,
    read_error: Option<String>,
}

/// Runs one child with concurrent hard-capped pipe capture.
///
/// On overflow or deadline, the child is killed (unless it won the exit race),
/// waited/reaped, and both reader threads are joined before this function
/// returns.
///
/// # Errors
///
/// Returns an error for invalid bounds or spawn/supervision failures. Pipe
/// reader failures are retained explicitly in [`ProcessResult`] for the
/// command-specific caller to reject after both readers join. Once spawned,
/// ownership cleanup runs before an error is returned.
///
/// # Panics
///
/// Panics only if the internal supervisor loses a reader handle after both
/// handles were installed; that condition indicates a checker invariant bug.
#[allow(clippy::too_many_lines)]
pub fn run(spec: &CommandSpec, limits: ProcessLimits) -> io::Result<ProcessResult> {
    if limits.stdout_bytes == 0 || limits.stderr_bytes == 0 || limits.deadline.is_zero() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "process limits must be non-zero"));
    }
    if limits.supervision_deadline <= limits.deadline {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "supervision deadline must exceed execution deadline",
        ));
    }
    let started_at = Instant::now();
    let execution_deadline_at = started_at.checked_add(limits.deadline).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "execution deadline overflow")
    })?;
    let supervision_deadline_at =
        started_at.checked_add(limits.supervision_deadline).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "supervision deadline overflow")
        })?;

    let mut command = Command::new(&spec.program);
    command
        .args(&spec.arguments)
        .current_dir(&spec.current_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if spec.clear_environment {
        command.env_clear();
    }
    command.envs(&spec.environment);

    let (watchdog_sender, watchdog_receiver) = mpsc::channel();
    let watchdog = thread::Builder::new()
        .name("reovim-process-watchdog".to_owned())
        .spawn(move || watchdog(watchdog_receiver, supervision_deadline_at))?;
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            watchdog_sender
                .send(WatchdogMessage::Complete)
                .map_err(|_| io::Error::other("process watchdog stopped during spawn failure"))?;
            watchdog
                .join()
                .map_err(|_| io::Error::other("process watchdog panicked during spawn failure"))?;
            return Err(error);
        }
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_overflow = Arc::new(AtomicBool::new(false));
    let stderr_overflow = Arc::new(AtomicBool::new(false));
    let mut supervisor = Supervisor {
        child: Some(child),
        stdout_reader: None,
        stderr_reader: None,
        watchdog: Some(watchdog),
        watchdog_sender: Some(watchdog_sender),
    };
    let stdout = stdout.ok_or_else(|| io::Error::other("configured stdout pipe missing"))?;
    let stderr = stderr.ok_or_else(|| io::Error::other("configured stderr pipe missing"))?;
    supervisor.stdout_reader = Some(spawn_reader(
        "reovim-stdout-reader",
        stdout,
        limits.stdout_bytes,
        Arc::clone(&stdout_overflow),
    )?);
    supervisor.stderr_reader = Some(spawn_reader(
        "reovim-stderr-reader",
        stderr,
        limits.stderr_bytes,
        Arc::clone(&stderr_overflow),
    )?);

    let mut termination_reason = TerminationReason::Exited;
    let mut observed_status = None;
    loop {
        if stdout_overflow.load(Ordering::Acquire) {
            termination_reason = TerminationReason::StdoutLimit;
            break;
        }
        if stderr_overflow.load(Ordering::Acquire) {
            termination_reason = TerminationReason::StderrLimit;
            break;
        }
        if Instant::now() >= execution_deadline_at {
            termination_reason = TerminationReason::Deadline;
            break;
        }
        if let Some(status) = supervisor.child_mut().try_wait()? {
            observed_status = Some(status);
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }

    let mut kill_attempted = false;
    let mut kill_raced_with_exit = false;
    let status = if let Some(status) = observed_status {
        status
    } else {
        kill_attempted = true;
        if let Err(error) = supervisor.child_mut().kill() {
            if let Some(status) = supervisor.child_mut().try_wait()? {
                kill_raced_with_exit = true;
                observed_status = Some(status);
            } else {
                return Err(io::Error::new(
                    error.kind(),
                    format!("failed to kill still-running child: {error}"),
                ));
            }
        }
        if let Some(status) = observed_status { status } else { supervisor.child_mut().wait()? }
    };
    supervisor.child.take();
    supervisor.mark_reaped();

    let stdout = join_reader(supervisor.stdout_reader.take().expect("checked above"), "stdout");
    let stderr = join_reader(supervisor.stderr_reader.take().expect("checked above"), "stderr");
    // Join both before propagating either reader failure.
    let stdout = stdout?;
    let stderr = stderr?;
    let limit_observed_after_exit =
        termination_reason == TerminationReason::Exited && (stdout.overflow || stderr.overflow);
    if limit_observed_after_exit {
        termination_reason = if stdout.overflow {
            TerminationReason::StdoutLimit
        } else {
            TerminationReason::StderrLimit
        };
    }
    supervisor.disarm_watchdog()?;

    Ok(ProcessResult {
        status,
        stdout: stdout.retained,
        stderr: stderr.retained,
        stdout_bytes: stdout.total,
        stderr_bytes: stderr.total,
        stdout_overflow: stdout.overflow,
        stderr_overflow: stderr.overflow,
        stdout_read_error: stdout.read_error,
        stderr_read_error: stderr.read_error,
        termination_reason,
        kill_attempted,
        kill_raced_with_exit,
        limit_observed_after_exit,
        reaped: true,
        readers_joined: true,
    })
}

/// Owns every post-spawn resource. An error cannot silently detach a child or
/// either capture thread: dropping the supervisor kills, waits, then joins.
struct Supervisor {
    child: Option<std::process::Child>,
    stdout_reader: Option<thread::JoinHandle<Capture>>,
    stderr_reader: Option<thread::JoinHandle<Capture>>,
    watchdog: Option<thread::JoinHandle<()>>,
    watchdog_sender: Option<mpsc::Sender<WatchdogMessage>>,
}

impl Supervisor {
    const fn child_mut(&mut self) -> &mut std::process::Child {
        self.child.as_mut().expect("live supervisor owns child")
    }

    fn mark_reaped(&self) {
        if let Some(sender) = &self.watchdog_sender {
            let _ = sender.send(WatchdogMessage::Reaped);
        }
    }

    fn disarm_watchdog(&mut self) -> io::Result<()> {
        if let Some(sender) = self.watchdog_sender.take() {
            sender
                .send(WatchdogMessage::Complete)
                .map_err(|_| io::Error::other("process watchdog stopped before disarm"))?;
        }
        if let Some(watchdog) = self.watchdog.take() {
            watchdog.join().map_err(|_| io::Error::other("process watchdog panicked"))?;
        }
        Ok(())
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if !matches!(child.try_wait(), Ok(Some(_))) {
                if child.kill().is_err() && child.try_wait().ok().flatten().is_none() {
                    std::process::abort();
                }
                // Waiting here is intentional: fail-loud ownership is
                // preferred to returning while a checker child is live.
                if child.wait().is_err() {
                    std::process::abort();
                }
            }
            self.mark_reaped();
        }
        if let Some(reader) = self.stdout_reader.take() {
            let _ = reader.join();
        }
        if let Some(reader) = self.stderr_reader.take() {
            let _ = reader.join();
        }
        if self.disarm_watchdog().is_err() {
            std::process::abort();
        }
    }
}

#[derive(Clone, Copy)]
enum WatchdogMessage {
    Reaped,
    Complete,
}

#[allow(clippy::needless_pass_by_value)]
fn watchdog(receiver: mpsc::Receiver<WatchdogMessage>, deadline_at: Instant) {
    let mut reaped = false;
    loop {
        let Some(remaining) = deadline_at.checked_duration_since(Instant::now()) else {
            std::process::abort();
        };
        match receiver.recv_timeout(remaining) {
            Ok(WatchdogMessage::Complete) => return,
            Ok(WatchdogMessage::Reaped) => reaped = true,
            Err(_) => {
                let _ = reaped;
                std::process::abort();
            }
        }
    }
}

fn spawn_reader<R>(
    name: &str,
    mut reader: R,
    limit: usize,
    overflow_signal: Arc<AtomicBool>,
) -> io::Result<thread::JoinHandle<Capture>>
where
    R: Read + Send + 'static,
{
    thread::Builder::new().name(name.to_owned()).spawn(move || {
        let mut retained = Vec::with_capacity(limit.min(16 * 1024));
        let mut total = 0_u64;
        let mut overflow = false;
        let mut buffer = [0_u8; 8 * 1024];
        let mut read_error = None;
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    total = total.saturating_add(count as u64);
                    let remaining = limit.saturating_sub(retained.len());
                    retained.extend_from_slice(&buffer[..count.min(remaining)]);
                    if count > remaining {
                        overflow = true;
                        overflow_signal.store(true, Ordering::Release);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    read_error = Some(error.to_string());
                    break;
                }
            }
        }
        Capture { retained, total, overflow, read_error }
    })
}

fn join_reader(handle: thread::JoinHandle<Capture>, stream: &'static str) -> io::Result<Capture> {
    handle.join().map_err(|_| io::Error::other(format!("{stream} reader thread panicked")))
}
