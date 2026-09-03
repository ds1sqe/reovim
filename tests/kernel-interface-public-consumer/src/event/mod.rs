mod canonical;
mod derives;
mod descriptor;
mod outcomes;
mod sequence;
mod snapshot;

use reovim_kernel_interface::carrier::{
    BorrowModel, CarrierSafetyDescriptor, ConcurrencySafety, CopyModel,
    DropModel, InvocationModel, InvocationSafety, LifetimeModel, MemorySafety,
    OwnershipModel, SharingModel, ThreadTransfer, TypeMismatch, UnsafeBoundary,
    UnwindModel,
};
use reovim_kernel_interface::clock::ClockDescriptor;
use reovim_kernel_interface::contract::{
    CompatibleVersionRange, ContractId, ContractVersion,
};
use reovim_kernel_interface::event::{
    EVENT_RING_MAX_BYTES, STARTUP_CANONICAL_MAX_BYTES, StartupEventDescriptor,
    StartupEventIdentities, StartupEventLimits, StartupEventRecord,
};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::operation::ContractSelection;

pub const EVENT_CONTRACT: u128 = 0x11;
pub const STARTUP_FAMILY: u128 = 0x12;
pub const REMEDIATION_CATALOG: u128 = 0x13;
pub const CANONICAL_REFERENCE_TYPE: u128 = 0x14;
pub const STREAM_INSTANCE: u128 = 0x21;
pub const STREAM_INCARNATION: u128 = 0x22;
pub const PRODUCER: u128 = 0x51;
pub const CORRELATION: u128 = 0x52;

pub const READY_LINE: &[u8] = b"reovim-event/v1 \
stream=00000000000000000000000000000022 \
sequence=1 \
timestamp_ns=5 \
producer=00000000000000000000000000000051 \
family=kernel.startup \
schema=1 \
severity=info \
correlation=00000000000000000000000000000052 \
actionability=none \
remediation=none \
outcome=ready \
rollback=none \
active=3 \
degraded=0 \
unavailable=0 \
bootstrap_health=clear\n";

pub const DEGRADED_LINE: &[u8] = b"reovim-event/v1 \
stream=00000000000000000000000000000022 \
sequence=2 \
timestamp_ns=6 \
producer=00000000000000000000000000000051 \
family=kernel.startup \
schema=1 \
severity=warning \
correlation=00000000000000000000000000000052 \
actionability=operator \
remediation=kernel.bootstrap/1/diagnostic-path-degraded \
outcome=diagnostics-degraded \
rollback=none \
active=1 \
degraded=2 \
unavailable=0 \
bootstrap_health=imported\n";

pub const FAILED_LINE: &[u8] = b"reovim-event/v1 \
stream=00000000000000000000000000000022 \
sequence=3 \
timestamp_ns=7 \
producer=00000000000000000000000000000051 \
family=kernel.startup \
schema=1 \
severity=error \
correlation=00000000000000000000000000000052 \
actionability=operator \
remediation=kernel.bootstrap/1/activation-failed \
outcome=activation-failed \
rollback=reverted \
active=0 \
degraded=1 \
unavailable=2 \
bootstrap_health=pending\n";

pub const fn const_cases() -> bool {
    descriptor::const_cases()
        && sequence::const_cases()
        && snapshot::const_cases()
        && derives::const_cases()
}

pub fn exercise() -> bool {
    canonical::exercise()
        && snapshot::exercise()
        && outcomes::exercise()
        && derives::exercise()
}

pub const fn lifecycle_owner() -> LifecycleIdentity {
    LifecycleIdentity::new(0x31, 0x32, Generation::new(4))
}

pub const fn stream_identity() -> LifecycleIdentity {
    LifecycleIdentity::new(
        STREAM_INSTANCE,
        STREAM_INCARNATION,
        Generation::new(3),
    )
}

pub const fn make_clock_descriptor() -> Option<ClockDescriptor> {
    let Ok(contract) = ContractSelection::new(0x41, 1, 1, 2) else {
        return None;
    };
    let lifecycle = LifecycleIdentity::new(0x42, 0x43, Generation::new(5));
    match ClockDescriptor::new(
        contract,
        lifecycle,
        lifecycle_owner(),
        1,
        1_000,
        0x44,
    ) {
        Ok(descriptor) => Some(descriptor),
        Err(_) => None,
    }
}

pub const fn make_identities() -> StartupEventIdentities {
    StartupEventIdentities::new(
        EVENT_CONTRACT,
        STARTUP_FAMILY,
        REMEDIATION_CATALOG,
        CANONICAL_REFERENCE_TYPE,
        stream_identity(),
        lifecycle_owner(),
    )
}

pub const fn make_limits() -> Option<StartupEventLimits> {
    match StartupEventLimits::new(STREAM_INSTANCE, 8, 16, 32, 2_048, 4, 2_048) {
        Ok(limits) => Some(limits),
        Err(_) => None,
    }
}

pub const fn make_canonical_reference_safety(
    sharing: SharingModel,
) -> Option<CarrierSafetyDescriptor> {
    let Ok(versions) = CompatibleVersionRange::new(
        ContractVersion::new(1),
        ContractVersion::new(2),
    ) else {
        return None;
    };
    let memory = MemorySafety::new(
        OwnershipModel::Borrowed,
        BorrowModel::Shared,
        CopyModel::Copy,
        DropModel::Trivial,
        LifetimeModel::IncarnationBound,
    );
    let concurrency =
        ConcurrencySafety::new(ThreadTransfer::Transferable, sharing);
    let invocation = InvocationSafety::new(
        InvocationModel::DataOnly,
        UnwindModel::NotInvocable,
    );
    match CarrierSafetyDescriptor::new(
        ContractId::new(CANONICAL_REFERENCE_TYPE),
        versions,
        memory,
        concurrency,
        invocation,
        TypeMismatch::Reject,
        UnsafeBoundary::None,
    ) {
        Ok(safety) => Some(safety),
        Err(_) => None,
    }
}

pub const fn make_descriptor() -> Option<StartupEventDescriptor> {
    let Ok(contract) = ContractSelection::new(EVENT_CONTRACT, 1, 1, 2) else {
        return None;
    };
    let Some(clock) = make_clock_descriptor() else {
        return None;
    };
    let Some(limits) = make_limits() else {
        return None;
    };
    let Some(safety) = make_canonical_reference_safety(SharingModel::Immutable)
    else {
        return None;
    };
    match StartupEventDescriptor::new(
        contract,
        make_identities(),
        clock,
        limits,
        safety,
    ) {
        Ok(descriptor) => Some(descriptor),
        Err(_) => None,
    }
}

pub fn make_ready_record() -> Option<StartupEventRecord<'static>> {
    let descriptor = make_descriptor()?;
    canonical::make_ready_record(&descriptor)
}
