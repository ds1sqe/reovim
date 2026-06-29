//! Minimal system-kernel process table for rootd and image programs.
//!
//! This is the first RTOS-itself process model: bounded, static, and suitable
//! for early boot before a block-backed executable loader exists.

use {
    crate::{
        exec_artifact::{ExecArtifactBodyFormat, ExecArtifactBodyInnerFormat},
        mm,
        program::{
            LoadedProgram, MAX_PROGRAM_ARG_BYTES, MAX_PROGRAM_ARGS, MAX_PROGRAM_ENV_NAME_BYTES,
            MAX_PROGRAM_ENV_VALUE_BYTES, MAX_PROGRAM_ENVS, ProgramArgvBuffer, ProgramEnvBuffer,
            ProgramImageKind,
        },
        sched::{self, BlockReason, KernelTaskState},
        service, source_store,
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
/// Maximum retained argv entries in one process record.
pub const MAX_PROCESS_ARGS: usize = MAX_PROGRAM_ARGS;
/// Maximum retained bytes of one argv entry in one process record.
pub const MAX_PROCESS_ARG_BYTES: usize = MAX_PROGRAM_ARG_BYTES;
/// Maximum retained bytes of `argv[0]` in one process record.
pub const MAX_PROCESS_ARGV0_BYTES: usize = MAX_PROCESS_ARG_BYTES;
/// Maximum retained bytes of `argv[1]` in one process record.
pub const MAX_PROCESS_ARGV1_BYTES: usize = MAX_PROCESS_ARG_BYTES;
/// Maximum retained environment entries in one process record.
pub const MAX_PROCESS_ENVS: usize = MAX_PROGRAM_ENVS;
/// Maximum retained bytes of one environment variable name.
pub const MAX_PROCESS_ENV_NAME_BYTES: usize = MAX_PROGRAM_ENV_NAME_BYTES;
/// Maximum retained bytes of one environment variable value.
pub const MAX_PROCESS_ENV_VALUE_BYTES: usize = MAX_PROGRAM_ENV_VALUE_BYTES;

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
    /// Process was observed by a successful wait and is no longer waitable.
    Reaped,
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
            Self::Reaped => "reaped",
        }
    }
}

const fn process_state_can_run(state: ProcessState) -> bool {
    match state {
        ProcessState::Ready | ProcessState::Running => true,
        ProcessState::Empty
        | ProcessState::New
        | ProcessState::Blocked
        | ProcessState::Exited
        | ProcessState::Failed
        | ProcessState::Halted
        | ProcessState::Reaped => false,
    }
}

const fn process_state_can_block(state: ProcessState) -> bool {
    match state {
        ProcessState::Ready | ProcessState::Running => true,
        ProcessState::Empty
        | ProcessState::New
        | ProcessState::Blocked
        | ProcessState::Exited
        | ProcessState::Failed
        | ProcessState::Halted
        | ProcessState::Reaped => false,
    }
}

const fn process_state_can_replace_image(state: ProcessState) -> bool {
    match state {
        ProcessState::Running => true,
        ProcessState::Empty
        | ProcessState::New
        | ProcessState::Ready
        | ProcessState::Blocked
        | ProcessState::Exited
        | ProcessState::Failed
        | ProcessState::Halted
        | ProcessState::Reaped => false,
    }
}

const fn process_state_can_reclaim_slot(state: ProcessState) -> bool {
    match state {
        ProcessState::Empty
        | ProcessState::Exited
        | ProcessState::Failed
        | ProcessState::Halted
        | ProcessState::Reaped => true,
        ProcessState::New | ProcessState::Ready | ProcessState::Running | ProcessState::Blocked => {
            false
        }
    }
}

const fn process_state_retires_address_space(state: ProcessState) -> bool {
    match state {
        ProcessState::Exited
        | ProcessState::Failed
        | ProcessState::Halted
        | ProcessState::Reaped => true,
        ProcessState::Empty
        | ProcessState::New
        | ProcessState::Ready
        | ProcessState::Running
        | ProcessState::Blocked => false,
    }
}

const fn wait_record_can_reclaim_slot(record: WaitRecord) -> bool {
    record.parent_pid == 0 || record.completed
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
    /// Retained address-space identity for this process image.
    pub address_space_id: usize,
    /// Process-local image generation. Same-PID `execve` increments this.
    pub image_generation: usize,
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
    /// Executable body format retained for process diagnostics.
    pub artifact_body_format: ExecArtifactBodyFormat,
    /// Inner checked executable body contract, when any.
    pub artifact_body_inner_format: ExecArtifactBodyInnerFormat,
    /// Numeric exit status.
    pub exit_code: i32,
    /// Number of argv entries admitted for this process.
    pub argc: usize,
    /// Retained argv bytes.
    pub argv: [[u8; MAX_PROCESS_ARG_BYTES]; MAX_PROCESS_ARGS],
    /// Retained argv byte counts.
    pub argv_len: [usize; MAX_PROCESS_ARGS],
    /// Whether each retained argv value was truncated.
    pub argv_truncated: [bool; MAX_PROCESS_ARGS],
    /// Number of environment entries admitted for this process.
    pub envc: usize,
    /// Retained environment variable names.
    pub env_name: [[u8; MAX_PROCESS_ENV_NAME_BYTES]; MAX_PROCESS_ENVS],
    /// Retained environment variable name byte counts.
    pub env_name_len: [usize; MAX_PROCESS_ENVS],
    /// Retained environment variable values.
    pub env_value: [[u8; MAX_PROCESS_ENV_VALUE_BYTES]; MAX_PROCESS_ENVS],
    /// Retained environment variable value byte counts.
    pub env_value_len: [usize; MAX_PROCESS_ENVS],
    /// Whether each retained environment entry was truncated.
    pub env_truncated: [bool; MAX_PROCESS_ENVS],
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
            address_space_id: 0,
            image_generation: 0,
            state: ProcessState::Empty,
            block_reason: BlockReason::None,
            program_path: "",
            loader: "",
            entry_name: "",
            artifact_body_format: ExecArtifactBodyFormat::None,
            artifact_body_inner_format: ExecArtifactBodyInnerFormat::None,
            exit_code: 0,
            argc: 0,
            argv: [[0u8; MAX_PROCESS_ARG_BYTES]; MAX_PROCESS_ARGS],
            argv_len: [0; MAX_PROCESS_ARGS],
            argv_truncated: [false; MAX_PROCESS_ARGS],
            envc: 0,
            env_name: [[0u8; MAX_PROCESS_ENV_NAME_BYTES]; MAX_PROCESS_ENVS],
            env_name_len: [0; MAX_PROCESS_ENVS],
            env_value: [[0u8; MAX_PROCESS_ENV_VALUE_BYTES]; MAX_PROCESS_ENVS],
            env_value_len: [0; MAX_PROCESS_ENVS],
            env_truncated: [false; MAX_PROCESS_ENVS],
        }
    }

    const fn kernel(
        pid: usize,
        parent_pid: usize,
        program_path: &'static str,
        entry_name: &'static str,
    ) -> Self {
        let mut record = Self {
            pid,
            parent_pid,
            task_id: pid,
            address_space_id: pid,
            image_generation: 1,
            state: ProcessState::Running,
            block_reason: BlockReason::None,
            program_path,
            loader: "kernel",
            entry_name,
            artifact_body_format: ExecArtifactBodyFormat::None,
            artifact_body_inner_format: ExecArtifactBodyInnerFormat::None,
            exit_code: 0,
            argc: 1,
            argv: [[0u8; MAX_PROCESS_ARG_BYTES]; MAX_PROCESS_ARGS],
            argv_len: [0; MAX_PROCESS_ARGS],
            argv_truncated: [false; MAX_PROCESS_ARGS],
            envc: 0,
            env_name: [[0u8; MAX_PROCESS_ENV_NAME_BYTES]; MAX_PROCESS_ENVS],
            env_name_len: [0; MAX_PROCESS_ENVS],
            env_value: [[0u8; MAX_PROCESS_ENV_VALUE_BYTES]; MAX_PROCESS_ENVS],
            env_value_len: [0; MAX_PROCESS_ENVS],
            env_truncated: [false; MAX_PROCESS_ENVS],
        };
        record.write_argv0(program_path);
        record
    }

    const fn write_arg(&mut self, index: usize, arg: &str) {
        if index < MAX_PROCESS_ARGS {
            self.argv_len[index] = 0;
            self.argv_truncated[index] = false;
            let bytes = arg.as_bytes();
            while self.argv_len[index] < bytes.len()
                && self.argv_len[index] < self.argv[index].len()
            {
                self.argv[index][self.argv_len[index]] = bytes[self.argv_len[index]];
                self.argv_len[index] += 1;
            }
            self.argv_truncated[index] = self.argv_len[index] < bytes.len();
        }
    }

    const fn write_argv0(&mut self, argv0: &str) {
        self.write_arg(0, argv0);
    }

    fn write_program_argv(&mut self, argv: &ProgramArgvBuffer, fallback_argv0: &str) {
        if argv.argc() == 0 {
            self.write_arg(0, fallback_argv0);
            return;
        }

        let mut index = 0usize;
        while index < argv.argc() && index < MAX_PROCESS_ARGS {
            let arg = if index == 0 {
                argv.arg(0).unwrap_or(fallback_argv0)
            } else {
                argv.arg(index).unwrap_or("")
            };
            self.write_arg(index, arg);
            index += 1;
        }
    }

    fn write_env_entry(&mut self, index: usize, name: &str, value: &str) {
        if index >= MAX_PROCESS_ENVS {
            return;
        }

        self.env_name_len[index] = 0;
        self.env_value_len[index] = 0;
        self.env_truncated[index] = false;

        let name_bytes = name.as_bytes();
        while self.env_name_len[index] < name_bytes.len()
            && self.env_name_len[index] < self.env_name[index].len()
        {
            self.env_name[index][self.env_name_len[index]] = name_bytes[self.env_name_len[index]];
            self.env_name_len[index] += 1;
        }

        let value_bytes = value.as_bytes();
        while self.env_value_len[index] < value_bytes.len()
            && self.env_value_len[index] < self.env_value[index].len()
        {
            self.env_value[index][self.env_value_len[index]] =
                value_bytes[self.env_value_len[index]];
            self.env_value_len[index] += 1;
        }

        self.env_truncated[index] = self.env_name_len[index] < name_bytes.len()
            || self.env_value_len[index] < value_bytes.len();
    }

    fn write_program_env(&mut self, env: &ProgramEnvBuffer) {
        self.envc = env.envc();
        self.env_name = [[0u8; MAX_PROCESS_ENV_NAME_BYTES]; MAX_PROCESS_ENVS];
        self.env_name_len = [0; MAX_PROCESS_ENVS];
        self.env_value = [[0u8; MAX_PROCESS_ENV_VALUE_BYTES]; MAX_PROCESS_ENVS];
        self.env_value_len = [0; MAX_PROCESS_ENVS];
        self.env_truncated = [false; MAX_PROCESS_ENVS];

        let mut index = 0usize;
        while index < env.envc() && index < MAX_PROCESS_ENVS {
            self.write_env_entry(
                index,
                env.name(index).unwrap_or(""),
                env.value(index).unwrap_or(""),
            );
            index += 1;
        }
    }

    /// Retained argv text at `index`.
    #[must_use]
    pub fn argv(&self, index: usize) -> &str {
        if index >= MAX_PROCESS_ARGS {
            return "";
        }
        // Process argv originates from the root-shell tokenizer, process
        // descriptors, or static kernel labels, all already represented as str.
        unsafe { core::str::from_utf8_unchecked(&self.argv[index][..self.argv_len[index]]) }
    }

    /// Whether retained argv text at `index` was truncated.
    #[must_use]
    pub const fn argv_was_truncated(&self, index: usize) -> bool {
        index < MAX_PROCESS_ARGS && self.argv_truncated[index]
    }

    /// Retained `argv[0]` text.
    #[must_use]
    pub fn argv0(&self) -> &str {
        self.argv(0)
    }

    /// Retained `argv[1]` text.
    #[must_use]
    pub fn argv1(&self) -> &str {
        self.argv(1)
    }

    /// Retained environment variable name at `index`.
    #[must_use]
    pub fn env_name(&self, index: usize) -> &str {
        if index >= MAX_PROCESS_ENVS {
            return "";
        }
        // Process env originates from shell tokens already represented as str.
        unsafe { core::str::from_utf8_unchecked(&self.env_name[index][..self.env_name_len[index]]) }
    }

    /// Retained environment variable value at `index`.
    #[must_use]
    pub fn env_value(&self, index: usize) -> &str {
        if index >= MAX_PROCESS_ENVS {
            return "";
        }
        // Process env originates from shell tokens already represented as str.
        unsafe {
            core::str::from_utf8_unchecked(&self.env_value[index][..self.env_value_len[index]])
        }
    }

    /// Whether retained environment text at `index` was truncated.
    #[must_use]
    pub const fn env_was_truncated(&self, index: usize) -> bool {
        index < MAX_PROCESS_ENVS && self.env_truncated[index]
    }
}

/// Empty process record used for bounded snapshots.
pub const EMPTY_PROCESS_RECORD: ProcessRecord = ProcessRecord::empty();

const fn process_handle(record: ProcessRecord) -> ProcessHandle {
    ProcessHandle {
        pid: record.pid,
        task_id: record.task_id,
        program_path: record.program_path,
        loader: record.loader,
        entry_name: record.entry_name,
    }
}

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

/// Empty process handle used for bounded handoff arrays.
pub const EMPTY_PROCESS_HANDLE: ProcessHandle = ProcessHandle {
    pid: 0,
    task_id: 0,
    program_path: "",
    loader: "",
    entry_name: "",
};

/// Children reparented by one process exit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessAdoptionSnapshot {
    /// Number of valid adopted child handles.
    pub count: usize,
    /// Adopted child handles.
    pub children: [ProcessHandle; MAX_PROCESSES],
}

impl ProcessAdoptionSnapshot {
    /// Empty adoption snapshot.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            count: 0,
            children: [EMPTY_PROCESS_HANDLE; MAX_PROCESSES],
        }
    }
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

    fn program_slot_for_write(&self, pid: usize) -> Option<usize> {
        let preferred = 2 + ((pid.saturating_sub(3)) % (MAX_PROCESSES - 2));
        if process_state_can_reclaim_slot(self.records[preferred].state) {
            return Some(preferred);
        }

        let mut index = 2usize;
        while index < self.records.len() {
            if process_state_can_reclaim_slot(self.records[index].state) {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn has_program_slot(&self) -> bool {
        self.program_slot_for_write(NEXT_PID.load(Ordering::Acquire))
            .is_some()
    }

    fn write_program(&mut self, record: ProcessRecord) -> bool {
        let Some(slot) = self.program_slot_for_write(record.pid) else {
            return false;
        };
        self.records[slot] = record;
        true
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
                if state == ProcessState::Running
                    && !process_state_can_run(self.records[index].state)
                {
                    return None;
                }
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

    fn replace_image(
        &mut self,
        pid: usize,
        address_space_id: usize,
        path: &'static str,
        loader: &'static str,
        entry_name: &'static str,
        argv: &ProgramArgvBuffer,
        env: &ProgramEnvBuffer,
        fallback_argv0: &str,
        artifact_body_format: ExecArtifactBodyFormat,
        artifact_body_inner_format: ExecArtifactBodyInnerFormat,
    ) -> Option<ProcessHandle> {
        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].pid == pid {
                if !process_state_can_replace_image(self.records[index].state) {
                    return None;
                }
                self.records[index].state = ProcessState::Running;
                self.records[index].block_reason = BlockReason::None;
                self.records[index].address_space_id = address_space_id;
                self.records[index].image_generation =
                    self.records[index].image_generation.saturating_add(1);
                self.records[index].program_path = path;
                self.records[index].loader = loader;
                self.records[index].entry_name = entry_name;
                self.records[index].artifact_body_format = artifact_body_format;
                self.records[index].artifact_body_inner_format = artifact_body_inner_format;
                self.records[index].exit_code = 0;
                self.records[index].argc = argv.argc();
                self.records[index].argv = [[0u8; MAX_PROCESS_ARG_BYTES]; MAX_PROCESS_ARGS];
                self.records[index].argv_len = [0; MAX_PROCESS_ARGS];
                self.records[index].argv_truncated = [false; MAX_PROCESS_ARGS];
                self.records[index].write_program_argv(argv, fallback_argv0);
                self.records[index].write_program_env(env);
                return Some(ProcessHandle {
                    pid,
                    task_id: self.records[index].task_id,
                    program_path: path,
                    loader,
                    entry_name,
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

    fn wait_slot_for_write(&self, record: WaitRecord) -> Option<usize> {
        let preferred = record.child_pid % MAX_WAITS;
        if wait_record_can_reclaim_slot(self.waits[preferred]) {
            return Some(preferred);
        }

        let mut index = 0usize;
        while index < self.waits.len() {
            if self.waits[index].parent_pid == 0 {
                return Some(index);
            }
            index += 1;
        }

        index = 0usize;
        while index < self.waits.len() {
            if self.waits[index].completed {
                return Some(index);
            }
            index += 1;
        }

        None
    }

    fn has_wait_slot(&self, record: WaitRecord) -> bool {
        self.wait_slot_for_write(record).is_some()
    }

    fn write_wait(&mut self, record: WaitRecord) -> bool {
        let Some(slot) = self.wait_slot_for_write(record) else {
            return false;
        };
        self.waits[slot] = record;
        true
    }

    fn complete_wait(&mut self, parent_pid: usize, child: ProcessRecord) -> Option<WaitRecord> {
        if !matches!(
            child.state,
            ProcessState::Exited | ProcessState::Failed | ProcessState::Halted
        ) {
            return None;
        }
        let mut index = 0usize;
        while index < self.waits.len() {
            if self.waits[index].parent_pid == parent_pid
                && self.waits[index].child_pid == child.pid
            {
                self.waits[index].child_state = child.state;
                self.waits[index].exit_code = child.exit_code;
                self.waits[index].completed = true;
                let wait = self.waits[index];
                let _ = self.update_state(
                    child.pid,
                    ProcessState::Reaped,
                    child.exit_code,
                    BlockReason::None,
                );
                return Some(wait);
            }
            index += 1;
        }
        None
    }

    fn complete_wait_for_child(
        &mut self,
        child: ProcessRecord,
    ) -> Option<(ProcessRecord, WaitRecord)> {
        if !matches!(
            child.state,
            ProcessState::Exited | ProcessState::Failed | ProcessState::Halted
        ) {
            return None;
        }
        let mut index = 0usize;
        while index < self.waits.len() {
            if self.waits[index].child_pid == child.pid && !self.waits[index].completed {
                let parent = self.find_process(self.waits[index].parent_pid)?;
                self.waits[index].child_state = child.state;
                self.waits[index].exit_code = child.exit_code;
                self.waits[index].completed = true;
                let wait = self.waits[index];
                let _ = self.update_state(
                    child.pid,
                    ProcessState::Reaped,
                    child.exit_code,
                    BlockReason::None,
                );
                return Some((parent, wait));
            }
            index += 1;
        }
        None
    }

    fn wake_sleepers_due(&mut self, current_tick: usize, out: &mut [ProcessHandle]) -> usize {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < self.records.len() && written < out.len() {
            let record = self.records[index];
            if record.state == ProcessState::Blocked && record.block_reason == BlockReason::Sleep {
                if let Some(task) = sched::task(record.task_id) {
                    if task.state == KernelTaskState::Blocked
                        && task.block_reason == BlockReason::Sleep
                        && task.wake_tick != 0
                        && task.wake_tick <= current_tick
                    {
                        out[written] = ProcessHandle {
                            pid: record.pid,
                            task_id: record.task_id,
                            program_path: record.program_path,
                            loader: record.loader,
                            entry_name: record.entry_name,
                        };
                        written += 1;
                    }
                }
            }
            index += 1;
        }
        written
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

    fn cancel_incomplete_waits_for_parent(&mut self, parent_pid: usize) -> usize {
        let mut canceled = 0usize;
        let mut index = 0usize;
        while index < self.waits.len() {
            if self.waits[index].parent_pid == parent_pid && !self.waits[index].completed {
                self.waits[index] = WaitRecord::empty();
                canceled += 1;
            }
            index += 1;
        }
        canceled
    }

    fn find_wait(&self, parent_pid: usize, child_pid: usize) -> Option<WaitRecord> {
        let mut index = 0usize;
        while index < self.waits.len() {
            if self.waits[index].parent_pid == parent_pid
                && self.waits[index].child_pid == child_pid
            {
                return Some(self.waits[index]);
            }
            index += 1;
        }
        None
    }

    fn reparent_live_children(
        &mut self,
        parent_pid: usize,
        new_parent_pid: usize,
        children: &mut [ProcessHandle],
    ) -> usize {
        let mut count = 0usize;
        let mut index = 0usize;
        while index < self.records.len() && count < children.len() {
            if self.records[index].parent_pid == parent_pid
                && matches!(
                    self.records[index].state,
                    ProcessState::New
                        | ProcessState::Ready
                        | ProcessState::Running
                        | ProcessState::Blocked
                )
            {
                children[count] = ProcessHandle {
                    pid: self.records[index].pid,
                    task_id: self.records[index].task_id,
                    program_path: self.records[index].program_path,
                    loader: self.records[index].loader,
                    entry_name: self.records[index].entry_name,
                };
                self.records[index].parent_pid = new_parent_pid;
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
    mm::reset_address_spaces();
    sched::reset_kernel_scheduler();
    service::reset();
    with_table(ProcessTable::reset);
}

/// Rebinds the reserved shell-session process to the active shell image.
pub fn install_shell_session(
    program_path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
) {
    install_shell_session_with_body_metadata(
        program_path,
        loader,
        entry_name,
        ExecArtifactBodyFormat::None,
        ExecArtifactBodyInnerFormat::None,
    );
}

/// Rebinds the reserved shell-session process to the active shell image with
/// executable body metadata.
pub fn install_shell_session_with_body_metadata(
    program_path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
    artifact_body_format: ExecArtifactBodyFormat,
    artifact_body_inner_format: ExecArtifactBodyInnerFormat,
) {
    with_table(|table| {
        table.records[1] = ProcessRecord {
            pid: SHELL_PID,
            parent_pid: ROOTD_PID,
            task_id: SHELL_PID,
            address_space_id: SHELL_PID,
            image_generation: 1,
            state: ProcessState::Running,
            block_reason: BlockReason::None,
            program_path,
            loader,
            entry_name,
            artifact_body_format,
            artifact_body_inner_format,
            exit_code: 0,
            argc: 1,
            argv: [[0u8; MAX_PROCESS_ARG_BYTES]; MAX_PROCESS_ARGS],
            argv_len: [0; MAX_PROCESS_ARGS],
            argv_truncated: [false; MAX_PROCESS_ARGS],
            envc: 0,
            env_name: [[0u8; MAX_PROCESS_ENV_NAME_BYTES]; MAX_PROCESS_ENVS],
            env_name_len: [0; MAX_PROCESS_ENVS],
            env_value: [[0u8; MAX_PROCESS_ENV_VALUE_BYTES]; MAX_PROCESS_ENVS],
            env_value_len: [0; MAX_PROCESS_ENVS],
            env_truncated: [false; MAX_PROCESS_ENVS],
        };
        table.records[1].write_argv0(program_path);
    });
    mm::install_shell_address_space(program_path, loader, entry_name);
    sched::install_shell_session_task(program_path);
}

/// Builds the MM image mapping metadata for a loaded `/bin` program.
#[must_use]
pub(crate) fn address_space_image_for_program(program: LoadedProgram) -> mm::AddressSpaceImage {
    match program.image_kind {
        ProgramImageKind::SourceImage | ProgramImageKind::ReovimExecBody => {
            mm::AddressSpaceImage::source(
                program.source_path,
                program.source_bytes().len(),
                source_store::source_media_checksum32(program.source_bytes()),
            )
        }
        ProgramImageKind::LinkedBin => mm::AddressSpaceImage::linked(program.source_path),
    }
}

/// Creates a ready process record for an image-packaged program.
#[must_use]
pub fn spawn_program(parent_pid: usize, program: LoadedProgram) -> Option<ProcessHandle> {
    let image = address_space_image_for_program(program);
    let (artifact_body_format, artifact_body_inner_format) =
        crate::exec_artifact::program_body_formats(program);
    spawn_path(
        parent_pid,
        parent_pid,
        program.descriptor.path,
        program.image_kind.as_str(),
        program.descriptor.entry_name,
        image,
        1,
        program.descriptor.name,
        None,
        artifact_body_format,
        artifact_body_inner_format,
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
) -> Option<ProcessHandle> {
    spawn_path(
        parent_pid,
        parent_task_id,
        path,
        loader,
        entry_name,
        mm::AddressSpaceImage::metadata_only(path),
        1,
        path,
        None,
        ExecArtifactBodyFormat::None,
        ExecArtifactBodyInnerFormat::None,
    )
}

/// Creates a ready child process record with admitted argv metadata.
#[must_use]
pub fn spawn_child_with_argv(
    parent_pid: usize,
    parent_task_id: usize,
    path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
    argc: usize,
    argv0: &str,
    argv1: Option<&str>,
) -> Option<ProcessHandle> {
    spawn_path(
        parent_pid,
        parent_task_id,
        path,
        loader,
        entry_name,
        mm::AddressSpaceImage::metadata_only(path),
        argc,
        argv0,
        argv1,
        ExecArtifactBodyFormat::None,
        ExecArtifactBodyInnerFormat::None,
    )
}

/// Creates a ready child process record with all admitted argv metadata.
#[must_use]
pub fn spawn_child_with_program_argv(
    parent_pid: usize,
    parent_task_id: usize,
    path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
    argv: &ProgramArgvBuffer,
    fallback_argv0: &str,
) -> Option<ProcessHandle> {
    spawn_child_with_program_argv_mapped(
        parent_pid,
        parent_task_id,
        path,
        loader,
        entry_name,
        mm::AddressSpaceImage::metadata_only(path),
        argv,
        fallback_argv0,
    )
}

/// Creates a ready child process with argv plus explicit address-space image metadata.
#[must_use]
pub fn spawn_child_with_program_argv_mapped(
    parent_pid: usize,
    parent_task_id: usize,
    path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
    image: mm::AddressSpaceImage,
    argv: &ProgramArgvBuffer,
    fallback_argv0: &str,
) -> Option<ProcessHandle> {
    spawn_child_with_program_argv_env_mapped(
        parent_pid,
        parent_task_id,
        path,
        loader,
        entry_name,
        image,
        argv,
        &ProgramEnvBuffer::empty(),
        fallback_argv0,
        ExecArtifactBodyFormat::None,
        ExecArtifactBodyInnerFormat::None,
    )
}

/// Creates a ready child process with argv/env plus explicit address-space image metadata.
#[must_use]
pub fn spawn_child_with_program_argv_env_mapped(
    parent_pid: usize,
    parent_task_id: usize,
    path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
    image: mm::AddressSpaceImage,
    argv: &ProgramArgvBuffer,
    env: &ProgramEnvBuffer,
    fallback_argv0: &str,
    artifact_body_format: ExecArtifactBodyFormat,
    artifact_body_inner_format: ExecArtifactBodyInnerFormat,
) -> Option<ProcessHandle> {
    spawn_path_with_args(
        parent_pid,
        parent_task_id,
        path,
        loader,
        entry_name,
        image,
        argv.argc(),
        artifact_body_format,
        artifact_body_inner_format,
        |record| {
            record.write_program_argv(argv, fallback_argv0);
            record.write_program_env(env);
        },
    )
}

/// Replaces an existing process record with a newly loaded `/bin` image.
#[must_use]
pub fn replace_program_image(
    pid: usize,
    path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
    argv: &ProgramArgvBuffer,
    fallback_argv0: &str,
) -> Option<ProcessHandle> {
    replace_program_image_env_mapped(
        pid,
        path,
        loader,
        entry_name,
        mm::AddressSpaceImage::metadata_only(path),
        argv,
        &ProgramEnvBuffer::empty(),
        fallback_argv0,
        ExecArtifactBodyFormat::None,
        ExecArtifactBodyInnerFormat::None,
    )
}

/// Replaces an existing process record with explicit address-space image metadata.
#[must_use]
pub fn replace_program_image_mapped(
    pid: usize,
    path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
    image: mm::AddressSpaceImage,
    argv: &ProgramArgvBuffer,
    fallback_argv0: &str,
) -> Option<ProcessHandle> {
    replace_program_image_env_mapped(
        pid,
        path,
        loader,
        entry_name,
        image,
        argv,
        &ProgramEnvBuffer::empty(),
        fallback_argv0,
        ExecArtifactBodyFormat::None,
        ExecArtifactBodyInnerFormat::None,
    )
}

/// Replaces an existing process record with explicit image and environment metadata.
#[must_use]
pub fn replace_program_image_env_mapped(
    pid: usize,
    path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
    image: mm::AddressSpaceImage,
    argv: &ProgramArgvBuffer,
    env: &ProgramEnvBuffer,
    fallback_argv0: &str,
    artifact_body_format: ExecArtifactBodyFormat,
    artifact_body_inner_format: ExecArtifactBodyInnerFormat,
) -> Option<ProcessHandle> {
    let record = with_table(|table| table.find_process(pid))?;
    if !process_state_can_replace_image(record.state) {
        return None;
    }
    let _ = sched::replace_kernel_task_image(record.task_id, entry_name)?;
    let image_generation = record.image_generation.saturating_add(1);
    let address_space_id = mm::replace_process_address_space(
        pid,
        record.address_space_id,
        image_generation,
        path,
        loader,
        entry_name,
        image,
    )?;
    with_table(|table| {
        table.replace_image(
            pid,
            address_space_id,
            path,
            loader,
            entry_name,
            argv,
            env,
            fallback_argv0,
            artifact_body_format,
            artifact_body_inner_format,
        )
    })
}

fn spawn_path(
    parent_pid: usize,
    parent_task_id: usize,
    path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
    image: mm::AddressSpaceImage,
    argc: usize,
    argv0: &str,
    argv1: Option<&str>,
    artifact_body_format: ExecArtifactBodyFormat,
    artifact_body_inner_format: ExecArtifactBodyInnerFormat,
) -> Option<ProcessHandle> {
    spawn_path_with_args(
        parent_pid,
        parent_task_id,
        path,
        loader,
        entry_name,
        image,
        argc,
        artifact_body_format,
        artifact_body_inner_format,
        |record| {
            record.write_argv0(argv0);
            if let Some(argv1) = argv1 {
                record.write_arg(1, argv1);
            }
        },
    )
}

fn spawn_path_with_args(
    parent_pid: usize,
    parent_task_id: usize,
    path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
    image: mm::AddressSpaceImage,
    argc: usize,
    artifact_body_format: ExecArtifactBodyFormat,
    artifact_body_inner_format: ExecArtifactBodyInnerFormat,
    write_args: impl FnOnce(&mut ProcessRecord),
) -> Option<ProcessHandle> {
    if !with_table(|table| table.has_program_slot()) {
        return None;
    }

    let pid = NEXT_PID.fetch_add(1, Ordering::AcqRel);
    let task_id = sched::register_program_task(pid, parent_task_id, path)?;
    let Some(address_space_id) =
        mm::allocate_process_address_space(pid, 1, path, loader, entry_name, image)
    else {
        sched::complete_kernel_task(task_id, KernelTaskState::Failed);
        return None;
    };
    let mut record = ProcessRecord {
        pid,
        parent_pid,
        task_id,
        address_space_id,
        image_generation: 1,
        state: ProcessState::Ready,
        block_reason: BlockReason::None,
        program_path: path,
        loader,
        entry_name,
        artifact_body_format,
        artifact_body_inner_format,
        exit_code: 0,
        argc,
        argv: [[0u8; MAX_PROCESS_ARG_BYTES]; MAX_PROCESS_ARGS],
        argv_len: [0; MAX_PROCESS_ARGS],
        argv_truncated: [false; MAX_PROCESS_ARGS],
        envc: 0,
        env_name: [[0u8; MAX_PROCESS_ENV_NAME_BYTES]; MAX_PROCESS_ENVS],
        env_name_len: [0; MAX_PROCESS_ENVS],
        env_value: [[0u8; MAX_PROCESS_ENV_VALUE_BYTES]; MAX_PROCESS_ENVS],
        env_value_len: [0; MAX_PROCESS_ENVS],
        env_truncated: [false; MAX_PROCESS_ENVS],
    };
    write_args(&mut record);
    if !with_table(|table| table.write_program(record)) {
        sched::complete_kernel_task(task_id, KernelTaskState::Failed);
        mm::retire_process_address_space(pid, address_space_id);
        return None;
    }
    Some(ProcessHandle {
        pid,
        task_id,
        program_path: path,
        loader,
        entry_name,
    })
}

/// Marks a process and its main task as running.
#[must_use]
pub fn run_process(pid: usize) -> Option<ProcessHandle> {
    let record = with_table(|table| table.find_process(pid))?;
    if !process_state_can_run(record.state) {
        return None;
    }
    let _ = sched::run_kernel_task(record.task_id)?;
    with_table(|table| table.update_state(pid, ProcessState::Running, 0, BlockReason::None))
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

/// Marks a process and its main task as a resident service hold.
#[must_use]
pub fn block_service_process(pid: usize) -> Option<ProcessHandle> {
    block_process_with_reason(pid, BlockReason::Service)
}

/// Marks a process and its main task as waiting for pipe input.
#[must_use]
pub fn block_pipe_read_process(pid: usize) -> Option<ProcessHandle> {
    block_process_with_reason(pid, BlockReason::PipeRead)
}

/// Marks a process and its main task as waiting for pipe output capacity.
#[must_use]
pub fn block_pipe_write_process(pid: usize) -> Option<ProcessHandle> {
    block_process_with_reason(pid, BlockReason::PipeWrite)
}

/// Marks a process and its main task as waiting for child completion.
#[must_use]
pub fn block_wait_child_process(pid: usize) -> Option<ProcessHandle> {
    block_process_with_reason(pid, BlockReason::WaitChild)
}

/// Marks a process as waiting for future linked-user resume after raw replay.
#[must_use]
pub fn block_user_resume_process(pid: usize) -> Option<ProcessHandle> {
    block_process_with_reason(pid, BlockReason::UserResume)
}

fn block_process_with_reason(pid: usize, reason: BlockReason) -> Option<ProcessHandle> {
    let record = with_table(|table| table.find_process(pid))?;
    if !process_state_can_block(record.state) {
        return None;
    }
    let _ = sched::block_kernel_task(record.task_id, reason)?;
    with_table(|table| table.update_state(pid, ProcessState::Blocked, 0, reason))
}

/// Blocks a process until the scheduler reaches `wake_tick`.
#[must_use]
pub fn sleep_process_until(pid: usize, wake_tick: usize) -> Option<ProcessHandle> {
    let record = with_table(|table| table.find_process(pid))?;
    if !process_state_can_block(record.state) {
        return None;
    }
    let _ = sched::sleep_kernel_task_until(record.task_id, wake_tick)?;
    with_table(|table| table.update_state(pid, ProcessState::Blocked, 0, BlockReason::Sleep))
}

/// Wakes sleeping processes whose scheduler deadline has arrived.
#[must_use]
pub fn wake_sleepers_due(current_tick: usize, out: &mut [ProcessHandle]) -> usize {
    let count = with_table(|table| table.wake_sleepers_due(current_tick, out));
    let mut written = 0usize;
    let mut index = 0usize;
    while index < count {
        let candidate = out[index];
        if sched::wake_kernel_task(candidate.task_id).is_some() {
            if let Some(handle) = with_table(|table| {
                table.update_state(candidate.pid, ProcessState::Ready, 0, BlockReason::None)
            }) {
                out[written] = handle;
                written += 1;
            }
        }
        index += 1;
    }
    let accepted = written;
    while written < count {
        out[written] = EMPTY_PROCESS_HANDLE;
        written += 1;
    }
    accepted
}

/// Blocks `parent_pid` waiting for `child_pid`.
#[must_use]
pub fn begin_wait(parent_pid: usize, child_pid: usize) -> Option<WaitRecord> {
    let child = with_table(|table| table.find_process(child_pid))?;
    let record = WaitRecord {
        parent_pid,
        child_pid,
        child_state: child.state,
        exit_code: child.exit_code,
        completed: false,
    };
    if !with_table(|table| table.has_wait_slot(record)) {
        return None;
    }
    let _ = block_process_with_reason(parent_pid, BlockReason::WaitChild)?;
    if !with_table(|table| table.write_wait(record)) {
        let _ = wake_process(parent_pid);
        return None;
    }
    Some(record)
}

/// Completes a parent/child wait and resumes the parent.
#[must_use]
pub fn finish_wait(parent_pid: usize, child_pid: usize) -> Option<WaitRecord> {
    let child = with_table(|table| table.find_process(child_pid))?;
    let wait = with_table(|table| table.complete_wait(parent_pid, child))?;
    sched::complete_kernel_task(child.task_id, KernelTaskState::Reaped);
    let _ = wake_process(parent_pid);
    let _ = run_process(parent_pid);
    Some(wait)
}

/// Completes a retained wait for `child_pid` and wakes the parent without
/// running it.
///
/// Retained syscall continuations use this path: child completion makes the
/// parent scheduler-ready, and rootd later dispatches the parent to replay the
/// saved raw frame.
#[must_use]
pub fn complete_wait_for_child(
    child_pid: usize,
) -> Option<(ProcessHandle, ProcessHandle, WaitRecord)> {
    let child = with_table(|table| table.find_process(child_pid))?;
    let (parent, wait) = with_table(|table| table.complete_wait_for_child(child))?;
    sched::complete_kernel_task(child.task_id, KernelTaskState::Reaped);
    let _ = wake_process(parent.pid);
    Some((process_handle(parent), process_handle(child), wait))
}

/// Cancels an incomplete parent/child wait and resumes the parent.
#[must_use]
pub fn cancel_wait(parent_pid: usize, child_pid: usize) -> Option<WaitRecord> {
    let wait = with_table(|table| table.cancel_wait(parent_pid, child_pid))?;
    let _ = wake_process(parent_pid);
    let _ = run_process(parent_pid);
    Some(wait)
}

/// Returns a retained wait record for a parent/child pair.
#[must_use]
pub fn wait_record(parent_pid: usize, child_pid: usize) -> Option<WaitRecord> {
    with_table(|table| table.find_wait(parent_pid, child_pid))
}

/// Wakes an event-owned blocked process and marks its main task ready.
///
/// This generic helper is for kernel events that already own their wake
/// channel, such as pipe reads, child waits, and sleep deadlines. A
/// `UserResume` block is deliberately excluded until a trap-backed user-frame
/// resume primitive exists.
#[must_use]
pub fn wake_process(pid: usize) -> Option<ProcessHandle> {
    wake_blocked_process(pid, None)
}

/// Resumes a process whose raw syscall replay reached an explicit retained
/// user-frame continuation.
#[must_use]
pub fn run_user_resume_process(pid: usize) -> Option<ProcessHandle> {
    let record = with_table(|table| table.find_process(pid))?;
    if record.state != ProcessState::Blocked || record.block_reason != BlockReason::UserResume {
        return None;
    }
    let _ = sched::wake_kernel_task(record.task_id)?;
    let _ = sched::run_kernel_task(record.task_id)?;
    with_table(|table| table.update_state(pid, ProcessState::Running, 0, BlockReason::None))
}

/// Wakes an operator-blocked process and marks its main task ready.
#[must_use]
pub fn wake_operator_blocked_process(pid: usize) -> Option<ProcessHandle> {
    wake_blocked_process(pid, Some(BlockReason::Operator))
}

fn wake_blocked_process(pid: usize, expected_reason: Option<BlockReason>) -> Option<ProcessHandle> {
    let record = with_table(|table| table.find_process(pid))?;
    if record.state != ProcessState::Blocked {
        return None;
    }
    if expected_reason.is_none() && record.block_reason == BlockReason::UserResume {
        return None;
    }
    if let Some(reason) = expected_reason {
        if record.block_reason != reason {
            return None;
        }
    }
    let _ = sched::wake_kernel_task(record.task_id)?;
    with_table(|table| table.update_state(pid, ProcessState::Ready, 0, BlockReason::None))
}

/// Marks a running process and task ready without dispatching it immediately.
///
/// This is used by direct-backend `execve` admission: after same-PID image
/// replacement retains a pending program body, rootd must be able to dispatch
/// that replacement through the normal ready-work loop.
#[must_use]
pub fn ready_process(pid: usize) -> Option<ProcessHandle> {
    let record = with_table(|table| table.find_process(pid))?;
    if record.state != ProcessState::Running {
        return None;
    }
    let _ = sched::ready_kernel_task(record.task_id)?;
    with_table(|table| table.update_state(pid, ProcessState::Ready, 0, BlockReason::None))
}

/// Cooperatively yields a process to the scheduler ready queue.
#[must_use]
pub fn yield_process(pid: usize) -> Option<ProcessHandle> {
    let record = with_table(|table| table.find_process(pid))?;
    if record.state != ProcessState::Running {
        return None;
    }
    let scheduled_task_id = sched::yield_kernel_task(record.task_id)?;
    if scheduled_task_id != record.task_id {
        let _ =
            with_table(|table| table.update_state(pid, ProcessState::Ready, 0, BlockReason::None))?;
    }
    with_table(|table| {
        table.update_task_state(scheduled_task_id, ProcessState::Running, 0, BlockReason::None)
    })
}

/// Marks a process as exited.
pub fn exit_process(pid: usize, state: ProcessState, exit_code: i32) -> ProcessAdoptionSnapshot {
    let _ = with_table(|table| table.cancel_incomplete_waits_for_parent(pid));
    let mut adoption = ProcessAdoptionSnapshot::empty();
    adoption.count = if pid == ROOTD_PID {
        0usize
    } else {
        with_table(|table| table.reparent_live_children(pid, ROOTD_PID, &mut adoption.children))
    };
    let mut reparent_index = 0usize;
    while reparent_index < adoption.count {
        sched::reparent_kernel_task(adoption.children[reparent_index].task_id, ROOTD_PID);
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
        ProcessState::Reaped => KernelTaskState::Reaped,
    };
    let (task_id, address_space_id) = with_table(|table| {
        let mut index = 0usize;
        while index < table.records.len() {
            if table.records[index].pid == pid {
                return (table.records[index].task_id, table.records[index].address_space_id);
            }
            index += 1;
        }
        (0, 0)
    });
    let _ = with_table(|table| table.update_state(pid, state, exit_code, BlockReason::None));
    if address_space_id != 0 && process_state_retires_address_space(state) {
        mm::retire_process_address_space(pid, address_space_id);
    }
    if task_id != 0 {
        sched::complete_kernel_task(task_id, task_state);
    }
    adoption
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
