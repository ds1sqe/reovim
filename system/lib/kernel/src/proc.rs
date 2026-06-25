//! Minimal system-kernel process table for rootd and image programs.
//!
//! This is the first RTOS-itself process model: bounded, static, and suitable
//! for early boot before a block-backed executable loader exists.

use {
    crate::{
        program::LoadedProgram,
        sched::{self, BlockReason, KernelTaskState},
    },
    core::{
        cell::UnsafeCell,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

/// PID reserved for the root daemon.
pub const ROOTD_PID: usize = 1;
/// PID reserved for the interactive root shell session.
pub const SHELL_PID: usize = 2;
/// Maximum retained process records.
pub const MAX_PROCESSES: usize = 32;
/// Maximum retained process wait records.
pub const MAX_WAITS: usize = 32;

/// Process lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessState {
    /// Slot is unused.
    Empty,
    /// Process record exists but has not been scheduled.
    New,
    /// Process can run.
    Ready,
    /// Process is running.
    Running,
    /// Process is waiting for a kernel condition.
    Blocked,
    /// Process exited successfully.
    Exited,
    /// Process failed.
    Failed,
    /// Process requested halt.
    Halted,
}

impl ProcessState {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::New => "new",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Blocked => "blocked",
            Self::Exited => "exited",
            Self::Failed => "failed",
            Self::Halted => "halted",
        }
    }
}

/// One retained process-table record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessRecord {
    /// Process identifier.
    pub pid: usize,
    /// Parent process identifier.
    pub parent_pid: usize,
    /// Main task identifier.
    pub task_id: usize,
    /// Process state.
    pub state: ProcessState,
    /// Reason when the process is blocked.
    pub block_reason: BlockReason,
    /// Program path, or a kernel task name for root processes.
    pub program_path: &'static str,
    /// Loader/source kind for the executable image.
    pub loader: &'static str,
    /// Stable executable entry name.
    pub entry_name: &'static str,
    /// Numeric exit status.
    pub exit_code: i32,
}

/// One retained parent/child wait record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitRecord {
    /// Waiting parent process identifier.
    pub parent_pid: usize,
    /// Child process identifier being waited for.
    pub child_pid: usize,
    /// Child state observed when the wait completed.
    pub child_state: ProcessState,
    /// Child exit status observed when the wait completed.
    pub exit_code: i32,
    /// Whether the wait has observed child completion.
    pub completed: bool,
}

impl WaitRecord {
    /// Empty wait-table slot.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            parent_pid: 0,
            child_pid: 0,
            child_state: ProcessState::Empty,
            exit_code: 0,
            completed: false,
        }
    }
}

/// Empty wait record used for bounded snapshots.
pub const EMPTY_WAIT_RECORD: WaitRecord = WaitRecord::empty();

impl ProcessRecord {
    /// Empty process-table slot.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            pid: 0,
            parent_pid: 0,
            task_id: 0,
            state: ProcessState::Empty,
            block_reason: BlockReason::None,
            program_path: "",
            loader: "",
            entry_name: "",
            exit_code: 0,
        }
    }

    const fn kernel(
        pid: usize,
        parent_pid: usize,
        program_path: &'static str,
        entry_name: &'static str,
    ) -> Self {
        Self {
            pid,
            parent_pid,
            task_id: pid,
            state: ProcessState::Running,
            block_reason: BlockReason::None,
            program_path,
            loader: "kernel",
            entry_name,
            exit_code: 0,
        }
    }
}

/// Empty process record used for bounded snapshots.
pub const EMPTY_PROCESS_RECORD: ProcessRecord = ProcessRecord::empty();

/// Running program invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessHandle {
    /// Spawned process identifier.
    pub pid: usize,
    /// Main task identifier.
    pub task_id: usize,
    /// Program path.
    pub program_path: &'static str,
    /// Loader/source kind for the executable image.
    pub loader: &'static str,
    /// Stable executable entry name.
    pub entry_name: &'static str,
}

struct ProcessTable {
    records: [ProcessRecord; MAX_PROCESSES],
    waits: [WaitRecord; MAX_WAITS],
}

impl ProcessTable {
    const fn new() -> Self {
        let mut records = [ProcessRecord::empty(); MAX_PROCESSES];
        records[0] = ProcessRecord::kernel(ROOTD_PID, 0, "rootd", "rootd_main");
        records[1] = ProcessRecord::kernel(SHELL_PID, ROOTD_PID, "root-shell", "root_shell");
        Self {
            records,
            waits: [WaitRecord::empty(); MAX_WAITS],
        }
    }

    fn reset(&mut self) {
        self.records = [ProcessRecord::empty(); MAX_PROCESSES];
        self.waits = [WaitRecord::empty(); MAX_WAITS];
        self.records[0] = ProcessRecord::kernel(ROOTD_PID, 0, "rootd", "rootd_main");
        self.records[1] = ProcessRecord::kernel(SHELL_PID, ROOTD_PID, "root-shell", "root_shell");
    }

    fn write_program(&mut self, record: ProcessRecord) {
        let slot = 2 + ((record.pid.saturating_sub(3)) % (MAX_PROCESSES - 2));
        self.records[slot] = record;
    }

    fn update_state(
        &mut self,
        pid: usize,
        state: ProcessState,
        exit_code: i32,
        block_reason: BlockReason,
    ) -> Option<ProcessHandle> {
        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].pid == pid {
                self.records[index].state = state;
                self.records[index].exit_code = exit_code;
                self.records[index].block_reason = block_reason;
                return Some(ProcessHandle {
                    pid,
                    task_id: self.records[index].task_id,
                    program_path: self.records[index].program_path,
                    loader: self.records[index].loader,
                    entry_name: self.records[index].entry_name,
                });
            }
            index += 1;
        }
        None
    }

    fn update_task_state(
        &mut self,
        task_id: usize,
        state: ProcessState,
        exit_code: i32,
        block_reason: BlockReason,
    ) -> Option<ProcessHandle> {
        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].task_id == task_id {
                self.records[index].state = state;
                self.records[index].exit_code = exit_code;
                self.records[index].block_reason = block_reason;
                return Some(ProcessHandle {
                    pid: self.records[index].pid,
                    task_id,
                    program_path: self.records[index].program_path,
                    loader: self.records[index].loader,
                    entry_name: self.records[index].entry_name,
                });
            }
            index += 1;
        }
        None
    }

    fn find_process(&self, pid: usize) -> Option<ProcessRecord> {
        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].pid == pid {
                return Some(self.records[index]);
            }
            index += 1;
        }
        None
    }

    fn write_wait(&mut self, record: WaitRecord) {
        let slot = record.child_pid % MAX_WAITS;
        self.waits[slot] = record;
    }

    fn complete_wait(&mut self, parent_pid: usize, child: ProcessRecord) -> Option<WaitRecord> {
        let mut index = 0usize;
        while index < self.waits.len() {
            if self.waits[index].parent_pid == parent_pid
                && self.waits[index].child_pid == child.pid
            {
                self.waits[index].child_state = child.state;
                self.waits[index].exit_code = child.exit_code;
                self.waits[index].completed = true;
                return Some(self.waits[index]);
            }
            index += 1;
        }
        None
    }

    fn cancel_wait(&mut self, parent_pid: usize, child_pid: usize) -> Option<WaitRecord> {
        let mut index = 0usize;
        while index < self.waits.len() {
            if self.waits[index].parent_pid == parent_pid
                && self.waits[index].child_pid == child_pid
                && !self.waits[index].completed
            {
                let wait = self.waits[index];
                self.waits[index] = WaitRecord::empty();
                return Some(wait);
            }
            index += 1;
        }
        None
    }

    fn reparent_live_children(
        &mut self,
        parent_pid: usize,
        new_parent_pid: usize,
        task_ids: &mut [usize],
    ) -> usize {
        let mut count = 0usize;
        let mut index = 0usize;
        while index < self.records.len() && count < task_ids.len() {
            if self.records[index].parent_pid == parent_pid
                && matches!(
                    self.records[index].state,
                    ProcessState::New
                        | ProcessState::Ready
                        | ProcessState::Running
                        | ProcessState::Blocked
                )
            {
                self.records[index].parent_pid = new_parent_pid;
                task_ids[count] = self.records[index].task_id;
                count += 1;
            }
            index += 1;
        }
        count
    }
}

struct ProcessCell(UnsafeCell<ProcessTable>);

// SAFETY: mutable access is serialized by `LOCK`.
unsafe impl Sync for ProcessCell {}

static TABLE: ProcessCell = ProcessCell(UnsafeCell::new(ProcessTable::new()));
static LOCK: AtomicBool = AtomicBool::new(false);
static NEXT_PID: AtomicUsize = AtomicUsize::new(3);

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

fn with_table<R>(f: impl FnOnce(&mut ProcessTable) -> R) -> R {
    let _guard = Guard::acquire();
    // SAFETY: `LOCK` serializes access to the process table.
    let table = unsafe { &mut *TABLE.0.get() };
    f(table)
}

/// Resets the process table to rootd plus root-shell.
pub fn reset() {
    NEXT_PID.store(3, Ordering::Release);
    sched::reset_kernel_scheduler();
    with_table(ProcessTable::reset);
}

/// Creates a ready process record for an image-packaged program.
#[must_use]
pub fn spawn_program(parent_pid: usize, program: LoadedProgram) -> ProcessHandle {
    spawn_path(
        parent_pid,
        parent_pid,
        program.descriptor.path,
        program.image_kind.as_str(),
        program.descriptor.entry_name,
    )
}

/// Creates a ready child process record for a non-shell payload.
#[must_use]
pub fn spawn_child(
    parent_pid: usize,
    parent_task_id: usize,
    path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
) -> ProcessHandle {
    spawn_path(parent_pid, parent_task_id, path, loader, entry_name)
}

fn spawn_path(
    parent_pid: usize,
    parent_task_id: usize,
    path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
) -> ProcessHandle {
    let pid = NEXT_PID.fetch_add(1, Ordering::AcqRel);
    let task_id = sched::register_program_task(pid, parent_task_id, path);
    let record = ProcessRecord {
        pid,
        parent_pid,
        task_id,
        state: ProcessState::Ready,
        block_reason: BlockReason::None,
        program_path: path,
        loader,
        entry_name,
        exit_code: 0,
    };
    with_table(|table| table.write_program(record));
    ProcessHandle {
        pid,
        task_id,
        program_path: path,
        loader,
        entry_name,
    }
}

/// Marks a process and its main task as running.
#[must_use]
pub fn run_process(pid: usize) -> Option<ProcessHandle> {
    let handle =
        with_table(|table| table.update_state(pid, ProcessState::Running, 0, BlockReason::None));
    if let Some(handle) = handle {
        sched::run_kernel_task(handle.task_id);
    }
    handle
}

/// Dispatches the next scheduler-ready task and marks its process as running.
#[must_use]
pub fn dispatch_next_ready_process() -> Option<ProcessHandle> {
    let task = sched::dispatch_next_ready_task()?;
    with_table(|table| {
        table.update_task_state(task.task_id, ProcessState::Running, 0, BlockReason::None)
    })
}

/// Marks a process and its main task as blocked.
#[must_use]
pub fn block_process(pid: usize) -> Option<ProcessHandle> {
    block_process_with_reason(pid, BlockReason::Operator)
}

fn block_process_with_reason(pid: usize, reason: BlockReason) -> Option<ProcessHandle> {
    let handle = with_table(|table| table.update_state(pid, ProcessState::Blocked, 0, reason));
    if let Some(handle) = handle {
        sched::block_kernel_task(handle.task_id, reason);
    }
    handle
}

/// Blocks `parent_pid` waiting for `child_pid`.
#[must_use]
pub fn begin_wait(parent_pid: usize, child_pid: usize) -> Option<WaitRecord> {
    let _ = block_process_with_reason(parent_pid, BlockReason::WaitChild)?;
    let record = WaitRecord {
        parent_pid,
        child_pid,
        child_state: ProcessState::Running,
        exit_code: 0,
        completed: false,
    };
    with_table(|table| table.write_wait(record));
    Some(record)
}

/// Completes a parent/child wait and resumes the parent.
#[must_use]
pub fn finish_wait(parent_pid: usize, child_pid: usize) -> Option<WaitRecord> {
    let child = with_table(|table| table.find_process(child_pid))?;
    let wait = with_table(|table| table.complete_wait(parent_pid, child))?;
    let _ = wake_process(parent_pid);
    let _ = run_process(parent_pid);
    Some(wait)
}

/// Cancels an incomplete parent/child wait and resumes the parent.
#[must_use]
pub fn cancel_wait(parent_pid: usize, child_pid: usize) -> Option<WaitRecord> {
    let wait = with_table(|table| table.cancel_wait(parent_pid, child_pid))?;
    let _ = wake_process(parent_pid);
    let _ = run_process(parent_pid);
    Some(wait)
}

/// Wakes a blocked process and marks its main task ready.
#[must_use]
pub fn wake_process(pid: usize) -> Option<ProcessHandle> {
    let record = with_table(|table| table.find_process(pid))?;
    if record.state != ProcessState::Blocked {
        return None;
    }
    let handle =
        with_table(|table| table.update_state(pid, ProcessState::Ready, 0, BlockReason::None));
    if let Some(handle) = handle {
        sched::wake_kernel_task(handle.task_id);
    }
    handle
}

/// Cooperatively yields a process to the scheduler ready queue.
#[must_use]
pub fn yield_process(pid: usize) -> Option<ProcessHandle> {
    let record = with_table(|table| table.find_process(pid))?;
    if record.state != ProcessState::Running {
        return None;
    }
    let handle =
        with_table(|table| table.update_state(pid, ProcessState::Ready, 0, BlockReason::None))?;
    let scheduled_task_id = sched::yield_kernel_task(handle.task_id)?;
    with_table(|table| {
        table.update_task_state(scheduled_task_id, ProcessState::Running, 0, BlockReason::None)
    })
}

/// Marks a process as exited.
pub fn exit_process(pid: usize, state: ProcessState, exit_code: i32) {
    let mut reparented_tasks = [0usize; MAX_PROCESSES];
    let reparented_count = if pid == ROOTD_PID {
        0
    } else {
        with_table(|table| table.reparent_live_children(pid, ROOTD_PID, &mut reparented_tasks))
    };
    let mut reparent_index = 0usize;
    while reparent_index < reparented_count {
        sched::reparent_kernel_task(reparented_tasks[reparent_index], ROOTD_PID);
        reparent_index += 1;
    }

    let task_state = match state {
        ProcessState::Empty => KernelTaskState::Empty,
        ProcessState::New | ProcessState::Ready => KernelTaskState::Ready,
        ProcessState::Running => KernelTaskState::Running,
        ProcessState::Blocked => KernelTaskState::Blocked,
        ProcessState::Exited => KernelTaskState::Exited,
        ProcessState::Failed => KernelTaskState::Failed,
        ProcessState::Halted => KernelTaskState::Halted,
    };
    let task_id = with_table(|table| {
        let mut index = 0usize;
        while index < table.records.len() {
            if table.records[index].pid == pid {
                return table.records[index].task_id;
            }
            index += 1;
        }
        0
    });
    let _ = with_table(|table| table.update_state(pid, state, exit_code, BlockReason::None));
    if task_id != 0 {
        sched::complete_kernel_task(task_id, task_state);
    }
}

/// Copies retained wait records into `out`, returning the count copied.
pub fn snapshot_waits(out: &mut [WaitRecord]) -> usize {
    with_table(|table| {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < table.waits.len() && written < out.len() {
            if table.waits[index].parent_pid != 0 {
                out[written] = table.waits[index];
                written += 1;
            }
            index += 1;
        }
        written
    })
}

/// Returns one retained process record by PID.
#[must_use]
pub fn process(pid: usize) -> Option<ProcessRecord> {
    with_table(|table| table.find_process(pid))
}

/// Copies retained process records into `out`, returning the count copied.
pub fn snapshot(out: &mut [ProcessRecord]) -> usize {
    with_table(|table| {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < table.records.len() && written < out.len() {
            if table.records[index].state != ProcessState::Empty {
                out[written] = table.records[index];
                written += 1;
            }
            index += 1;
        }
        written
    })
}

#[cfg(feature = "selftest")]
#[path = "proc_tests.rs"]
mod tests;
