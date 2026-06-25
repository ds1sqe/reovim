//! Scheduler, clock, and thread-identity bridge.
//!
//! Upper layers receive `uapi/sched` function tables from composition roots.
//! This bridge owns the current handle-backed scheduler, clock, thread-id, and
//! sync implementations so pure data-structure code does not expose World
//! service helpers upward.

use {
    core::{
        alloc::Layout,
        cell::UnsafeCell,
        sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
    },
    reovim_kabi_platform::handle,
    reovim_uapi_sched::{
        ClockControl, DetachedThreadSpawner, SpawnError, SyncControl, ThreadControl,
    },
};

/// Maximum retained kernel task records.
pub const MAX_KERNEL_TASKS: usize = 32;

/// Kernel scheduler-visible task lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KernelTaskState {
    /// Slot is unused.
    Empty,
    /// Task can run.
    Ready,
    /// Task is currently running.
    Running,
    /// Task is waiting for a kernel condition.
    Blocked,
    /// Task exited successfully.
    Exited,
    /// Task failed.
    Failed,
    /// Task requested halt.
    Halted,
}

impl KernelTaskState {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Blocked => "blocked",
            Self::Exited => "exited",
            Self::Failed => "failed",
            Self::Halted => "halted",
        }
    }
}

/// Scheduler-visible reason for a blocked task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockReason {
    /// Task is not blocked.
    None,
    /// Task is explicitly blocked by an operator/process-control request.
    Operator,
    /// Task is waiting for a child process to complete.
    WaitChild,
}

impl BlockReason {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Operator => "operator",
            Self::WaitChild => "wait-child",
        }
    }
}

/// Retained scheduler task record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KernelTaskRecord {
    /// Task identifier.
    pub task_id: usize,
    /// Owning process identifier.
    pub process_id: usize,
    /// Parent task identifier.
    pub parent_task_id: usize,
    /// Task state.
    pub state: KernelTaskState,
    /// Reason when the task is blocked.
    pub block_reason: BlockReason,
    /// Number of times the scheduler dispatched this task.
    pub run_count: usize,
    /// Explicit scheduler ticks charged to this task while running.
    pub runtime_ticks: usize,
    /// Scheduler-visible entry name.
    pub entry: &'static str,
}

impl KernelTaskRecord {
    /// Empty task-table slot.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            task_id: 0,
            process_id: 0,
            parent_task_id: 0,
            state: KernelTaskState::Empty,
            block_reason: BlockReason::None,
            run_count: 0,
            runtime_ticks: 0,
            entry: "",
        }
    }

    const fn kernel(
        task_id: usize,
        process_id: usize,
        parent_task_id: usize,
        entry: &'static str,
    ) -> Self {
        Self {
            task_id,
            process_id,
            parent_task_id,
            state: KernelTaskState::Running,
            block_reason: BlockReason::None,
            run_count: 1,
            runtime_ticks: 0,
            entry,
        }
    }
}

/// Empty task record used for bounded snapshots.
pub const EMPTY_KERNEL_TASK_RECORD: KernelTaskRecord = KernelTaskRecord::empty();

/// Snapshot of the bounded scheduler run queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchedulerSnapshot {
    /// Scheduler-selected running task.
    pub current_task_id: usize,
    /// Number of retained ready-queue entries.
    pub ready_len: usize,
    /// Ready queue task IDs in FIFO order.
    pub ready_queue: [usize; MAX_KERNEL_TASKS],
    /// Number of explicit task dispatches.
    pub dispatch_count: usize,
    /// Number of cooperative yields.
    pub yield_count: usize,
    /// Number of explicit scheduler ticks.
    pub tick_count: usize,
}

impl SchedulerSnapshot {
    const fn empty() -> Self {
        Self {
            current_task_id: 0,
            ready_len: 0,
            ready_queue: [0; MAX_KERNEL_TASKS],
            dispatch_count: 0,
            yield_count: 0,
            tick_count: 0,
        }
    }

    /// Returns the next ready task, if any.
    #[must_use]
    pub const fn next_ready_task_id(self) -> usize {
        if self.ready_len == 0 {
            0
        } else {
            self.ready_queue[0]
        }
    }
}

struct KernelTaskTable {
    records: [KernelTaskRecord; MAX_KERNEL_TASKS],
    ready_queue: [usize; MAX_KERNEL_TASKS],
    ready_len: usize,
    current_task_id: usize,
    dispatch_count: usize,
    yield_count: usize,
    tick_count: usize,
}

impl KernelTaskTable {
    const fn new() -> Self {
        let mut records = [KernelTaskRecord::empty(); MAX_KERNEL_TASKS];
        records[0] = KernelTaskRecord::kernel(1, 1, 0, "rootd");
        records[1] = KernelTaskRecord::kernel(2, 2, 1, "root-shell");
        Self {
            records,
            ready_queue: [0; MAX_KERNEL_TASKS],
            ready_len: 0,
            current_task_id: 2,
            dispatch_count: 0,
            yield_count: 0,
            tick_count: 0,
        }
    }

    fn reset(&mut self) {
        self.records = [KernelTaskRecord::empty(); MAX_KERNEL_TASKS];
        self.records[0] = KernelTaskRecord::kernel(1, 1, 0, "rootd");
        self.records[1] = KernelTaskRecord::kernel(2, 2, 1, "root-shell");
        self.ready_queue = [0; MAX_KERNEL_TASKS];
        self.ready_len = 0;
        self.current_task_id = 2;
        self.dispatch_count = 0;
        self.yield_count = 0;
        self.tick_count = 0;
    }

    fn write_program(&mut self, record: KernelTaskRecord) {
        let slot = 2 + ((record.task_id.saturating_sub(3)) % (MAX_KERNEL_TASKS - 2));
        if self.records[slot].state == KernelTaskState::Ready {
            self.remove_ready(self.records[slot].task_id);
        }
        self.records[slot] = record;
    }

    fn update(
        &mut self,
        task_id: usize,
        state: KernelTaskState,
        block_reason: BlockReason,
    ) -> Option<KernelTaskRecord> {
        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].task_id == task_id {
                self.records[index].state = state;
                self.records[index].block_reason = block_reason;
                return Some(self.records[index]);
            }
            index += 1;
        }
        None
    }

    fn reparent(&mut self, task_id: usize, parent_task_id: usize) -> Option<KernelTaskRecord> {
        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].task_id == task_id {
                self.records[index].parent_task_id = parent_task_id;
                return Some(self.records[index]);
            }
            index += 1;
        }
        None
    }

    fn find(&self, task_id: usize) -> Option<KernelTaskRecord> {
        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].task_id == task_id {
                return Some(self.records[index]);
            }
            index += 1;
        }
        None
    }

    fn ready_contains(&self, task_id: usize) -> bool {
        let mut index = 0usize;
        while index < self.ready_len {
            if self.ready_queue[index] == task_id {
                return true;
            }
            index += 1;
        }
        false
    }

    fn enqueue_ready(&mut self, task_id: usize) {
        if task_id == 0 || self.ready_len >= self.ready_queue.len() || self.ready_contains(task_id)
        {
            return;
        }
        self.ready_queue[self.ready_len] = task_id;
        self.ready_len += 1;
    }

    fn remove_ready(&mut self, task_id: usize) {
        let mut index = 0usize;
        while index < self.ready_len {
            if self.ready_queue[index] == task_id {
                let mut move_index = index;
                while move_index + 1 < self.ready_len {
                    self.ready_queue[move_index] = self.ready_queue[move_index + 1];
                    move_index += 1;
                }
                self.ready_len -= 1;
                self.ready_queue[self.ready_len] = 0;
                return;
            }
            index += 1;
        }
    }

    fn pop_ready(&mut self) -> Option<usize> {
        if self.ready_len == 0 {
            return None;
        }
        let task_id = self.ready_queue[0];
        self.remove_ready(task_id);
        Some(task_id)
    }

    fn dispatch(&mut self, task_id: usize) -> Option<KernelTaskRecord> {
        self.remove_ready(task_id);
        let record = self.update(task_id, KernelTaskState::Running, BlockReason::None)?;
        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].task_id == task_id {
                self.records[index].run_count = self.records[index].run_count.saturating_add(1);
                break;
            }
            index += 1;
        }
        let record = self.find(task_id).unwrap_or(record);
        if self.current_task_id != task_id {
            self.dispatch_count = self.dispatch_count.saturating_add(1);
        }
        self.current_task_id = task_id;
        Some(record)
    }

    fn dispatch_next_ready(&mut self) -> Option<KernelTaskRecord> {
        let task_id = self.pop_ready()?;
        self.dispatch(task_id)
    }

    fn complete(&mut self, task_id: usize, state: KernelTaskState) {
        self.remove_ready(task_id);
        let Some(record) = self.update(task_id, state, BlockReason::None) else {
            return;
        };
        if self.current_task_id == task_id {
            self.current_task_id = record.parent_task_id;
        }
    }

    fn block(&mut self, task_id: usize, reason: BlockReason) {
        self.remove_ready(task_id);
        let Some(record) = self.update(task_id, KernelTaskState::Blocked, reason) else {
            return;
        };
        if self.current_task_id == task_id {
            self.current_task_id = record.parent_task_id;
        }
    }

    fn wake(&mut self, task_id: usize) -> Option<KernelTaskRecord> {
        let record = self.update(task_id, KernelTaskState::Ready, BlockReason::None)?;
        self.enqueue_ready(task_id);
        Some(record)
    }

    fn yield_task(&mut self, task_id: usize) -> Option<usize> {
        let record = self.find(task_id)?;
        if record.state != KernelTaskState::Running {
            return Some(self.current_task_id);
        }

        self.yield_count = self.yield_count.saturating_add(1);
        self.update(task_id, KernelTaskState::Ready, BlockReason::None);
        self.enqueue_ready(task_id);

        if let Some(next_task_id) = self.pop_ready() {
            let _ = self.dispatch(next_task_id);
            return Some(next_task_id);
        }

        let _ = self.dispatch(task_id);
        Some(task_id)
    }

    fn tick(&mut self, task_id: usize) -> Option<KernelTaskRecord> {
        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].task_id == task_id {
                if self.records[index].state != KernelTaskState::Running {
                    return None;
                }
                self.records[index].runtime_ticks =
                    self.records[index].runtime_ticks.saturating_add(1);
                self.tick_count = self.tick_count.saturating_add(1);
                return Some(self.records[index]);
            }
            index += 1;
        }
        None
    }

    fn snapshot_scheduler(&self) -> SchedulerSnapshot {
        let mut snapshot = SchedulerSnapshot::empty();
        snapshot.current_task_id = self.current_task_id;
        snapshot.ready_len = self.ready_len;
        snapshot.dispatch_count = self.dispatch_count;
        snapshot.yield_count = self.yield_count;
        snapshot.tick_count = self.tick_count;
        let mut index = 0usize;
        while index < self.ready_len {
            snapshot.ready_queue[index] = self.ready_queue[index];
            index += 1;
        }
        snapshot
    }
}

struct KernelTaskCell(UnsafeCell<KernelTaskTable>);

// SAFETY: access is serialized by `TASK_LOCK`.
unsafe impl Sync for KernelTaskCell {}

static TASKS: KernelTaskCell = KernelTaskCell(UnsafeCell::new(KernelTaskTable::new()));
static TASK_LOCK: AtomicBool = AtomicBool::new(false);
static NEXT_TASK_ID: AtomicUsize = AtomicUsize::new(3);

struct TaskGuard;

impl TaskGuard {
    fn acquire() -> Self {
        while TASK_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        TASK_LOCK.store(false, Ordering::Release);
    }
}

fn with_kernel_tasks<R>(f: impl FnOnce(&mut KernelTaskTable) -> R) -> R {
    let _guard = TaskGuard::acquire();
    // SAFETY: `TASK_LOCK` serializes access to the task table.
    let tasks = unsafe { &mut *TASKS.0.get() };
    f(tasks)
}

/// Resets scheduler bookkeeping to rootd plus root shell.
pub fn reset_kernel_scheduler() {
    NEXT_TASK_ID.store(3, Ordering::Release);
    with_kernel_tasks(KernelTaskTable::reset);
}

/// Rebinds the reserved shell-session task to the active shell image.
pub fn install_shell_session_task(entry: &'static str) {
    with_kernel_tasks(|tasks| {
        tasks.records[1] = KernelTaskRecord::kernel(2, 2, 1, entry);
        tasks.current_task_id = 2;
        tasks.remove_ready(2);
    });
}

/// Registers a ready task for an image-packaged program.
#[must_use]
pub fn register_program_task(
    process_id: usize,
    parent_task_id: usize,
    entry: &'static str,
) -> usize {
    let task_id = NEXT_TASK_ID.fetch_add(1, Ordering::AcqRel);
    with_kernel_tasks(|tasks| {
        tasks.write_program(KernelTaskRecord {
            task_id,
            process_id,
            parent_task_id,
            state: KernelTaskState::Ready,
            block_reason: BlockReason::None,
            run_count: 0,
            runtime_ticks: 0,
            entry,
        });
        tasks.enqueue_ready(task_id);
    });
    task_id
}

/// Marks a task as running.
pub fn run_kernel_task(task_id: usize) {
    with_kernel_tasks(|tasks| {
        let _ = tasks.dispatch(task_id);
    });
}

/// Dispatches the next ready task in FIFO scheduler order.
#[must_use]
pub fn dispatch_next_ready_task() -> Option<KernelTaskRecord> {
    with_kernel_tasks(KernelTaskTable::dispatch_next_ready)
}

/// Marks a task as blocked.
pub fn block_kernel_task(task_id: usize, reason: BlockReason) {
    with_kernel_tasks(|tasks| tasks.block(task_id, reason));
}

/// Wakes a blocked task and makes it ready.
pub fn wake_kernel_task(task_id: usize) {
    with_kernel_tasks(|tasks| {
        let _ = tasks.wake(task_id);
    });
}

/// Cooperatively yields from the current task to the next ready task.
#[must_use]
pub fn yield_kernel_task(task_id: usize) -> Option<usize> {
    with_kernel_tasks(|tasks| tasks.yield_task(task_id))
}

/// Records one scheduler tick against a running task.
#[must_use]
pub fn tick_kernel_task(task_id: usize) -> Option<KernelTaskRecord> {
    with_kernel_tasks(|tasks| tasks.tick(task_id))
}

/// Completes a task record.
pub fn complete_kernel_task(task_id: usize, state: KernelTaskState) {
    with_kernel_tasks(|tasks| tasks.complete(task_id, state));
}

/// Reparents a task to a new supervisor task.
pub fn reparent_kernel_task(task_id: usize, parent_task_id: usize) {
    with_kernel_tasks(|tasks| {
        let _ = tasks.reparent(task_id, parent_task_id);
    });
}

/// Copies retained scheduler task records into `out`, returning the count.
pub fn snapshot_kernel_tasks(out: &mut [KernelTaskRecord]) -> usize {
    with_kernel_tasks(|tasks| {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < tasks.records.len() && written < out.len() {
            if tasks.records[index].state != KernelTaskState::Empty {
                out[written] = tasks.records[index];
                written += 1;
            }
            index += 1;
        }
        written
    })
}

/// Returns scheduler run-queue metadata.
#[must_use]
pub fn snapshot_scheduler() -> SchedulerSnapshot {
    with_kernel_tasks(|tasks| tasks.snapshot_scheduler())
}

/// Returns the up-face clock control table backed by this bridge.
///
/// ```rust,no_run
/// use reovim_system_kernel::sched::clock_control;
///
/// let clock = clock_control();
/// let _ = clock.monotonic();
/// ```
#[must_use]
pub const fn clock_control() -> ClockControl {
    ClockControl::new(monotonic, realtime)
}

/// Returns the up-face thread identity control table backed by this bridge.
///
/// ```rust,no_run
/// use reovim_system_kernel::sched::thread_control;
///
/// let thread = thread_control();
/// let _ = thread.current_id();
/// ```
#[must_use]
pub const fn thread_control() -> ThreadControl {
    ThreadControl::new(current_id)
}

/// Returns the up-face sync park/unpark control table backed by this bridge.
///
/// ```rust,no_run
/// use core::sync::atomic::AtomicU32;
/// use reovim_system_kernel::sched::sync_control;
///
/// let word = AtomicU32::new(0);
/// sync_control().unpark(&word);
/// ```
#[must_use]
pub const fn sync_control() -> SyncControl {
    SyncControl::new(park, unpark, unpark_all)
}

/// Installs this bridge's sync control table into `lib/ds`.
///
/// # Errors
///
/// Returns [`reovim_lib_ds::sync_backend::SyncBackendInstallError`] when the
/// process already installed a sync backend.
pub fn install_lib_ds_sync_backend()
-> Result<(), reovim_lib_ds::sync_backend::SyncBackendInstallError> {
    reovim_lib_ds::sync_backend::install(sync_control())
}

/// System-kernel implementation of the product-facing detached thread spawner.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemThreadSpawner;

impl DetachedThreadSpawner for SystemThreadSpawner {
    fn spawn_detached<F>(self, f: F) -> Result<(), SpawnError>
    where
        F: FnOnce() + Send + 'static,
    {
        spawn_detached(f)
    }
}

/// Returns the up-face detached thread spawner backed by this bridge.
///
/// ```rust,no_run
/// use reovim_system_kernel::sched::thread_spawner;
/// use reovim_uapi_sched::DetachedThreadSpawner;
///
/// thread_spawner().spawn_detached(|| {}).expect("spawn succeeds");
/// ```
#[must_use]
pub const fn thread_spawner() -> SystemThreadSpawner {
    SystemThreadSpawner
}

fn monotonic() -> i64 {
    handle().clock()
}

fn realtime() -> i64 {
    handle().realtime()
}

fn current_id() -> i64 {
    i64::from(handle().thread_id())
}

fn park(word: &AtomicU32, expected: u32) {
    reovim_kabi_platform::handle().park(word, expected);
}

fn unpark(word: &AtomicU32) {
    reovim_kabi_platform::handle().unpark(word);
}

fn unpark_all(word: &AtomicU32) {
    handle().unpark_all(word);
}

unsafe extern "C" fn trampoline<F: FnOnce()>(arg: *mut u8) {
    let boxed = arg.cast::<F>();
    // SAFETY: `arg` is the closure handle `spawn_detached` produced: either a
    // heap box for a non-ZST `F`, or a dangling-aligned pointer for a ZST. The
    // spawning thread never touches it after a successful spawn.
    let f = unsafe { core::ptr::read(boxed) };
    if core::mem::size_of::<F>() != 0 {
        // SAFETY: for a non-ZST `F`, `boxed` is the unique live allocation that
        // held the closure. The value was moved out above, so only the backing
        // storage remains to free.
        unsafe {
            handle()
                .dealloc(core::ptr::NonNull::new_unchecked(boxed.cast::<u8>()), Layout::new::<F>());
        }
    }
    f();
}

fn spawn_detached<F>(f: F) -> Result<(), SpawnError>
where
    F: FnOnce() + Send + 'static,
{
    if core::mem::size_of::<F>() == 0 {
        let arg = core::ptr::NonNull::<F>::dangling().as_ptr().cast::<u8>();
        let _ = f;
        // SAFETY: `trampoline::<F>` reconstructs a zero-sized `F` from the
        // aligned dangling pointer and frees no storage.
        let result = unsafe { handle().thread_spawn(trampoline::<F>, arg) };
        return result.map(|_| ()).map_err(map_spawn_errno);
    }

    let layout = Layout::new::<F>();
    let boxed = handle()
        .alloc(layout)
        .map_err(|_| SpawnError::OutOfMemory)?
        .cast::<F>();
    // SAFETY: `boxed` is fresh storage of the correct size and alignment for
    // `F`; ownership transfers to the child thread after a successful spawn.
    unsafe {
        core::ptr::write(boxed.as_ptr(), f);
    }

    // SAFETY: `trampoline::<F>` consumes exactly the closure box passed as
    // `arg`; the child thread owns that box on success.
    let result = unsafe { handle().thread_spawn(trampoline::<F>, boxed.as_ptr().cast::<u8>()) };
    match result {
        Ok(_tid) => Ok(()),
        Err(errno) => {
            // SAFETY: the thread did not start, so this thread still uniquely
            // owns the initialized closure box and must reclaim it.
            unsafe {
                core::ptr::drop_in_place(boxed.as_ptr());
                handle().dealloc(boxed.cast::<u8>(), layout);
            }
            Err(map_spawn_errno(errno))
        }
    }
}

const fn map_spawn_errno(errno: reovim_kabi_platform::Errno) -> SpawnError {
    SpawnError::Refused(errno.code())
}
