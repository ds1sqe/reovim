//! Bounded service table for root-daemon boot supervision.
//!
//! This is not a userspace service manager yet. It records typed `/bin/init`
//! service requests and payload readiness publication so boot/service policy is
//! visible as kernel state instead of private rootd or payload branches.

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// Maximum retained service records.
pub const MAX_SERVICES: usize = 8;

/// Service lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceState {
    /// Empty table slot.
    Empty,
    /// A process requested this service, but it has not started yet.
    Requested,
    /// Rootd started the requested service target.
    Started,
    /// The service process exited normally.
    Exited,
    /// The service was explicitly stopped by an operator-facing control path.
    Stopped,
    /// Rootd failed to start the requested service target.
    Failed,
}

impl ServiceState {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Requested => "requested",
            Self::Started => "started",
            Self::Exited => "exited",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }
}

/// Retained lifecycle reason for a service row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceReason {
    /// Empty table slot.
    None,
    /// A process requested the service target.
    Requested,
    /// Rootd started the service target.
    Running,
    /// The retained service process exited normally.
    ProcessExited,
    /// The retained service process failed.
    ProcessFailed,
    /// The retained service process was explicitly killed.
    ProcessKilled,
    /// The retained service was explicitly stopped by an operator-facing path.
    OperatorStop,
    /// The service target could not be loaded.
    ExecLoadError,
    /// The service target exited through halt.
    Halt,
    /// The service target failed during startup.
    StartError,
}

impl ServiceReason {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Requested => "requested",
            Self::Running => "running",
            Self::ProcessExited => "process-exited",
            Self::ProcessFailed => "process-failed",
            Self::ProcessKilled => "process-killed",
            Self::OperatorStop => "operator-stop",
            Self::ExecLoadError => "exec-load-error",
            Self::Halt => "halt",
            Self::StartError => "start-error",
        }
    }
}

/// One retained service record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceRecord {
    /// Monotonic service sequence.
    pub seq: usize,
    /// Service name, for example `shell`.
    pub name: &'static str,
    /// Executable target selected for the service.
    pub target: &'static str,
    /// Process that requested the service.
    pub owner_pid: usize,
    /// Task that requested the service.
    pub owner_task_id: usize,
    /// Started or last retained service process, or `0` before the service starts.
    pub service_pid: usize,
    /// Started or last retained service task, or `0` before the service starts.
    pub service_task_id: usize,
    /// Retained lifecycle reason.
    pub reason: ServiceReason,
    /// Retained lifecycle state.
    pub state: ServiceState,
}

impl ServiceRecord {
    /// Empty service table slot.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            seq: 0,
            name: "",
            target: "",
            owner_pid: 0,
            owner_task_id: 0,
            service_pid: 0,
            service_task_id: 0,
            reason: ServiceReason::None,
            state: ServiceState::Empty,
        }
    }
}

/// Empty service record used for bounded snapshots.
pub const EMPTY_SERVICE_RECORD: ServiceRecord = ServiceRecord::empty();

/// Error while recording a service request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceError {
    /// Service name or target was empty.
    Empty,
    /// No running process context can own the request.
    Unavailable,
    /// The requesting process already owns retained syscall state.
    Busy,
    /// No free service slot remains.
    Full,
}

struct ServiceTable {
    records: [ServiceRecord; MAX_SERVICES],
}

impl ServiceTable {
    const fn new() -> Self {
        Self {
            records: [ServiceRecord::empty(); MAX_SERVICES],
        }
    }

    fn reset(&mut self) {
        self.records = [ServiceRecord::empty(); MAX_SERVICES];
    }

    fn find(&self, name: &str) -> Option<usize> {
        let mut index = 0usize;
        while index < self.records.len() {
            let record = self.records[index];
            if record.state != ServiceState::Empty && record.name == name {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn find_service_pid(&self, service_pid: usize) -> Option<usize> {
        if service_pid == 0 {
            return None;
        }
        let mut index = 0usize;
        while index < self.records.len() {
            let record = self.records[index];
            if record.state != ServiceState::Empty && record.service_pid == service_pid {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn empty_slot(&self) -> Option<usize> {
        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].state == ServiceState::Empty {
                return Some(index);
            }
            index += 1;
        }
        None
    }
}

struct TableCell(UnsafeCell<ServiceTable>);

// SAFETY: access is serialized by `LOCK`.
unsafe impl Sync for TableCell {}

static TABLE: TableCell = TableCell(UnsafeCell::new(ServiceTable::new()));
static LOCK: AtomicBool = AtomicBool::new(false);
static NEXT_SEQ: AtomicUsize = AtomicUsize::new(1);

struct Guard;

impl Guard {
    fn acquire() -> Self {
        while LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        LOCK.store(false, Ordering::Release);
    }
}

fn with_table<R>(f: impl FnOnce(&mut ServiceTable) -> R) -> R {
    let _guard = Guard::acquire();
    // SAFETY: `LOCK` serializes table access.
    let table = unsafe { &mut *TABLE.0.get() };
    f(table)
}

/// Resets retained service state.
pub fn reset() {
    NEXT_SEQ.store(1, Ordering::Release);
    with_table(ServiceTable::reset);
}

/// Records or updates a service request.
pub fn request_service(
    owner_pid: usize,
    owner_task_id: usize,
    name: &'static str,
    target: &'static str,
) -> Result<ServiceRecord, ServiceError> {
    if name.is_empty() || target.is_empty() {
        return Err(ServiceError::Empty);
    }
    let seq = NEXT_SEQ.fetch_add(1, Ordering::AcqRel);
    with_table(|table| {
        let slot = match table.find(name).or_else(|| table.empty_slot()) {
            Some(slot) => slot,
            None => return Err(ServiceError::Full),
        };
        let record = ServiceRecord {
            seq,
            name,
            target,
            owner_pid,
            owner_task_id,
            service_pid: 0,
            service_task_id: 0,
            reason: ServiceReason::Requested,
            state: ServiceState::Requested,
        };
        table.records[slot] = record;
        Ok(record)
    })
}

/// Marks a retained service as started by the supplied process/task.
pub fn mark_started(
    name: &str,
    service_pid: usize,
    service_task_id: usize,
) -> Option<ServiceRecord> {
    with_table(|table| {
        let slot = table.find(name)?;
        table.records[slot].service_pid = service_pid;
        table.records[slot].service_task_id = service_task_id;
        table.records[slot].reason = ServiceReason::Running;
        table.records[slot].state = ServiceState::Started;
        Some(table.records[slot])
    })
}

/// Marks a retained service as failed.
pub fn mark_failed(name: &str, reason: ServiceReason) -> Option<ServiceRecord> {
    with_table(|table| {
        let slot = table.find(name)?;
        table.records[slot].reason = reason;
        table.records[slot].state = ServiceState::Failed;
        Some(table.records[slot])
    })
}

/// Marks a retained service as normally exited by its service process.
///
/// The process/task ids stay on the row so post-mortem service snapshots can
/// identify the completed service instance.
pub fn mark_exited_by_service_pid(service_pid: usize) -> Option<ServiceRecord> {
    with_table(|table| {
        let slot = table.find_service_pid(service_pid)?;
        table.records[slot].reason = ServiceReason::ProcessExited;
        table.records[slot].state = ServiceState::Exited;
        Some(table.records[slot])
    })
}

/// Marks a retained service as failed by its running process.
///
/// The process/task ids stay on the row so post-mortem service snapshots can
/// identify the failed service instance.
pub fn mark_failed_by_service_pid(
    service_pid: usize,
    reason: ServiceReason,
) -> Option<ServiceRecord> {
    with_table(|table| {
        let slot = table.find_service_pid(service_pid)?;
        table.records[slot].reason = reason;
        table.records[slot].state = ServiceState::Failed;
        Some(table.records[slot])
    })
}

/// Marks a retained service as explicitly stopped.
///
/// The service process/task ids stay on the row so the stopped instance is
/// visible in post-mortem service snapshots.
pub fn mark_stopped(name: &str) -> Option<ServiceRecord> {
    with_table(|table| {
        let slot = table.find(name)?;
        table.records[slot].reason = ServiceReason::OperatorStop;
        table.records[slot].state = ServiceState::Stopped;
        Some(table.records[slot])
    })
}

/// Returns one retained service record by name.
#[must_use]
pub fn service(name: &str) -> Option<ServiceRecord> {
    with_table(|table| {
        let slot = table.find(name)?;
        Some(table.records[slot])
    })
}

/// Returns the target selected for a retained service.
#[must_use]
pub fn service_target(name: &str) -> Option<&'static str> {
    with_table(|table| {
        let slot = table.find(name)?;
        Some(table.records[slot].target)
    })
}

/// Copies retained service records into `out`, returning the count copied.
pub fn snapshot(out: &mut [ServiceRecord]) -> usize {
    with_table(|table| {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < table.records.len() && written < out.len() {
            if table.records[index].state != ServiceState::Empty {
                out[written] = table.records[index];
                written += 1;
            }
            index += 1;
        }
        written
    })
}

#[cfg(feature = "selftest")]
#[path = "service_tests.rs"]
mod tests;
