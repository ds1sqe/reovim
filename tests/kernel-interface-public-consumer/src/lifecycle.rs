use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationOutcome, CancellationState, DeadlineState,
    ExecutionOutcome, FenceCause, InstallationOutcome,
};
use reovim_kernel_interface::lifecycle::{
    Generation, LifecycleIdentity, LifecycleState, ReadinessOutcome,
    RollbackOutcome, ShutdownOutcome, WorkDisposition,
};

use super::traits;

pub const fn const_cases() -> bool {
    identity_const_cases() && cancellation_const_cases() && derive_const_cases()
}

const fn identity_const_cases() -> bool {
    let generation = Generation::new(3);
    let identity = LifecycleIdentity::new(11, 12, generation);
    let same_incarnation = LifecycleIdentity::new(11, 12, Generation::new(4));
    let other = LifecycleIdentity::new(11, 13, generation);

    generation.value() == 3
        && identity.instance() == 11
        && identity.incarnation() == 12
        && identity.generation().value() == 3
        && identity.same_incarnation(same_incarnation)
        && !identity.same_generation(same_incarnation)
        && !identity.same_incarnation(other)
        && identity.same_generation(identity)
}

const fn cancellation_const_cases() -> bool {
    let cancellation = CancellationOutcome::new(
        CancellationCause::Replacement,
        ExecutionOutcome::OwnedNonterminal,
    );

    InstallationOutcome::Current.permits_installation()
        && !InstallationOutcome::Fenced(FenceCause::Deadline)
            .permits_installation()
        && !ExecutionOutcome::OwnedNonterminal.is_terminal()
        && ExecutionOutcome::NeverStarted.is_terminal()
        && ExecutionOutcome::Stopped.is_terminal()
        && ExecutionOutcome::ContainingProcessReaped.is_terminal()
        && cancellation_cause_code(cancellation.cause()) == 1
        && execution_code(cancellation.execution()) == 1
}

const fn cancellation_cause_code(value: CancellationCause) -> u8 {
    match value {
        CancellationCause::ExplicitRequest => 0,
        CancellationCause::Replacement => 1,
        CancellationCause::OwnerClosing => 2,
    }
}

const fn execution_code(value: ExecutionOutcome) -> u8 {
    match value {
        ExecutionOutcome::NeverStarted => 0,
        ExecutionOutcome::OwnedNonterminal => 1,
        ExecutionOutcome::Stopped => 2,
        ExecutionOutcome::ContainingProcessReaped => 3,
    }
}

const fn derive_const_cases() -> bool {
    lifecycle_derive_cases() && cancellation_derive_cases()
}

const fn lifecycle_derive_cases() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::lifecycle::Generation>()
        && crate::derive_witness::<
            ::reovim_kernel_interface::lifecycle::LifecycleIdentity,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::lifecycle::LifecycleState,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::lifecycle::ReadinessOutcome,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::lifecycle::RollbackOutcome,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::lifecycle::WorkDisposition,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::lifecycle::ShutdownOutcome,
        >()
}

const fn cancellation_derive_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::cancellation::CancellationCause,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::cancellation::CancellationState,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::cancellation::DeadlineState,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::cancellation::FenceCause,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::cancellation::InstallationOutcome,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::cancellation::ExecutionOutcome,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::cancellation::CancellationOutcome,
    >()
}

pub fn exercise() -> bool {
    identity_values()
        && lifecycle_variants()
        && readiness_variants()
        && work_variants()
        && cancellation_variants()
        && fence_variants()
        && outcome_variants()
}

fn identity_values() -> bool {
    traits(Generation::new(8))
        && traits(LifecycleIdentity::new(31, 32, Generation::new(8)))
}

fn lifecycle_variants() -> bool {
    traits(LifecycleState::Constructed)
        && traits(LifecycleState::Starting)
        && traits(LifecycleState::Ready)
        && traits(LifecycleState::RollingBack)
        && traits(LifecycleState::Closing)
        && traits(LifecycleState::Closed)
        && traits(LifecycleState::IncompleteShutdown)
}

fn readiness_variants() -> bool {
    traits(ReadinessOutcome::Ready)
        && traits(ReadinessOutcome::DiagnosticsDegraded)
        && traits(ReadinessOutcome::ActivationFailed(RollbackOutcome::Reverted))
        && traits(ReadinessOutcome::ActivationFailed(
            RollbackOutcome::IncompleteShutdown,
        ))
        && traits(RollbackOutcome::Reverted)
        && traits(RollbackOutcome::IncompleteShutdown)
}

fn work_variants() -> bool {
    traits(WorkDisposition::Drained)
        && traits(WorkDisposition::Discarded)
        && traits(WorkDisposition::Unresolved)
        && traits(ShutdownOutcome::Closed)
        && traits(ShutdownOutcome::IncompleteShutdown)
}

fn cancellation_variants() -> bool {
    traits(CancellationCause::ExplicitRequest)
        && traits(CancellationCause::Replacement)
        && traits(CancellationCause::OwnerClosing)
        && traits(CancellationState::Open)
        && traits(CancellationState::Requested(
            CancellationCause::ExplicitRequest,
        ))
        && traits(DeadlineState::Pending)
        && traits(DeadlineState::Reached)
}

fn fence_variants() -> bool {
    traits(FenceCause::Cancellation)
        && traits(FenceCause::Deadline)
        && traits(FenceCause::CallerTimeout)
        && traits(FenceCause::Replacement)
        && traits(FenceCause::Closing)
        && traits(FenceCause::Closed)
}

fn outcome_variants() -> bool {
    let cancellation = CancellationOutcome::new(
        CancellationCause::OwnerClosing,
        ExecutionOutcome::Stopped,
    );
    traits(InstallationOutcome::Current)
        && traits(InstallationOutcome::Fenced(FenceCause::Cancellation))
        && traits(ExecutionOutcome::NeverStarted)
        && traits(ExecutionOutcome::OwnedNonterminal)
        && traits(ExecutionOutcome::Stopped)
        && traits(ExecutionOutcome::ContainingProcessReaped)
        && traits(cancellation)
}
