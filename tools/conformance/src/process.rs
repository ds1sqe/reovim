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

#[cfg(target_os = "linux")]
use nix::{
    errno::Errno,
    sys::signal::{Signal, killpg},
    unistd::{Pid, getpgid},
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
    /// Exact program plus arguments in invocation order. No shell expansion is
    /// involved; report construction rejects non-UTF-8 values rather than
    /// rendering them lossily.
    pub argv: Vec<OsString>,
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
    pub leader_pid: u32,
    /// Dedicated process-group identity on Linux. Other targets explicitly
    /// return `None` because this runner makes no portable group claim there.
    pub pgid: Option<u32>,
    pub group_identity_validated: bool,
    pub group_absent_after_reap: bool,
    /// True when the Linux group boundary was established before any command
    /// result was accepted. It covers descendants that remain in that group.
    pub descendants_owned: bool,
    /// True when a signal-zero group probe returned `ESRCH` after the leader
    /// was reaped. This does not claim ownership of a descendant that escaped
    /// the group with `setsid(2)` or `setpgid(2)`.
    pub descendants_reaped: bool,
    pub descendant_residue_observed: bool,
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
/// Linux children enter a dedicated process group before `exec`. The parent
/// verifies that the leader PID is the PGID, signals the entire group on a
/// limit/deadline or post-leader residue, reaps the leader, proves the group is
/// absent, and joins both readers before returning. Other targets retain the
/// bounded leader-only behavior and make no group/descendant claim.
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
pub fn run(
    spec: &CommandSpec,
    limits: ProcessLimits,
) -> io::Result<ProcessResult> {
    validate_limits(limits)?;
    let started_at = Instant::now();
    let execution_deadline_at =
        started_at.checked_add(limits.deadline).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "execution deadline overflow",
            )
        })?;
    let supervision_deadline_at = started_at
        .checked_add(limits.supervision_deadline)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "supervision deadline overflow",
            )
        })?;

    let mut argv = Vec::with_capacity(spec.arguments.len().saturating_add(1));
    argv.push(spec.program.as_os_str().to_owned());
    argv.extend(spec.arguments.iter().cloned());
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
    configure_process_group(&mut command);

    let (watchdog_sender, watchdog_receiver) = mpsc::channel();
    let watchdog = thread::Builder::new()
        .name("reovim-process-watchdog".to_owned())
        .spawn(move || watchdog(watchdog_receiver, supervision_deadline_at))?;
    let child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            disarm_spawn_watchdog(&watchdog_sender, watchdog)?;
            return Err(error);
        }
    };
    let leader_pid = child.id();
    #[cfg(target_os = "linux")]
    let pgid = Some(leader_pid);
    #[cfg(not(target_os = "linux"))]
    let pgid = None;
    let stdout = child.stdout.as_ref().map(|_| ());
    let stderr = child.stderr.as_ref().map(|_| ());
    let mut supervisor = Supervisor {
        child: Some(child),
        #[cfg(target_os = "linux")]
        leader_pid,
        pgid,
        group_identity_validated: false,
        group_absent_after_reap: false,
        descendants_owned: false,
        descendants_reaped: false,
        leader_reaped: false,
        stdout_reader: None,
        stderr_reader: None,
        watchdog: Some(watchdog),
        watchdog_sender: Some(watchdog_sender),
        #[cfg(target_os = "linux")]
        supervision_deadline_at,
    };
    supervisor.validate_group_identity()?;
    if stdout.is_none() || stderr.is_none() {
        return Err(io::Error::other("configured child pipe is missing"));
    }
    let stdout =
        supervisor.child_mut().stdout.take().ok_or_else(|| {
            io::Error::other("configured stdout pipe missing")
        })?;
    let stderr =
        supervisor.child_mut().stderr.take().ok_or_else(|| {
            io::Error::other("configured stderr pipe missing")
        })?;
    let stdout_overflow = Arc::new(AtomicBool::new(false));
    let stderr_overflow = Arc::new(AtomicBool::new(false));
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
        supervisor
            .signal_owned(&mut kill_attempted, &mut kill_raced_with_exit)?;
        supervisor.child_mut().wait()?
    };
    supervisor.mark_leader_reaped();
    supervisor.child.take();
    let descendant_residue_observed = supervisor
        .finish_group(&mut kill_attempted, &mut kill_raced_with_exit)?;

    let stdout = join_reader(
        supervisor.stdout_reader.take().expect("checked above"),
        "stdout",
    );
    let stderr = join_reader(
        supervisor.stderr_reader.take().expect("checked above"),
        "stderr",
    );
    // Join both before propagating either reader failure.
    let stdout = stdout?;
    let stderr = stderr?;
    let limit_observed_after_exit = termination_reason
        == TerminationReason::Exited
        && (stdout.overflow || stderr.overflow);
    if limit_observed_after_exit {
        termination_reason = if stdout.overflow {
            TerminationReason::StdoutLimit
        } else {
            TerminationReason::StderrLimit
        };
    }
    supervisor.disarm_watchdog()?;

    Ok(ProcessResult {
        argv,
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
        leader_pid,
        pgid: supervisor.pgid,
        group_identity_validated: supervisor.group_identity_validated,
        group_absent_after_reap: supervisor.group_absent_after_reap,
        descendants_owned: supervisor.descendants_owned,
        descendants_reaped: supervisor.descendants_reaped,
        descendant_residue_observed,
        reaped: supervisor.leader_reaped,
        readers_joined: true,
    })
}

fn validate_limits(limits: ProcessLimits) -> io::Result<()> {
    if limits.stdout_bytes == 0
        || limits.stderr_bytes == 0
        || limits.deadline.is_zero()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "process limits must be non-zero",
        ));
    }
    if limits.supervision_deadline <= limits.deadline {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "supervision deadline must exceed execution deadline",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
}

#[cfg(not(target_os = "linux"))]
const fn configure_process_group(_command: &mut Command) {}

fn disarm_spawn_watchdog(
    sender: &mpsc::Sender<WatchdogMessage>,
    watchdog: thread::JoinHandle<()>,
) -> io::Result<()> {
    sender.send(WatchdogMessage::Complete).map_err(|_| {
        io::Error::other("process watchdog stopped during spawn failure")
    })?;
    watchdog.join().map_err(|_| {
        io::Error::other("process watchdog panicked during spawn failure")
    })
}

/// Owns every post-spawn resource. An error cannot silently detach a child,
/// same-group descendant, or capture thread. Uncertain Linux group cleanup is
/// a checker invariant failure and aborts rather than returning silently.
#[allow(clippy::struct_excessive_bools)]
struct Supervisor {
    child: Option<std::process::Child>,
    #[cfg(target_os = "linux")]
    leader_pid: u32,
    pgid: Option<u32>,
    group_identity_validated: bool,
    group_absent_after_reap: bool,
    descendants_owned: bool,
    descendants_reaped: bool,
    leader_reaped: bool,
    stdout_reader: Option<thread::JoinHandle<Capture>>,
    stderr_reader: Option<thread::JoinHandle<Capture>>,
    watchdog: Option<thread::JoinHandle<()>>,
    watchdog_sender: Option<mpsc::Sender<WatchdogMessage>>,
    #[cfg(target_os = "linux")]
    supervision_deadline_at: Instant,
}

impl Supervisor {
    const fn child_mut(&mut self) -> &mut std::process::Child {
        self.child.as_mut().expect("live supervisor owns child")
    }

    #[cfg(target_os = "linux")]
    fn validate_group_identity(&mut self) -> io::Result<()> {
        let leader = linux_pid(self.leader_pid)?;
        let actual = getpgid(Some(leader)).map_err(nix_error)?;
        if actual != leader {
            return Err(io::Error::other(format!(
                "child group identity mismatch: leader {}, PGID {}",
                leader.as_raw(),
                actual.as_raw()
            )));
        }
        self.group_identity_validated = true;
        self.descendants_owned = true;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    const fn validate_group_identity(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn mark_leader_reaped(&mut self) {
        self.leader_reaped = true;
        if let Some(sender) = &self.watchdog_sender {
            let _ = sender.send(WatchdogMessage::LeaderReaped);
        }
    }

    #[cfg(target_os = "linux")]
    fn signal_owned(
        &self,
        kill_attempted: &mut bool,
        kill_raced_with_exit: &mut bool,
    ) -> io::Result<()> {
        if !self.group_identity_validated {
            return Err(io::Error::other(
                "refusing to signal an unvalidated process group",
            ));
        }
        *kill_attempted = true;
        match signal_group(self.pgid.expect("validated Linux group"))? {
            SignalOutcome::Sent => Ok(()),
            SignalOutcome::AlreadyAbsent => {
                *kill_raced_with_exit = true;
                Ok(())
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn signal_owned(
        &mut self,
        kill_attempted: &mut bool,
        kill_raced_with_exit: &mut bool,
    ) -> io::Result<()> {
        *kill_attempted = true;
        if let Err(error) = self.child_mut().kill() {
            if self.child_mut().try_wait()?.is_some() {
                *kill_raced_with_exit = true;
                Ok(())
            } else {
                Err(io::Error::new(
                    error.kind(),
                    format!("failed to kill still-running child: {error}"),
                ))
            }
        } else {
            Ok(())
        }
    }

    #[cfg(target_os = "linux")]
    fn finish_group(
        &mut self,
        kill_attempted: &mut bool,
        kill_raced_with_exit: &mut bool,
    ) -> io::Result<bool> {
        if !self.leader_reaped || !self.group_identity_validated {
            return Err(io::Error::other(
                "group cleanup began without reaped validated leader",
            ));
        }
        let pgid = self.pgid.expect("validated Linux group");
        let residue_observed = wait_for_group_absence(
            pgid,
            self.supervision_deadline_at,
            kill_attempted,
            kill_raced_with_exit,
        )?;
        self.group_absent_after_reap = true;
        self.descendants_reaped = true;
        Ok(residue_observed)
    }

    #[cfg(not(target_os = "linux"))]
    const fn finish_group(
        &mut self,
        _kill_attempted: &mut bool,
        _kill_raced_with_exit: &mut bool,
    ) -> io::Result<bool> {
        Ok(false)
    }

    fn disarm_watchdog(&mut self) -> io::Result<()> {
        if let Some(sender) = self.watchdog_sender.take() {
            send_watchdog_complete(&sender)?;
        }
        if let Some(watchdog) = self.watchdog.take() {
            join_watchdog(watchdog)?;
        }
        Ok(())
    }

    fn cleanup_for_drop(&mut self) -> io::Result<()> {
        let mut kill_attempted = false;
        let mut kill_raced_with_exit = false;
        self.cleanup_child(&mut kill_attempted, &mut kill_raced_with_exit)?;
        self.cleanup_group(&mut kill_attempted, &mut kill_raced_with_exit)?;
        join_optional_reader(self.stdout_reader.take(), "stdout")?;
        join_optional_reader(self.stderr_reader.take(), "stderr")?;
        self.disarm_watchdog()
    }

    fn cleanup_child(
        &mut self,
        kill_attempted: &mut bool,
        kill_raced_with_exit: &mut bool,
    ) -> io::Result<()> {
        if self.child.is_none() {
            return Ok(());
        }
        self.signal_child_for_cleanup(kill_attempted, kill_raced_with_exit)?;
        self.child_mut().wait()?;
        self.mark_leader_reaped();
        self.child.take();
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn signal_child_for_cleanup(
        &mut self,
        kill_attempted: &mut bool,
        kill_raced_with_exit: &mut bool,
    ) -> io::Result<()> {
        if self.group_identity_validated {
            return self.signal_owned(kill_attempted, kill_raced_with_exit);
        }
        let _ = self.child_mut().kill();
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    fn signal_child_for_cleanup(
        &mut self,
        kill_attempted: &mut bool,
        kill_raced_with_exit: &mut bool,
    ) -> io::Result<()> {
        self.signal_owned(kill_attempted, kill_raced_with_exit)
    }

    #[cfg(target_os = "linux")]
    fn cleanup_group(
        &mut self,
        kill_attempted: &mut bool,
        kill_raced_with_exit: &mut bool,
    ) -> io::Result<()> {
        if !self.group_identity_validated {
            return Err(io::Error::other(
                "Linux process-group identity remained uncertain",
            ));
        }
        if self.group_absent_after_reap {
            return Ok(());
        }
        self.finish_group(kill_attempted, kill_raced_with_exit)?;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    const fn cleanup_group(
        &mut self,
        _kill_attempted: &mut bool,
        _kill_raced_with_exit: &mut bool,
    ) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        if self.cleanup_for_drop().is_err() {
            std::process::abort();
        }
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SignalOutcome {
    Sent,
    AlreadyAbsent,
}

#[cfg(target_os = "linux")]
fn linux_pid(pid: u32) -> io::Result<Pid> {
    let raw = i32::try_from(pid).map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidData, "child PID exceeds i32")
    })?;
    Ok(Pid::from_raw(raw))
}

#[cfg(target_os = "linux")]
fn signal_group(pgid: u32) -> io::Result<SignalOutcome> {
    match killpg(linux_pid(pgid)?, Signal::SIGKILL) {
        Ok(()) => Ok(SignalOutcome::Sent),
        Err(Errno::ESRCH) => Ok(SignalOutcome::AlreadyAbsent),
        Err(error) => Err(nix_error(error)),
    }
}

#[cfg(target_os = "linux")]
fn mark_signal_race(
    pgid: u32,
    kill_raced_with_exit: &mut bool,
) -> io::Result<()> {
    if signal_group(pgid)? == SignalOutcome::AlreadyAbsent {
        *kill_raced_with_exit = true;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn group_present(pgid: u32) -> io::Result<bool> {
    match killpg(linux_pid(pgid)?, None::<Signal>) {
        Ok(()) => Ok(true),
        Err(Errno::ESRCH) => Ok(false),
        Err(error) => Err(nix_error(error)),
    }
}

#[cfg(target_os = "linux")]
fn wait_for_group_absence(
    pgid: u32,
    supervision_deadline_at: Instant,
    kill_attempted: &mut bool,
    kill_raced_with_exit: &mut bool,
) -> io::Result<bool> {
    let mut residue_observed = false;
    loop {
        if !group_present(pgid)? {
            return Ok(residue_observed);
        }
        residue_observed = true;
        *kill_attempted = true;
        mark_signal_race(pgid, kill_raced_with_exit)?;
        let remaining = remaining_supervision_time(supervision_deadline_at)?;
        thread::sleep(remaining.min(Duration::from_millis(1)));
    }
}

#[cfg(target_os = "linux")]
fn remaining_supervision_time(deadline_at: Instant) -> io::Result<Duration> {
    deadline_at.checked_duration_since(Instant::now()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "process group remained present at supervision deadline",
        )
    })
}

#[cfg(target_os = "linux")]
fn nix_error(error: Errno) -> io::Error {
    io::Error::from_raw_os_error(error as i32)
}

#[derive(Clone, Copy)]
enum WatchdogMessage {
    LeaderReaped,
    Complete,
}

fn send_watchdog_complete(
    sender: &mpsc::Sender<WatchdogMessage>,
) -> io::Result<()> {
    sender
        .send(WatchdogMessage::Complete)
        .map_err(|_| io::Error::other("process watchdog stopped before disarm"))
}

fn join_watchdog(watchdog: thread::JoinHandle<()>) -> io::Result<()> {
    watchdog.join().map_err(|_| io::Error::other("process watchdog panicked"))
}

#[allow(clippy::needless_pass_by_value)]
fn watchdog(receiver: mpsc::Receiver<WatchdogMessage>, deadline_at: Instant) {
    let mut leader_reaped = false;
    loop {
        let Some(remaining) =
            deadline_at.checked_duration_since(Instant::now())
        else {
            std::process::abort();
        };
        match receiver.recv_timeout(remaining) {
            Ok(WatchdogMessage::Complete) => return,
            Ok(WatchdogMessage::LeaderReaped) => leader_reaped = true,
            Err(_) => {
                let _ = leader_reaped;
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
    thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || capture_reader(&mut reader, limit, &overflow_signal))
}

fn capture_reader(
    reader: &mut impl Read,
    limit: usize,
    overflow_signal: &AtomicBool,
) -> Capture {
    let mut capture = Capture {
        retained: Vec::with_capacity(limit.min(16 * 1024)),
        total: 0,
        overflow: false,
        read_error: None,
    };
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        if !capture_reader_step(
            reader,
            &mut buffer,
            limit,
            overflow_signal,
            &mut capture,
        ) {
            return capture;
        }
    }
}

fn capture_reader_step(
    reader: &mut impl Read,
    buffer: &mut [u8],
    limit: usize,
    overflow_signal: &AtomicBool,
    capture: &mut Capture,
) -> bool {
    match reader.read(buffer) {
        Ok(0) => false,
        Ok(count) => {
            retain_capture(buffer, count, limit, overflow_signal, capture);
            true
        }
        Err(error) if error.kind() == io::ErrorKind::Interrupted => true,
        Err(error) => {
            capture.read_error = Some(error.to_string());
            false
        }
    }
}

fn retain_capture(
    buffer: &[u8],
    count: usize,
    limit: usize,
    overflow_signal: &AtomicBool,
    capture: &mut Capture,
) {
    capture.total = capture.total.saturating_add(count as u64);
    let remaining = limit.saturating_sub(capture.retained.len());
    capture.retained.extend_from_slice(&buffer[..count.min(remaining)]);
    if count > remaining {
        capture.overflow = true;
        overflow_signal.store(true, Ordering::Release);
    }
}

fn join_optional_reader(
    reader: Option<thread::JoinHandle<Capture>>,
    stream: &'static str,
) -> io::Result<()> {
    if let Some(reader) = reader {
        join_reader(reader, stream)?;
    }
    Ok(())
}

fn join_reader(
    handle: thread::JoinHandle<Capture>,
    stream: &'static str,
) -> io::Result<Capture> {
    handle.join().map_err(|_| {
        io::Error::other(format!("{stream} reader thread panicked"))
    })
}
