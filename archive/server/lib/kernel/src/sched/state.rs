//! Runtime lifecycle state machine.
//!
//! Linux equivalent: Task states in `kernel/sched/sched.h`
//!
//! The runtime transitions through these states during its lifecycle:
//! - `Booting` -> `Running` -> `Stopping`
//! - Any state can transition to `Emergency` on panic

/// Runtime lifecycle state.
///
/// Represents the current operating mode of the scheduler runtime.
///
/// # State Transitions
///
/// ```text
/// ┌─────────┐     ┌─────────┐     ┌──────────┐
/// │ Booting │────>│ Running │────>│ Stopping │
/// └─────────┘     └─────────┘     └──────────┘
///      │               │               │
///      └───────────────┴───────────────┘
///                      │
///                      v
///               ┌───────────┐
///               │ Emergency │
///               └───────────┘
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RuntimeState {
    /// Initializing subsystems, loading modules.
    ///
    /// Linux equivalent: `TASK_NEW`
    #[default]
    Booting,

    /// Normal operation, processing events.
    ///
    /// Linux equivalent: `TASK_RUNNING`
    Running,

    /// Graceful shutdown in progress.
    ///
    /// Remaining work is drained before full stop.
    ///
    /// Linux equivalent: `TASK_STOPPED`
    Stopping,

    /// Emergency shutdown (panic recovery mode).
    ///
    /// No new work accepted, cleanup only.
    ///
    /// Linux equivalent: Kernel oops handling
    Emergency,
}

impl RuntimeState {
    /// Check if runtime is in the running state.
    ///
    /// Only in `Running` state can new work be accepted.
    #[inline]
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Check if shutdown is in progress.
    ///
    /// Returns `true` for both `Stopping` and `Emergency` states.
    #[inline]
    #[must_use]
    pub const fn is_shutting_down(&self) -> bool {
        matches!(self, Self::Stopping | Self::Emergency)
    }

    /// Check if runtime can accept new work.
    ///
    /// Equivalent to `is_running()` - only running runtime accepts work.
    #[inline]
    #[must_use]
    pub const fn can_accept_work(&self) -> bool {
        self.is_running()
    }

    /// Check if this is a terminal state.
    ///
    /// Terminal states indicate the runtime should stop its main loop.
    #[inline]
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopping | Self::Emergency)
    }
}

impl std::fmt::Display for RuntimeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Booting => write!(f, "Booting"),
            Self::Running => write!(f, "Running"),
            Self::Stopping => write!(f, "Stopping"),
            Self::Emergency => write!(f, "Emergency"),
        }
    }
}
