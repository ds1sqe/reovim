//! Cancellation, deadline, installation, and execution distinctions.

/// Cause carried by one explicit cancellation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancellationCause {
    /// The owning cancellation authority requested cancellation directly.
    ExplicitRequest,
    /// Replacement requires old-generation work to cooperate with shutdown.
    Replacement,
    /// The owning lifecycle requested cancellation while Closing.
    OwnerClosing,
}

/// Whether cooperative cancellation has been requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancellationState {
    /// No cancellation request has been observed.
    Open,
    /// Cancellation was requested for the stated cause.
    Requested(CancellationCause),
}

/// Independent observation of one declared deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeadlineState {
    /// The deadline has not been observed as reached.
    Pending,
    /// The deadline was observed as reached.
    Reached,
}

/// Cause that retires installation authority for affected work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FenceCause {
    /// A cancellation policy fenced installation.
    Cancellation,
    /// A declared deadline fenced installation.
    Deadline,
    /// The caller ended its wait without proving execution terminal.
    CallerTimeout,
    /// A newer incompatible generation replaced the observed generation.
    Replacement,
    /// The owning lifecycle began Closing.
    Closing,
    /// The owning lifecycle is Closed.
    Closed,
}

/// Whether a result may still install into the observed generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallationOutcome {
    /// Installation authority remains current.
    Current,
    /// Installation authority ended for the stated independent cause.
    Fenced(FenceCause),
}

impl InstallationOutcome {
    /// Reports whether installation into current state remains permitted.
    ///
    /// This result says nothing about whether execution has stopped.
    #[must_use]
    pub const fn permits_installation(self) -> bool {
        matches_current(self)
    }
}

/// Separately observed terminality of still-owned execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionOutcome {
    /// Cancellation won before execution began.
    NeverStarted,
    /// Execution is not proved terminal and remains owned.
    OwnedNonterminal,
    /// Execution was observed to have stopped.
    Stopped,
    /// An isolation owner proved the containing process was reaped.
    ContainingProcessReaped,
}

impl ExecutionOutcome {
    /// Reports whether the execution-terminal requirement is satisfied.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        execution_code(self) != 1
    }
}

/// Result of one cooperative cancellation attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CancellationOutcome {
    cause: CancellationCause,
    execution: ExecutionOutcome,
}

impl CancellationOutcome {
    /// Constructs a cancellation result without inferring a generation fence.
    #[must_use]
    pub const fn new(
        cause: CancellationCause,
        execution: ExecutionOutcome,
    ) -> Self {
        Self { cause, execution }
    }

    /// Returns why cooperative cancellation was requested.
    #[must_use]
    pub const fn cause(self) -> CancellationCause {
        self.cause
    }

    /// Returns the separately observed execution disposition.
    #[must_use]
    pub const fn execution(self) -> ExecutionOutcome {
        self.execution
    }
}

const fn matches_current(outcome: InstallationOutcome) -> bool {
    match outcome {
        InstallationOutcome::Current => true,
        InstallationOutcome::Fenced(_) => false,
    }
}

const fn execution_code(outcome: ExecutionOutcome) -> u8 {
    match outcome {
        ExecutionOutcome::NeverStarted => 0,
        ExecutionOutcome::OwnedNonterminal => 1,
        ExecutionOutcome::Stopped => 2,
        ExecutionOutcome::ContainingProcessReaped => 3,
    }
}
