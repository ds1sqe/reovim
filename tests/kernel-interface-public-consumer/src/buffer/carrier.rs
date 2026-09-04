use reovim_kernel_interface::bound::BoundError;
use reovim_kernel_interface::buffer::carrier::{
    BufferCarrierContract, BufferCarrierContracts, BufferCarrierError,
    BufferCarrierPreparation, BufferCarrierRole, BufferCarrierStamp,
    BufferPlanCarrier,
};
use reovim_kernel_interface::buffer::plan::BufferPlanDescriptor;
use reovim_kernel_interface::carrier::{
    BorrowModel, CarrierSafetyDescriptor, ConcurrencySafety, CopyModel,
    DropModel, InvocationModel, InvocationSafety, LifetimeModel, MemorySafety,
    OwnershipModel, SharingModel, ThreadTransfer, TypeMismatch, UnsafeBoundary,
    UnwindModel,
};
use reovim_kernel_interface::contract::{
    CompatibleVersionRange, ContractId, ContractVersion,
};

use crate::error_traits;

const ORIGIN_TYPE: u128 = 811;
const OPERATION_TYPE: u128 = 812;
const PRECONDITION_TYPE: u128 = 813;

pub const fn const_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::buffer::carrier::BufferCarrierRole,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::buffer::carrier::BufferCarrierStamp,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::buffer::carrier::BufferCarrierContract,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::buffer::carrier::BufferCarrierContracts,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::buffer::carrier::BufferCarrierError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::buffer::carrier::BufferCarrierPreparation,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::buffer::carrier::BufferPlanCarrier,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::buffer::carrier::BufferCarrierRejection,
    >()
}

pub fn contracts() -> Option<BufferCarrierContracts> {
    let origin = carrier_contract(ORIGIN_TYPE)?;
    let operations = carrier_contract(OPERATION_TYPE)?;
    let preconditions = carrier_contract(PRECONDITION_TYPE)?;
    BufferCarrierContracts::new(&origin, &operations, &preconditions).ok()
}

pub fn make_carrier(
    descriptor: &BufferPlanDescriptor,
    role: BufferCarrierRole,
    items: u64,
    bytes: &[u8],
) -> Option<BufferPlanCarrier> {
    let contract = contract(descriptor.carriers(), role);
    let size = u64::try_from(bytes.len()).ok()?;
    let preparation = descriptor
        .prepare_carrier(role, contract.selected_stamp(), items, size)
        .ok()?;
    preparation.bind(descriptor, bytes.into()).ok()
}

pub fn exercise() -> bool {
    contract_cases()
        && valid_carriers()
        && preparation_errors()
        && binding_error()
}

fn contract_cases() -> bool {
    let Some(origin) = carrier_contract(ORIGIN_TYPE) else {
        return false;
    };
    let Some(operations) = carrier_contract(OPERATION_TYPE) else {
        return false;
    };
    let Some(preconditions) = carrier_contract(PRECONDITION_TYPE) else {
        return false;
    };
    let duplicate =
        BufferCarrierContracts::new(&origin, &origin, &preconditions);
    let invalid = invalid_carrier_contract(ORIGIN_TYPE);
    let Some(contracts) =
        BufferCarrierContracts::new(&origin, &operations, &preconditions).ok()
    else {
        return false;
    };
    let duplicate_code = match duplicate {
        Err(BufferCarrierError::DuplicateTypeIdentity {
            first: BufferCarrierRole::Origin,
            second: BufferCarrierRole::Operations,
        }) => 0,
        _ => 1,
    };
    contracts.origin().schema().identity() == ORIGIN_TYPE
        && contracts.operations().schema().identity() == OPERATION_TYPE
        && contracts.preconditions().schema().identity() == PRECONDITION_TYPE
        && contracts.origin().selected_stamp()
            == BufferCarrierStamp::new(ORIGIN_TYPE, 1)
        && duplicate_code == 0
        && invalid == Err(BufferCarrierError::IncoherentSafety)
        && error_traits(&BufferCarrierError::IncoherentSafety)
}

fn valid_carriers() -> bool {
    let Some(descriptor) = super::descriptor() else { return false };
    let Some(origin) =
        make_carrier(&descriptor, BufferCarrierRole::Origin, 1, b"origin")
    else {
        return false;
    };
    let Some(operations) =
        make_carrier(&descriptor, BufferCarrierRole::Operations, 1, b"hello")
    else {
        return false;
    };
    let Some(preconditions) =
        make_carrier(&descriptor, BufferCarrierRole::Preconditions, 0, b"")
    else {
        return false;
    };
    origin.role() == BufferCarrierRole::Origin
        && origin.items().value() == 1
        && origin.size().value() == 6
        && origin.bytes() == b"origin"
        && operations.role() == BufferCarrierRole::Operations
        && operations.items().value() == 1
        && operations.bytes() == b"hello"
        && preconditions.role() == BufferCarrierRole::Preconditions
        && preconditions.items().value() == 0
        && preconditions.bytes().is_empty()
        && preconditions.into_bytes().is_empty()
        && operations.into_bytes().as_ref() == b"hello"
        && origin.into_bytes().as_ref() == b"origin"
}

fn preparation_errors() -> bool {
    let Some(descriptor) = super::descriptor() else { return false };
    let wrong_type = descriptor.prepare_carrier(
        BufferCarrierRole::Origin,
        BufferCarrierStamp::new(999, 1),
        1,
        1,
    );
    let wrong_version = descriptor.prepare_carrier(
        BufferCarrierRole::Operations,
        BufferCarrierStamp::new(OPERATION_TYPE, 2),
        1,
        1,
    );
    let origin_count = descriptor.prepare_carrier(
        BufferCarrierRole::Origin,
        descriptor.carriers().origin().selected_stamp(),
        0,
        1,
    );
    let empty_operations = descriptor.prepare_carrier(
        BufferCarrierRole::Operations,
        descriptor.carriers().operations().selected_stamp(),
        0,
        0,
    );
    let precondition_shape = descriptor.prepare_carrier(
        BufferCarrierRole::Preconditions,
        descriptor.carriers().preconditions().selected_stamp(),
        0,
        1,
    );
    let reverse_precondition_shape = descriptor.prepare_carrier(
        BufferCarrierRole::Preconditions,
        descriptor.carriers().preconditions().selected_stamp(),
        1,
        0,
    );
    let operation_count = descriptor.prepare_carrier(
        BufferCarrierRole::Operations,
        descriptor.carriers().operations().selected_stamp(),
        5,
        5,
    );
    let origin_bytes = descriptor.prepare_carrier(
        BufferCarrierRole::Origin,
        descriptor.carriers().origin().selected_stamp(),
        1,
        9,
    );
    let type_code = match wrong_type {
        Err(BufferCarrierError::TypeMismatch {
            role: BufferCarrierRole::Origin,
        }) => 0,
        _ => 1,
    };
    let version_code = match wrong_version {
        Err(BufferCarrierError::IncompatibleSchema {
            role: BufferCarrierRole::Operations,
        }) => 0,
        _ => 1,
    };
    type_code == 0
        && version_code == 0
        && origin_count
            == Err(BufferCarrierError::OriginRecordCount { actual: 0 })
        && zero_operations(empty_operations)
        && precondition_shape
            == Err(BufferCarrierError::PreconditionShape { items: 0, bytes: 1 })
        && reverse_precondition_shape
            == Err(BufferCarrierError::PreconditionShape { items: 1, bytes: 0 })
        && exhausted_operations(operation_count)
        && origin_bytes
            == Err(BufferCarrierError::Bound {
                role: BufferCarrierRole::Origin,
                cause: BoundError::Exhausted {
                    unit: reovim_kernel_interface::bound::BoundUnit::Bytes,
                    requested: 9,
                    maximum: 8,
                },
            })
}

const fn zero_operations(
    result: Result<BufferCarrierPreparation, BufferCarrierError>,
) -> bool {
    let code = match result {
        Err(BufferCarrierError::Bound {
            role: BufferCarrierRole::Operations,
            cause: BoundError::ZeroRejected { .. },
        }) => 0,
        _ => 1,
    };
    code == 0
}

const fn exhausted_operations(
    result: Result<BufferCarrierPreparation, BufferCarrierError>,
) -> bool {
    let code = match result {
        Err(BufferCarrierError::Bound {
            role: BufferCarrierRole::Operations,
            cause: BoundError::Exhausted { requested: 5, maximum: 4, .. },
        }) => 0,
        _ => 1,
    };
    code == 0
}

fn binding_error() -> bool {
    let Some(descriptor) = super::descriptor() else { return false };
    let stamp = descriptor.carriers().origin().selected_stamp();
    let Some(preparation) =
        descriptor.prepare_carrier(BufferCarrierRole::Origin, stamp, 1, 3).ok()
    else {
        return false;
    };
    let Err(rejection) =
        preparation.bind(&descriptor, [7_u8, 8].as_slice().into())
    else {
        return false;
    };
    let cause_code = match rejection.cause() {
        BufferCarrierError::LengthMismatch {
            role: BufferCarrierRole::Origin,
            prepared: 3,
            actual: 2,
        } => 0,
        _ => 1,
    };
    cause_code == 0
        && rejection.bytes() == [7, 8]
        && rejection.into_bytes().as_ref() == [7, 8]
}

const fn contract(
    contracts: &BufferCarrierContracts,
    role: BufferCarrierRole,
) -> &BufferCarrierContract {
    match role {
        BufferCarrierRole::Origin => contracts.origin(),
        BufferCarrierRole::Operations => contracts.operations(),
        BufferCarrierRole::Preconditions => contracts.preconditions(),
    }
}

fn carrier_contract(type_identity: u128) -> Option<BufferCarrierContract> {
    let schema = crate::resource::selection(type_identity)?;
    let safety = carrier_safety(type_identity, CopyModel::MoveOnly)?;
    BufferCarrierContract::new(schema, safety).ok()
}

fn invalid_carrier_contract(
    type_identity: u128,
) -> Result<BufferCarrierContract, BufferCarrierError> {
    let Some(schema) = crate::resource::selection(type_identity) else {
        return Err(BufferCarrierError::IncoherentSafety);
    };
    let Some(safety) = carrier_safety(type_identity, CopyModel::Copy) else {
        return Err(BufferCarrierError::IncoherentSafety);
    };
    BufferCarrierContract::new(schema, safety)
}

fn carrier_safety(
    type_identity: u128,
    copying: CopyModel,
) -> Option<CarrierSafetyDescriptor> {
    let versions = CompatibleVersionRange::new(
        ContractVersion::new(1),
        ContractVersion::new(1),
    )
    .ok()?;
    let memory = MemorySafety::new(
        OwnershipModel::Owned,
        BorrowModel::Shared,
        copying,
        DropModel::ReleasesOwnedState,
        LifetimeModel::Value,
    );
    let concurrency = ConcurrencySafety::new(
        ThreadTransfer::Transferable,
        SharingModel::Immutable,
    );
    let invocation = InvocationSafety::new(
        InvocationModel::DataOnly,
        UnwindModel::NotInvocable,
    );
    CarrierSafetyDescriptor::new(
        ContractId::new(type_identity),
        versions,
        memory,
        concurrency,
        invocation,
        TypeMismatch::Reject,
        UnsafeBoundary::None,
    )
    .ok()
}
