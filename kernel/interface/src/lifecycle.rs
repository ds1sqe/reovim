//! Bootstrap lifecycle identities and outcome distinctions.

use reovim_foundation::identity::{IncarnationId, InstanceId};

/// Explicit generation used to fence installation into current state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generation(u64);

impl Generation {
    /// Constructs an owner-supplied generation.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the supplied generation scalar.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Exact instance, incarnation, and generation of one lifecycle scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LifecycleIdentity {
    instance: InstanceId,
    incarnation: IncarnationId,
    generation: Generation,
}

impl LifecycleIdentity {
    /// Constructs an identity from explicitly supplied scalar values.
    #[must_use]
    pub const fn new(
        instance: u128,
        incarnation: u128,
        generation: Generation,
    ) -> Self {
        Self {
            instance: InstanceId::new(instance),
            incarnation: IncarnationId::new(incarnation),
            generation,
        }
    }

    /// Returns the stable instance identity scalar.
    #[must_use]
    pub const fn instance(self) -> u128 {
        self.instance.value()
    }

    /// Returns the incarnation identity scalar.
    #[must_use]
    pub const fn incarnation(self) -> u128 {
        self.incarnation.value()
    }

    /// Returns the generation.
    #[must_use]
    pub const fn generation(self) -> Generation {
        self.generation
    }

    /// Reports whether both values name the same incarnation.
    #[must_use]
    pub const fn same_incarnation(self, other: Self) -> bool {
        self.instance.value() == other.instance.value()
            && self.incarnation.value() == other.incarnation.value()
    }

    /// Reports whether both values name the same complete lifecycle stamp.
    #[must_use]
    pub const fn same_generation(self, other: Self) -> bool {
        self.same_incarnation(other)
            && self.generation.value() == other.generation.value()
    }
}

/// Observable bootstrap lifecycle phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleState {
    /// Constructed dependencies are inert and advertise no readiness.
    Constructed,
    /// The lifecycle owner is validating and activating dependencies.
    Starting,
    /// Every required readiness predicate has passed atomically.
    Ready,
    /// Partial activation is being reversed in dependency order.
    RollingBack,
    /// New admission is closed while owned work is resolved.
    Closing,
    /// All owned execution stopped or its containing process was reaped.
    Closed,
    /// Nonterminal work remains owned after the shutdown bound.
    IncompleteShutdown,
}

/// Result considered when the owner publishes readiness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadinessOutcome {
    /// Every required component is usable without degradation.
    Ready,
    /// The declared safe surface is ready with diagnostics degraded.
    DiagnosticsDegraded,
    /// Activation failed, no readiness was published, and rollback terminated.
    ActivationFailed(RollbackOutcome),
}

/// Terminal disposition of one failed activation rollback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollbackOutcome {
    /// Provisional contributions were fully reversed.
    Reverted,
    /// Nonterminal work remains failed-owned after the rollback bound.
    IncompleteShutdown,
}

/// Disposition of bounded owned work during rollback or shutdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkDisposition {
    /// Admitted work reached a terminal outcome during the bounded drain.
    Drained,
    /// Work was discarded under the predeclared shutdown policy.
    Discarded,
    /// Nonterminal work remains owned after the shutdown bound.
    Unresolved,
}

/// Idempotent terminal result of one lifecycle shutdown operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownOutcome {
    /// All execution stopped or its containing process was reaped.
    Closed,
    /// Nonterminal work remains failed-owned after the shutdown bound.
    IncompleteShutdown,
}
