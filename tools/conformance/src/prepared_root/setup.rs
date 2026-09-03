//! User/mount/network/PID namespace setup and PID-1 supervision.

use std::{
    io::Read,
    process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    fcntl::{FcntlArg, OFlag, fcntl},
    mount::{MsFlags, mount},
    sched::{CloneFlags, unshare},
};

use super::{
    config_transport, diagnostic,
    linux::{
        MountStack, mount_exposures, mount_scratch, one_final_line,
        read_bounded, read_trimmed, require_configured_identities,
        status_field, validate_config, write_proc,
    },
    model::{
        Config, Error, PID1_ARGUMENT, Pid1Result, SetupCleanup, SetupFact,
        SetupResult,
    },
    protocol,
};

pub(super) fn run(config: &Config) -> Result<SetupResult, Error> {
    require_single_threaded()?;
    validate_config(config)?;
    require_current_checker(config)?;
    unshare(CloneFlags::CLONE_NEWUSER).map_err(namespace_error)?;
    install_identifier_maps(config)?;
    let namespaces = CloneFlags::CLONE_NEWNS
        | CloneFlags::CLONE_NEWNET
        | CloneFlags::CLONE_NEWPID;
    unshare(namespaces).map_err(namespace_error)?;
    make_mounts_private()?;

    let mut mounts = MountStack::default();
    mount_scratch(config, &mut mounts)?;
    let exposures = mount_exposures(config, &mut mounts)?;
    let pid1 = launch_pid1(config);
    let cleanup = mounts.cleanup();
    let post = require_configured_identities(config);
    cleanup.result?;
    post?;
    let pid1 = pid1?;
    let mut setup_cleanup = SetupCleanup::empty();
    setup_cleanup.record(SetupFact::ReadersJoined, true);
    setup_cleanup.record(SetupFact::ProcUnmounted, cleanup.proc_unmounted);
    setup_cleanup
        .record(SetupFact::ExposuresUnmounted, cleanup.exposures_unmounted);
    setup_cleanup
        .record(SetupFact::ScratchUnmounted, cleanup.scratch_unmounted);
    setup_cleanup.record(SetupFact::SourceIdentityPreserved, true);
    setup_cleanup.record(SetupFact::ToolIdentityPreserved, true);
    Ok(SetupResult { exposures, pid1, cleanup: setup_cleanup })
}

fn require_single_threaded() -> Result<(), Error> {
    let status = read_bounded("/proc/self/status", 64 * 1024)?;
    let text = std::str::from_utf8(&status).map_err(|failure| {
        Error::new("PREPARED_STATUS_UTF8", failure.to_string())
    })?;
    let threads = status_field(text, "Threads:")?;
    if threads == "1" {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_SETUP_NOT_SINGLE_THREADED",
        format!("setup process has {threads} threads"),
    ))
}

fn require_current_checker(config: &Config) -> Result<(), Error> {
    let current = std::env::current_exe()
        .and_then(|path| path.canonicalize())
        .map_err(|failure| {
            Error::new("PREPARED_CHECKER_IDENTITY", failure.to_string())
        })?;
    if current == config.checker_executable {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_CHECKER_IDENTITY",
        "hidden setup executable differs from its config identity",
    ))
}

fn install_identifier_maps(config: &Config) -> Result<(), Error> {
    write_proc("/proc/self/setgroups", b"deny\n")?;
    let uid = format!("0 {} 1\n", config.outside_uid);
    write_proc("/proc/self/uid_map", uid.as_bytes())?;
    let gid = format!("0 {} 1\n", config.outside_gid);
    write_proc("/proc/self/gid_map", gid.as_bytes())?;
    verify_identifier_maps(config)
}

pub(super) fn verify_identifier_maps(config: &Config) -> Result<(), Error> {
    identifier_maps(config).map(|_| ())
}

pub(super) fn identifier_maps(
    config: &Config,
) -> Result<(String, String), Error> {
    let uid = read_trimmed("/proc/self/uid_map", 1024)?;
    let gid = read_trimmed("/proc/self/gid_map", 1024)?;
    let setgroups = read_trimmed("/proc/self/setgroups", 64)?;
    let user_map = normalized_fields(&uid);
    let group_map = normalized_fields(&gid);
    let expected_user = format!("0 {} 1", config.outside_uid);
    let expected_group = format!("0 {} 1", config.outside_gid);
    if user_map == expected_user
        && group_map == expected_group
        && setgroups == "deny"
    {
        return Ok((user_map, group_map));
    }
    Err(Error::new(
        "PREPARED_IDENTIFIER_MAP",
        "user namespace maps or setgroups policy differ",
    ))
}

fn normalized_fields(value: &str) -> String {
    value.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
}

fn make_mounts_private() -> Result<(), Error> {
    mount(
        None::<&str>,
        "/",
        None::<&str>,
        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        None::<&str>,
    )
    .map_err(|failure| {
        Error::new("PREPARED_MOUNT_PRIVATE", failure.to_string())
    })
}

fn launch_pid1(config: &Config) -> Result<Pid1Result, Error> {
    let bytes = pid1_argument_payload(config)?;
    let text = std::str::from_utf8(&bytes).map_err(|failure| {
        Error::new("PREPARED_CONFIG_UTF8", failure.to_string())
    })?;
    let mut command = Command::new(&config.checker_executable);
    command
        .arg(PID1_ARGUMENT)
        .arg(text)
        .current_dir("/")
        .env_clear()
        .env("TERM", "dumb")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().map_err(|failure| {
        Error::new("PREPARED_PID1_SPAWN", failure.to_string())
    })?;
    Pid1Supervisor::new(child)?.finish(config.dispatch_bounds)
}

fn pid1_argument_payload(config: &Config) -> Result<Vec<u8>, Error> {
    config_transport::encode(config).map(|value| value.0)
}

#[cfg(test)]
pub(super) fn pid1_argument_payload_for_test(
    config: &Config,
) -> Result<Vec<u8>, Error> {
    pid1_argument_payload(config)
}

struct Pid1Supervisor {
    child: Option<Child>,
    stdout: ChildStdout,
    stderr: ChildStderr,
    stdout_bytes: Vec<u8>,
    stderr_bytes: Vec<u8>,
    stdout_eof: bool,
    stderr_eof: bool,
    ready: bool,
    status: Option<ExitStatus>,
}

impl Pid1Supervisor {
    fn new(mut child: Child) -> Result<Self, Error> {
        let (stdout, stderr) = take_pipes(&mut child)?;
        set_nonblocking(&stdout)
            .map_err(|failure| terminate_unowned(&mut child, failure))?;
        set_nonblocking(&stderr)
            .map_err(|failure| terminate_unowned(&mut child, failure))?;
        Ok(Self {
            child: Some(child),
            stdout,
            stderr,
            stdout_bytes: Vec::new(),
            stderr_bytes: Vec::new(),
            stdout_eof: false,
            stderr_eof: false,
            ready: false,
            status: None,
        })
    }

    fn finish(
        mut self,
        bounds: super::model::DispatchBounds,
    ) -> Result<Pid1Result, Error> {
        supervise_pid1(&mut self, bounds)?;
        let status = self.status.take().ok_or_else(missing_pid1_status)?;
        validate_status(status, self.ready, &self.stderr_bytes, bounds)?;
        let stdout = self
            .stdout_bytes
            .strip_prefix(protocol::ready_frame())
            .ok_or_else(malformed_ready)?;
        let final_line = one_final_line(stdout, "PREPARED_PID1_RESULT")?;
        protocol::parse_pid1_result(final_line, bounds)
    }

    fn terminate(&mut self) -> Result<(), Error> {
        let Some(child) = self.child.as_mut() else { return Ok(()) };
        match child.kill() {
            Ok(()) => Ok(()),
            Err(failure)
                if failure.kind() == std::io::ErrorKind::InvalidInput =>
            {
                Ok(())
            }
            Err(failure) => {
                Err(Error::new("PREPARED_PID1_KILL", failure.to_string()))
            }
        }
    }

    fn terminate_and_wait(&mut self) -> Result<(), Error> {
        self.terminate()?;
        let Some(child) = self.child.as_mut() else { return Ok(()) };
        let status =
            child.wait().map_err(|failure| pid1_wait_error(&failure))?;
        self.child.take();
        self.status = Some(status);
        Ok(())
    }
}

fn supervise_pid1(
    supervisor: &mut Pid1Supervisor,
    bounds: super::model::DispatchBounds,
) -> Result<(), Error> {
    let started = Instant::now();
    loop {
        let progress = supervision_step(supervisor, bounds.result_bytes)?;
        if supervisor_complete(supervisor) {
            return Ok(());
        }
        enforce_supervision_deadline(supervisor, started, bounds)?;
        if !progress {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

fn supervision_step(
    supervisor: &mut Pid1Supervisor,
    limit: usize,
) -> Result<bool, Error> {
    let stdout = read_pipe_once(
        &mut supervisor.stdout,
        &mut supervisor.stdout_bytes,
        &mut supervisor.stdout_eof,
        limit,
        "PID1 stdout",
    )?;
    supervisor.ready = validate_ready_prefix(&supervisor.stdout_bytes)?;
    let stderr = read_pipe_once(
        &mut supervisor.stderr,
        &mut supervisor.stderr_bytes,
        &mut supervisor.stderr_eof,
        limit,
        "PID1 stderr",
    )?;
    let status = observe_child_status(supervisor)?;
    Ok(stdout || stderr || status)
}

fn observe_child_status(
    supervisor: &mut Pid1Supervisor,
) -> Result<bool, Error> {
    let Some(child) = supervisor.child.as_mut() else { return Ok(false) };
    let status =
        child.try_wait().map_err(|failure| pid1_wait_error(&failure))?;
    let Some(status) = status else { return Ok(false) };
    supervisor.child.take();
    supervisor.status = Some(status);
    Ok(true)
}

const fn supervisor_complete(supervisor: &Pid1Supervisor) -> bool {
    supervisor.status.is_some()
        && supervisor.stdout_eof
        && supervisor.stderr_eof
}

fn enforce_supervision_deadline(
    supervisor: &mut Pid1Supervisor,
    started: Instant,
    bounds: super::model::DispatchBounds,
) -> Result<(), Error> {
    let elapsed = started.elapsed();
    if !supervisor.ready && elapsed >= bounds.execution_deadline / 2 {
        supervisor.terminate_and_wait()?;
        return Err(Error::new(
            "PREPARED_PID1_HANDSHAKE_DEADLINE",
            "PID1 did not become ready before its handshake deadline",
        ));
    }
    if elapsed >= bounds.execution_deadline {
        supervisor.terminate_and_wait()?;
        return Err(Error::new(
            "PREPARED_PID1_DEADLINE",
            "PID1 exceeded its execution deadline",
        ));
    }
    Ok(())
}

fn read_pipe_once<R: Read>(
    reader: &mut R,
    retained: &mut Vec<u8>,
    eof: &mut bool,
    limit: usize,
    name: &str,
) -> Result<bool, Error> {
    if *eof {
        return Ok(false);
    }
    let mut buffer = [0_u8; 8192];
    match reader.read(&mut buffer) {
        Ok(0) => {
            *eof = true;
            Ok(true)
        }
        Ok(count) => {
            append_bounded(retained, &buffer[..count], limit, name)?;
            Ok(true)
        }
        Err(failure)
            if matches!(
                failure.kind(),
                std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::Interrupted
            ) =>
        {
            Ok(false)
        }
        Err(failure) => {
            Err(Error::at("PREPARED_READER_IO", name, failure.to_string()))
        }
    }
}

fn append_bounded(
    retained: &mut Vec<u8>,
    bytes: &[u8],
    limit: usize,
    name: &str,
) -> Result<(), Error> {
    let length = retained.len().checked_add(bytes.len()).ok_or_else(|| {
        Error::at("PREPARED_READER_LIMIT", name, "byte count overflow")
    })?;
    if length > limit {
        return Err(Error::at(
            "PREPARED_READER_LIMIT",
            name,
            format!("protocol output exceeds {limit} bytes"),
        ));
    }
    retained.extend_from_slice(bytes);
    Ok(())
}

fn validate_ready_prefix(bytes: &[u8]) -> Result<bool, Error> {
    let expected = protocol::ready_frame();
    if bytes.len() < expected.len() && expected.starts_with(bytes) {
        return Ok(false);
    }
    if bytes.starts_with(expected) {
        return Ok(true);
    }
    Err(malformed_ready())
}

fn set_nonblocking<Fd: std::os::fd::AsFd>(fd: &Fd) -> Result<(), Error> {
    let current = fcntl(fd, FcntlArg::F_GETFL).map_err(pipe_flag_error)?;
    let flags = OFlag::from_bits_truncate(current) | OFlag::O_NONBLOCK;
    fcntl(fd, FcntlArg::F_SETFL(flags)).map(drop).map_err(pipe_flag_error)
}

fn pipe_flag_error(failure: Errno) -> Error {
    Error::new("PREPARED_READER_NONBLOCK", failure.to_string())
}

fn malformed_ready() -> Error {
    Error::new(
        "PREPARED_PID1_HANDSHAKE",
        "PID1 ready frame is missing or malformed",
    )
}

fn missing_pid1_status() -> Error {
    Error::new("PREPARED_PID1_WAIT", "PID1 exit status is absent")
}

fn pid1_wait_error(failure: &std::io::Error) -> Error {
    Error::new("PREPARED_PID1_WAIT", failure.to_string())
}

fn take_pipes(child: &mut Child) -> Result<(ChildStdout, ChildStderr), Error> {
    let stdout = child.stdout.take().ok_or_else(|| {
        Error::new("PREPARED_PID1_PIPE", "PID1 stdout pipe is absent")
    });
    let stdout = stdout.map_err(|failure| terminate_unowned(child, failure))?;
    let stderr = child.stderr.take().ok_or_else(|| {
        Error::new("PREPARED_PID1_PIPE", "PID1 stderr pipe is absent")
    });
    stderr
        .map(|stderr| (stdout, stderr))
        .map_err(|failure| terminate_unowned(child, failure))
}

fn terminate_unowned(child: &mut Child, failure: Error) -> Error {
    let _ = child.kill();
    let _ = child.wait();
    failure
}

impl Drop for Pid1Supervisor {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn validate_status(
    status: ExitStatus,
    ready: bool,
    stderr: &[u8],
    bounds: super::model::DispatchBounds,
) -> Result<(), Error> {
    if ready && status.success() && stderr.is_empty() {
        return Ok(());
    }
    let diagnostic = if stderr.is_empty() {
        "child_diagnostic=absent".to_owned()
    } else {
        diagnostic::child_error_summary(stderr, bounds)
    };
    Err(Error::new(
        "PREPARED_PID1_STATUS",
        format!("ready={ready}, status={status}; {diagnostic}"),
    ))
}

fn namespace_error(failure: Errno) -> Error {
    Error::new("PREPARED_NAMESPACE_UNAVAILABLE", failure.to_string())
}

#[cfg(test)]
pub(super) fn ready_prefix_for_test(bytes: &[u8]) -> Result<bool, Error> {
    validate_ready_prefix(bytes)
}

#[cfg(test)]
pub(super) fn append_bounded_for_test(
    retained: &mut Vec<u8>,
    bytes: &[u8],
    limit: usize,
) -> Result<(), Error> {
    append_bounded(retained, bytes, limit, "test pipe")
}

#[cfg(test)]
pub(super) fn read_once_for_test<R: Read>(
    reader: &mut R,
    retained: &mut Vec<u8>,
    eof: &mut bool,
    limit: usize,
) -> Result<bool, Error> {
    read_pipe_once(reader, retained, eof, limit, "test pipe")
}

#[cfg(all(test, unix))]
pub(super) fn validate_status_for_test(
    status: ExitStatus,
    ready: bool,
    stderr: &[u8],
    bounds: super::model::DispatchBounds,
) -> Result<(), Error> {
    validate_status(status, ready, stderr, bounds)
}
