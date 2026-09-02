//! Bounded namespace-PID-1 descendant termination and reaping.

use std::{
    collections::BTreeSet,
    fs, thread,
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    sys::{
        signal::{Signal, kill},
        wait::{WaitPidFlag, WaitStatus, waitpid},
    },
    unistd::Pid,
};

use super::model::Error;

pub(super) fn terminate_and_reap(
    limit: usize,
    deadline: Duration,
) -> Result<bool, Error> {
    let started = Instant::now();
    let mut known = BTreeSet::new();
    let mut empty_rounds = 0_u8;
    loop {
        require_deadline(started, deadline)?;
        let children_absent = drain_wait(&mut known, limit)?;
        let descendants = descendant_pids(limit)?;
        remember_all(&descendants, &mut known, limit)?;
        signal_all(&descendants)?;
        if descendants.is_empty() && children_absent {
            empty_rounds = empty_rounds.saturating_add(1);
        } else {
            empty_rounds = 0;
        }
        if empty_rounds == 2 {
            return Ok(true);
        }
        thread::sleep(Duration::from_millis(1));
    }
}

fn descendant_pids(limit: usize) -> Result<Vec<Pid>, Error> {
    let entries = fs::read_dir("/proc").map_err(|failure| {
        Error::new("PREPARED_REAPER_PROC", failure.to_string())
    })?;
    let mut values = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|failure| {
            Error::new("PREPARED_REAPER_PROC", failure.to_string())
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(Error::new(
                "PREPARED_REAPER_PROC_UTF8",
                "procfs entry is not UTF-8",
            ));
        };
        let Ok(raw) = name.parse::<i32>() else { continue };
        if raw == 1 {
            continue;
        }
        if raw <= 0 {
            return Err(Error::new(
                "PREPARED_REAPER_PID",
                "procfs contains a non-positive process ID",
            ));
        }
        values.push(Pid::from_raw(raw));
        if values.len() > limit {
            return Err(Error::new(
                "PREPARED_REAPER_LIMIT",
                "descendant count exceeds its limit",
            ));
        }
    }
    values.sort_by_key(|value| value.as_raw());
    values.dedup();
    Ok(values)
}

fn remember_all(
    values: &[Pid],
    known: &mut BTreeSet<i32>,
    limit: usize,
) -> Result<(), Error> {
    for value in values {
        remember(*value, known, limit)?;
    }
    Ok(())
}

fn remember(
    value: Pid,
    known: &mut BTreeSet<i32>,
    limit: usize,
) -> Result<(), Error> {
    known.insert(value.as_raw());
    if known.len() <= limit {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_REAPER_LIMIT",
        "observed descendant identities exceed their limit",
    ))
}

fn signal_all(values: &[Pid]) -> Result<(), Error> {
    for value in values {
        match kill(*value, Signal::SIGKILL) {
            Ok(()) | Err(Errno::ESRCH) => {}
            Err(failure) => {
                return Err(Error::at(
                    "PREPARED_REAPER_SIGNAL",
                    value.as_raw().to_string(),
                    failure.to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn drain_wait(known: &mut BTreeSet<i32>, limit: usize) -> Result<bool, Error> {
    loop {
        let result = waitpid(None, Some(WaitPidFlag::WNOHANG));
        match result {
            Err(Errno::ECHILD) => return Ok(true),
            Err(failure) => {
                return Err(Error::new(
                    "PREPARED_REAPER_WAIT",
                    failure.to_string(),
                ));
            }
            Ok(WaitStatus::StillAlive) => return Ok(false),
            Ok(status) => remember(waited_pid(status)?, known, limit)?,
        }
    }
}

fn waited_pid(status: WaitStatus) -> Result<Pid, Error> {
    match status {
        WaitStatus::Exited(pid, _) | WaitStatus::Signaled(pid, _, _) => Ok(pid),
        _ => Err(Error::new(
            "PREPARED_REAPER_STATUS",
            format!("unexpected wait status {status:?}"),
        )),
    }
}

fn require_deadline(started: Instant, deadline: Duration) -> Result<(), Error> {
    if started.elapsed() < deadline {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_REAPER_DEADLINE",
        "descendant cleanup exceeded its deadline",
    ))
}
