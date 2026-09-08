use reovim_kernel_interface::carrier::{
    BorrowModel, CarrierSafetyDescriptor, ConcurrencySafety, CopyModel,
    DropModel, InvocationModel, InvocationSafety, LifetimeModel, MemorySafety,
    OwnershipModel, SharingModel, ThreadTransfer, TypeMismatch, UnsafeBoundary,
    UnwindModel,
};
use reovim_kernel_interface::contract::{
    CompatibleVersionRange, ContractId, ContractVersion,
};
use reovim_kernel_interface::diagnostic::{
    DiagnosticOutputCompletionBounds, DiagnosticOutputDescriptor,
    DiagnosticOutputError, DiagnosticOutputLimits, DiagnosticRecordContract,
    DiagnosticShutdownPolicy,
};
use reovim_kernel_interface::lifecycle::LifecycleIdentity;
use reovim_kernel_interface::operation::ContractSelection;

use super::QueueError;

const OUTPUT_CONTRACT: u128 = 0x0072_656f_7669_6d2e_6469_6167_2e6f_7574;
const RECORD_TYPE: u128 = 0x0072_656f_7669_6d2e_6469_6167_2e72_6563;

pub(super) fn build(
    owner: LifecycleIdentity,
    output: LifecycleIdentity,
    stream: LifecycleIdentity,
    capacity: u64,
    maximum_record: u64,
    maximum_bytes: u64,
) -> Result<DiagnosticOutputDescriptor, QueueError> {
    let contract =
        ContractSelection::new(OUTPUT_CONTRACT, 1, 1, 1).map_err(|_| {
            QueueError::Descriptor(DiagnosticOutputError::IncompatibleContract)
        })?;
    // Completion is a bounded local check, never a wait on external I/O.
    let completion =
        DiagnosticOutputCompletionBounds::new(1_000_000_000, 1_000_000_000)
            .map_err(|cause| {
                QueueError::Descriptor(DiagnosticOutputError::Bound(cause))
            })?;
    let limits = DiagnosticOutputLimits::new(
        output,
        16,
        maximum_record,
        capacity,
        maximum_bytes,
        completion,
    )
    .map_err(QueueError::Descriptor)?;
    DiagnosticOutputDescriptor::new(
        contract,
        output,
        owner,
        stream,
        limits,
        record_contract()?,
        DiagnosticShutdownPolicy::Drain,
    )
    .map_err(QueueError::Descriptor)
}

fn record_contract() -> Result<DiagnosticRecordContract, QueueError> {
    let versions = CompatibleVersionRange::new(
        ContractVersion::new(1),
        ContractVersion::new(1),
    )
    .map_err(|_| {
        QueueError::Descriptor(DiagnosticOutputError::IncoherentCarrier)
    })?;
    let memory = MemorySafety::new(
        OwnershipModel::Owned,
        BorrowModel::Shared,
        CopyModel::MoveOnly,
        DropModel::ReleasesOwnedState,
        LifetimeModel::Value,
    );
    let safety = CarrierSafetyDescriptor::new(
        ContractId::new(RECORD_TYPE),
        versions,
        memory,
        ConcurrencySafety::new(
            ThreadTransfer::Transferable,
            SharingModel::Immutable,
        ),
        InvocationSafety::new(
            InvocationModel::DataOnly,
            UnwindModel::NotInvocable,
        ),
        TypeMismatch::Reject,
        UnsafeBoundary::None,
    )
    .map_err(|_| {
        QueueError::Descriptor(DiagnosticOutputError::IncoherentCarrier)
    })?;
    Ok(DiagnosticRecordContract::new(ContractId::new(RECORD_TYPE), safety))
}
