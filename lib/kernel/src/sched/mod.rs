//! Scheduler subsystem.
//!
//! Linux equivalent: `kernel/sched/`
//!
//! This module provides the main runtime loop, task execution, and work queues.
//! It is the heart of the editor's event coordination, managing:
//!
//! - **Runtime**: Central event loop coordinator
//! - **Task**: Deferred work units with priorities
//! - **`WorkQueue`**: Bounded task queue with overflow detection
//! - **`PriorityQueue`**: Priority-ordered event queue
//! - **Executor**: Synchronous task executor with panic handling
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │                         Runtime                            │
//! │  ┌──────────────┐  ┌────────────┐  ┌────────────────────┐  │
//! │  │ PriorityQueue│  │  WorkQueue │  │     Executor       │  │
//! │  │   (events)   │  │   (tasks)  │  │ (panic handling)   │  │
//! │  └──────┬───────┘  └──────┬─────┘  └──────────┬─────────┘  │
//! │         │                 │                   │            │
//! │         v                 v                   v            │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │                    EventBus                         │   │
//! │  │              (type-erased dispatch)                 │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! └────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```
//! use reovim_kernel::sched::{Runtime, RuntimeState, Task, Priority};
//!
//! // Create and boot runtime
//! let mut runtime = Runtime::new();
//! runtime.boot();
//!
//! // Schedule work
//! runtime.schedule_work(|| println!("Hello from deferred task!"));
//!
//! // Schedule priority task
//! runtime.schedule_work_with_priority(Priority::HIGH, || {
//!     println!("High priority work!");
//! });
//!
//! // Process all pending work
//! while !runtime.is_idle() {
//!     runtime.tick();
//! }
//!
//! // Shutdown
//! runtime.shutdown();
//! assert_eq!(runtime.state(), RuntimeState::Stopping);
//! ```
//!
//! # Panic Safety
//!
//! The executor wraps task execution in `catch_unwind`, ensuring that a
//! panicking task doesn't bring down the entire editor. Failed tasks are
//! marked as such and counted in statistics.
//!
//! ```
//! use reovim_kernel::sched::Runtime;
//!
//! let mut runtime = Runtime::new();
//! runtime.boot();
//!
//! // This panic won't crash the runtime
//! runtime.schedule_work(|| panic!("intentional panic"));
//!
//! runtime.tick();
//!
//! let stats = runtime.stats();
//! assert_eq!(stats.tasks_failed, 1);
//! ```

mod executor;
mod priority;
mod runtime;
mod state;
mod task;
mod work_queue;

// Re-export all public types
pub use {
    executor::Executor,
    priority::{DEFAULT_CAPACITY as PRIORITY_QUEUE_DEFAULT_CAPACITY, PriorityQueue},
    runtime::{
        DEFAULT_BATCH_SIZE, DEFAULT_PRIORITY_QUEUE_CAPACITY, DEFAULT_WORK_QUEUE_CAPACITY, Runtime,
        RuntimeCommand, RuntimeConfig, RuntimeStats,
    },
    state::RuntimeState,
    task::{BoxedTask, Priority, Task, TaskId, TaskState},
    work_queue::{
        DEFAULT_CAPACITY as WORK_QUEUE_DEFAULT_CAPACITY, MAX_CAPACITY as WORK_QUEUE_MAX_CAPACITY,
        WorkQueue,
    },
};
