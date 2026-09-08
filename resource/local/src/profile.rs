//! The small, version-one reference profile selected by the concrete app.
//!
//! These constructors build only owned data-carrier and accounting metadata;
//! they do not discover providers or assign runtime authority. All callers
//! supply their own lifecycle and accounting owner identities.

use reovim_kernel_interface::bound::{
    BoundDescriptor, BoundValue, CheckedByteCount, CheckedCount, ZeroPolicy,
};
use reovim_kernel_interface::carrier::{
    BorrowModel, CarrierSafetyDescriptor, ConcurrencySafety, CopyModel,
    DropModel, InvocationModel, InvocationSafety, LifetimeModel, MemorySafety,
    OwnershipModel, SharingModel, ThreadTransfer, TypeMismatch, UnsafeBoundary,
    UnwindModel,
};
use reovim_kernel_interface::contract::{
    CompatibleVersionRange, ContractId, ContractVersion,
};
use reovim_kernel_interface::identity::InstanceId;
use reovim_kernel_interface::operation::ContractSelection;

/// Selects version one of an explicitly named reference schema.
///
/// # Panics
/// Only if the fixed version-one range becomes invalid.
#[must_use]
pub fn schema(identity: u128) -> ContractSelection {
    ContractSelection::new(identity, 1, 1, 1).expect("version-one profile")
}

/// Describes owned, immutable, move-only encoded data without callbacks.
///
/// # Panics
/// Only if the fixed data-carrier safety declaration becomes incoherent.
#[must_use]
pub fn safety(identity: u128) -> CarrierSafetyDescriptor {
    let versions = CompatibleVersionRange::new(
        ContractVersion::new(1),
        ContractVersion::new(1),
    )
    .expect("version-one profile");
    CarrierSafetyDescriptor::new(
        ContractId::new(identity),
        versions,
        MemorySafety::new(
            OwnershipModel::Owned,
            BorrowModel::Shared,
            CopyModel::MoveOnly,
            DropModel::ReleasesOwnedState,
            LifetimeModel::Value,
        ),
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
    .expect("owned encoded data profile")
}

/// Makes a zero-allowing count charge for explicit work/accounting bounds.
///
/// # Panics
/// Only if the zero-allowing count constructor rejects its input.
#[must_use]
pub fn count(value: u64) -> CheckedCount {
    CheckedCount::new(value, ZeroPolicy::Allow).expect("allowed count")
}

/// Makes a zero-allowing byte charge for explicit residency accounting.
///
/// # Panics
/// Only if the zero-allowing byte constructor rejects its input.
#[must_use]
pub fn bytes(value: u64) -> CheckedByteCount {
    CheckedByteCount::new(value, ZeroPolicy::Allow).expect("allowed bytes")
}

/// Declares a shared per-object/aggregate reference count reservation.
///
/// # Panics
/// Panics when a zero maximum is paired with a rejecting zero policy.
#[must_use]
pub fn count_bound(
    owner: u128,
    maximum: u64,
    zero: ZeroPolicy,
) -> BoundDescriptor {
    let maximum = CheckedCount::new(maximum, zero).expect("profile count");
    BoundDescriptor::new(
        BoundValue::Count(maximum),
        BoundValue::Count(maximum),
        zero,
        InstanceId::new(owner),
    )
    .expect("same count limits")
}

/// Declares a shared per-object/aggregate reference byte reservation.
///
/// # Panics
/// Panics when a zero maximum is paired with a rejecting zero policy.
#[must_use]
pub fn byte_bound(
    owner: u128,
    maximum: u64,
    zero: ZeroPolicy,
) -> BoundDescriptor {
    let maximum = CheckedByteCount::new(maximum, zero).expect("profile bytes");
    BoundDescriptor::new(
        BoundValue::Bytes(maximum),
        BoundValue::Bytes(maximum),
        zero,
        InstanceId::new(owner),
    )
    .expect("same byte limits")
}
