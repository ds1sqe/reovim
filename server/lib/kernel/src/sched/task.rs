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

// LLVM coverage artifact: closing brace of Display impl marked DA:0
// despite being exercised by test_task_id_display.
#[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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

