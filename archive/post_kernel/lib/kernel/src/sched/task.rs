//! Task abstraction for deferred work units.
//!
//! Linux equivalent: `kernel/sched/core.c` `task_struct`
//!
//! This module provides the [`Task`] type for encapsulating units of work
//! to be executed by the scheduler. Tasks are the fundamental unit of
//! deferred execution in the kernel.
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//!
//! // Create a task with default (normal) priority
//! let task = Task::new(|| {
//!     println!("Task executed!");
//! });
//!
//! // Create a high-priority task
//! let urgent = Task::with_priority(Priority::HIGH, || {
//!     println!("Urgent work!");
//! });
//! ```

use std::{
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};

/// Unique task identifier.
///
/// Task IDs are monotonically increasing and never reused within a session.
/// This follows the same pattern as [`BufferId`](crate::mm::BufferId) and
/// [`ScopeId`](crate::ipc::ScopeId).
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let id1 = TaskId::new();
/// let id2 = TaskId::new();
/// assert_ne!(id1, id2);
/// assert!(id2.as_u64() > id1.as_u64());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(u64);

impl TaskId {
    /// Create a new unique task ID.
    ///
    /// IDs start at 1 and increment monotonically.
    #[must_use]
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw numeric value.
    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Create from raw value.
    ///
    /// Primarily for testing and deserialization.
    #[inline]
    #[must_use]
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Task({})", self.0)
    }
}

/// Task execution priority.
///
/// Lower values indicate higher priority (processed sooner).
/// This follows the convention where 0 is the highest priority.
///
/// # Priority Levels
///
/// | Level | Value | Use Case |
/// |-------|-------|----------|
/// | `CRITICAL` | 0 | Kernel-internal, mode changes |
/// | `HIGH` | 50 | User input, commands |
/// | `NORMAL` | 100 | Default for most tasks |
/// | `LOW` | 200 | Background work |
/// | `IDLE` | 1000 | Cleanup, logging |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Priority(pub u32);

impl Priority {
    /// Critical priority - kernel-internal operations, mode changes.
    pub const CRITICAL: Self = Self(0);

    /// High priority - user input, commands.
    pub const HIGH: Self = Self(50);

    /// Normal priority - default for most tasks.
    pub const NORMAL: Self = Self(100);

    /// Low priority - background work, render signals.
    pub const LOW: Self = Self(200);

    /// Idle priority - cleanup, logging.
    pub const IDLE: Self = Self(1000);

    /// Create a priority with a custom value.
    #[inline]
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Get the raw priority value.
    #[inline]
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl Default for Priority {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::CRITICAL => write!(f, "Critical(0)"),
            Self::HIGH => write!(f, "High(50)"),
            Self::NORMAL => write!(f, "Normal(100)"),
            Self::LOW => write!(f, "Low(200)"),
            Self::IDLE => write!(f, "Idle(1000)"),
            Self(v) => write!(f, "Priority({v})"),
        }
    }
}

/// Task execution state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TaskState {
    /// Task is waiting to be executed.
    #[default]
    Pending,

    /// Task is currently executing.
    Running,

    /// Task completed successfully.
    Completed,

    /// Task failed (error or panic).
    Failed,
}

impl TaskState {
    /// Check if the task is still pending.
    #[inline]
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Check if the task has finished (completed or failed).
    #[inline]
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Running => write!(f, "Running"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

/// Type-erased task function.
///
/// A boxed closure that can be executed once. Tasks must be `Send` to allow
/// scheduling from any thread.
pub type BoxedTask = Box<dyn FnOnce() + Send + 'static>;

/// A unit of deferred work.
///
/// Tasks encapsulate a closure to be executed later by the scheduler.
/// Each task has a unique ID, priority, and execution state.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// // Create a high-priority task with a name
/// let mut task = Task::with_priority(Priority::HIGH, || println!("Hello!"))
///     .with_name("greeting");
///
/// assert_eq!(task.state(), TaskState::Pending);
/// assert_eq!(task.priority(), Priority::HIGH);
///
/// // Execute the task
/// let result = task.execute();
/// assert!(result.is_ok());
/// assert_eq!(task.state(), TaskState::Completed);
/// ```
pub struct Task {
    /// Unique identifier.
    id: TaskId,

    /// Task priority.
    priority: Priority,

    /// Current state.
    state: TaskState,

    /// The work to execute (None after execution).
    work: Option<BoxedTask>,

    /// Optional name for debugging.
    name: Option<&'static str>,
}

impl Task {
    /// Create a new task with normal priority.
    #[must_use]
    pub fn new<F>(work: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        Self::with_priority(Priority::NORMAL, work)
    }

    /// Create a task with specific priority.
    #[must_use]
    pub fn with_priority<F>(priority: Priority, work: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        Self {
            id: TaskId::new(),
            priority,
            state: TaskState::Pending,
            work: Some(Box::new(work)),
            name: None,
        }
    }

    /// Set the task name for debugging.
    #[must_use]
    pub const fn with_name(mut self, name: &'static str) -> Self {
        self.name = Some(name);
        self
    }

    /// Get the task ID.
    #[inline]
    #[must_use]
    pub const fn id(&self) -> TaskId {
        self.id
    }

    /// Get the task priority.
    #[inline]
    #[must_use]
    pub const fn priority(&self) -> Priority {
        self.priority
    }

    /// Get the current state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> TaskState {
        self.state
    }

    /// Get the task name (if set).
    #[inline]
    #[must_use]
    pub const fn name(&self) -> Option<&'static str> {
        self.name
    }

    /// Check if the task can still be executed.
    #[inline]
    #[must_use]
    pub const fn is_executable(&self) -> bool {
        matches!(self.state, TaskState::Pending) && self.work.is_some()
    }

    /// Execute the task.
    ///
    /// Consumes the work closure and transitions state to `Completed`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Task has already been executed
    /// - Task was cancelled (work is None)
    pub fn execute(&mut self) -> Result<(), &'static str> {
        if self.state != TaskState::Pending {
            return Err("task already executed or cancelled");
        }

        let work = self.work.take().ok_or("work closure missing")?;

        self.state = TaskState::Running;
        work();
        self.state = TaskState::Completed;

        Ok(())
    }

    /// Mark the task as failed.
    ///
    /// Used by the executor when a panic is caught during execution.
    pub fn mark_failed(&mut self) {
        self.work = None;
        self.state = TaskState::Failed;
    }

    /// Cancel the task.
    ///
    /// Removes the work closure without executing it.
    pub fn cancel(&mut self) {
        self.work = None;
        self.state = TaskState::Failed;
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("id", &self.id)
            .field("priority", &self.priority)
            .field("state", &self.state)
            .field("name", &self.name)
            .field("has_work", &self.work.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::sync::{Arc, atomic::AtomicBool},
    };

    // === TaskId tests ===

    #[test]
    fn test_task_id_new() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1, id2);
        assert!(id2.as_u64() > id1.as_u64());
    }

    #[test]
    fn test_task_id_from_raw() {
        let id = TaskId::from_raw(42);
        assert_eq!(id.as_u64(), 42);
    }

    #[test]
    fn test_task_id_display() {
        let id = TaskId::from_raw(123);
        assert_eq!(format!("{id}"), "Task(123)");
    }

    #[test]
    fn test_task_id_ordering() {
        let id1 = TaskId::from_raw(1);
        let id2 = TaskId::from_raw(2);
        assert!(id1 < id2);
    }

    #[test]
    fn test_task_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(TaskId::from_raw(1));
        set.insert(TaskId::from_raw(2));
        assert!(set.contains(&TaskId::from_raw(1)));
        assert!(!set.contains(&TaskId::from_raw(3)));
    }

    // === Priority tests ===

    #[test]
    fn test_priority_constants() {
        assert_eq!(Priority::CRITICAL.as_u32(), 0);
        assert_eq!(Priority::HIGH.as_u32(), 50);
        assert_eq!(Priority::NORMAL.as_u32(), 100);
        assert_eq!(Priority::LOW.as_u32(), 200);
        assert_eq!(Priority::IDLE.as_u32(), 1000);
    }

    #[test]
    fn test_priority_default() {
        assert_eq!(Priority::default(), Priority::NORMAL);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::CRITICAL < Priority::HIGH);
        assert!(Priority::HIGH < Priority::NORMAL);
        assert!(Priority::NORMAL < Priority::LOW);
        assert!(Priority::LOW < Priority::IDLE);
    }

    #[test]
    fn test_priority_custom() {
        let custom = Priority::new(75);
        assert_eq!(custom.as_u32(), 75);
        assert!(Priority::HIGH < custom);
        assert!(custom < Priority::NORMAL);
    }

    #[test]
    fn test_priority_display() {
        assert_eq!(format!("{}", Priority::CRITICAL), "Critical(0)");
        assert_eq!(format!("{}", Priority::HIGH), "High(50)");
        assert_eq!(format!("{}", Priority::NORMAL), "Normal(100)");
        assert_eq!(format!("{}", Priority::LOW), "Low(200)");
        assert_eq!(format!("{}", Priority::IDLE), "Idle(1000)");
        assert_eq!(format!("{}", Priority::new(75)), "Priority(75)");
    }

    // === TaskState tests ===

    #[test]
    fn test_task_state_default() {
        assert_eq!(TaskState::default(), TaskState::Pending);
    }

    #[test]
    fn test_task_state_is_pending() {
        assert!(TaskState::Pending.is_pending());
        assert!(!TaskState::Running.is_pending());
        assert!(!TaskState::Completed.is_pending());
        assert!(!TaskState::Failed.is_pending());
    }

    #[test]
    fn test_task_state_is_finished() {
        assert!(!TaskState::Pending.is_finished());
        assert!(!TaskState::Running.is_finished());
        assert!(TaskState::Completed.is_finished());
        assert!(TaskState::Failed.is_finished());
    }

    #[test]
    fn test_task_state_display() {
        assert_eq!(format!("{}", TaskState::Pending), "Pending");
        assert_eq!(format!("{}", TaskState::Running), "Running");
        assert_eq!(format!("{}", TaskState::Completed), "Completed");
        assert_eq!(format!("{}", TaskState::Failed), "Failed");
    }

    // === Task tests ===

    #[test]
    fn test_task_new() {
        let task = Task::new(|| {});
        assert_eq!(task.state(), TaskState::Pending);
        assert_eq!(task.priority(), Priority::NORMAL);
        assert!(task.name().is_none());
        assert!(task.is_executable());
    }

    #[test]
    fn test_task_with_priority() {
        let task = Task::with_priority(Priority::HIGH, || {});
        assert_eq!(task.priority(), Priority::HIGH);
    }

    #[test]
    fn test_task_with_name() {
        let task = Task::new(|| {}).with_name("test_task");
        assert_eq!(task.name(), Some("test_task"));
    }

    #[test]
    fn test_task_execute() {
        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = Arc::clone(&executed);

        let mut task = Task::new(move || {
            executed_clone.store(true, Ordering::SeqCst);
        });

        assert!(task.execute().is_ok());
        assert!(executed.load(Ordering::SeqCst));
        assert_eq!(task.state(), TaskState::Completed);
        assert!(!task.is_executable());
    }

    #[test]
    fn test_task_execute_twice_fails() {
        let mut task = Task::new(|| {});
        assert!(task.execute().is_ok());
        assert!(task.execute().is_err());
    }

    #[test]
    fn test_task_cancel() {
        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = Arc::clone(&executed);

        let mut task = Task::new(move || {
            executed_clone.store(true, Ordering::SeqCst);
        });

        task.cancel();
        assert_eq!(task.state(), TaskState::Failed);
        assert!(!task.is_executable());
        assert!(!executed.load(Ordering::SeqCst));
    }

    #[test]
    fn test_task_mark_failed() {
        let mut task = Task::new(|| {});
        task.mark_failed();
        assert_eq!(task.state(), TaskState::Failed);
        assert!(!task.is_executable());
    }

    #[test]
    fn test_task_debug() {
        let task = Task::new(|| {}).with_name("debug_test");
        let debug_str = format!("{task:?}");
        assert!(debug_str.contains("Task"));
        assert!(debug_str.contains("debug_test"));
        assert!(debug_str.contains("Pending"));
    }

    #[test]
    fn test_task_unique_ids() {
        let task1 = Task::new(|| {});
        let task2 = Task::new(|| {});
        assert_ne!(task1.id(), task2.id());
    }

    // === Send + Sync tests ===

    #[test]
    fn test_task_id_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TaskId>();
    }

    #[test]
    fn test_priority_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Priority>();
    }

    #[test]
    fn test_task_state_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TaskState>();
    }

    #[test]
    fn test_task_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Task>();
    }
}
