use reovim_kernel_interface::bound::BoundError;
use reovim_kernel_interface::carrier::{
    BorrowModel, CarrierSafetyDescriptor, ConcurrencySafety, CopyModel,
    DropModel, InvocationModel, InvocationSafety, LifetimeModel, MemorySafety,
    OwnershipModel, SharingModel, ThreadTransfer, TypeMismatch, UnsafeBoundary,
    UnwindModel,
};
use reovim_kernel_interface::contract::{
    CompatibleVersionRange, ContractId, ContractVersion,
};
use reovim_kernel_interface::resource::carrier::{
    ResourceCarrierContract, ResourceCarrierContracts, ResourceCarrierError,
    ResourceCarrierRole, ResourceCarrierStamp, ResourcePayloadPreparation,
    ResourceRegionPreparation,
};

use super::selection;
use crate::error_traits;

const REGION_TYPE: u128 = 401;
const PAYLOAD_TYPE: u128 = 402;

pub const fn const_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::resource::carrier::ResourceCarrierRole,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::carrier::ResourceCarrierStamp,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::resource::carrier::ResourceCarrierContract,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::resource::carrier::ResourceCarrierContracts,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::carrier::ResourceCarrierError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::carrier::
            ResourceRegionPreparation,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::resource::carrier::
            ResourcePayloadPreparation,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::resource::carrier::ResourceRegionCarrier,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::resource::carrier::ResourcePayloadCarrier,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::resource::carrier::ResourceCarrierRejection,
    >()
}

pub fn contracts() -> Option<ResourceCarrierContracts> {
    let regions = carrier_contract(REGION_TYPE)?;
    let payload = carrier_contract(PAYLOAD_TYPE)?;
    ResourceCarrierContracts::new(&regions, &payload).ok()
}

pub fn exercise() -> bool {
    contract_cases() && region_cases() && payload_cases()
}

fn contract_cases() -> bool {
    let Some(regions) = carrier_contract(REGION_TYPE) else {
        return false;
    };
    let Some(payload) = carrier_contract(PAYLOAD_TYPE) else {
        return false;
    };
    let duplicate = ResourceCarrierContracts::new(&regions, &regions);
    let invalid = invalid_carrier_contract(REGION_TYPE);
    let Some(contracts) =
        ResourceCarrierContracts::new(&regions, &payload).ok()
    else {
        return false;
    };
    contracts.regions().schema().identity() == REGION_TYPE
        && contracts.payload().schema().identity() == PAYLOAD_TYPE
        && contracts.regions().selected_stamp()
            == ResourceCarrierStamp::new(REGION_TYPE, 1)
        && duplicate == Err(ResourceCarrierError::DuplicateTypeIdentity)
        && invalid == Err(ResourceCarrierError::IncoherentSafety)
        && error_traits(&ResourceCarrierError::IncoherentSafety)
}

fn region_cases() -> bool {
    let Some(descriptor) = super::access::stable_descriptor() else {
        return false;
    };
    let stamp = descriptor.carriers().regions().selected_stamp();
    let Some(prepared) = descriptor.prepare_region_carrier(stamp, 1, 3).ok()
    else {
        return false;
    };
    let Some(carrier) = descriptor
        .bind_region_carrier(prepared, [1_u8, 2, 3].as_slice().into())
        .ok()
    else {
        return false;
    };
    preparation_fields(prepared)
        && carrier.stamp() == stamp
        && carrier.regions().value() == 1
        && carrier.size().value() == 3
        && carrier.bytes() == [1, 2, 3]
        && region_preparation_errors(&descriptor)
        && region_binding_error(&descriptor, stamp)
        && carrier.into_bytes().as_ref() == [1, 2, 3]
}

const fn preparation_fields(prepared: ResourceRegionPreparation) -> bool {
    prepared.stamp().type_identity() == REGION_TYPE
        && prepared.stamp().schema_version() == 1
        && prepared.regions().value() == 1
        && prepared.size().value() == 3
}

fn region_preparation_errors(
    descriptor: &reovim_kernel_interface::resource::access::
        ResourceAccessDescriptor,
) -> bool {
    let wrong = descriptor.prepare_region_carrier(
        ResourceCarrierStamp::new(999, 1),
        1,
        3,
    );
    let version = descriptor.prepare_region_carrier(
        ResourceCarrierStamp::new(REGION_TYPE, 2),
        1,
        3,
    );
    let regions = descriptor.prepare_region_carrier(
        ResourceCarrierStamp::new(REGION_TYPE, 1),
        2,
        3,
    );
    region_type_mismatch(wrong)
        && region_version_mismatch(version)
        && bound_exhausted(&regions, ResourceCarrierRole::Regions, 2, 1)
}

const fn region_type_mismatch(
    result: Result<ResourceRegionPreparation, ResourceCarrierError>,
) -> bool {
    match result {
        Err(ResourceCarrierError::TypeMismatch {
            role: ResourceCarrierRole::Regions,
        }) => true,
        Err(_) | Ok(_) => false,
    }
}

const fn region_version_mismatch(
    result: Result<ResourceRegionPreparation, ResourceCarrierError>,
) -> bool {
    match result {
        Err(ResourceCarrierError::IncompatibleSchema {
            role: ResourceCarrierRole::Regions,
        }) => true,
        Err(_) | Ok(_) => false,
    }
}

fn region_binding_error(
    descriptor: &reovim_kernel_interface::resource::access::
        ResourceAccessDescriptor,
    stamp: ResourceCarrierStamp,
) -> bool {
    let Some(prepared) = descriptor.prepare_region_carrier(stamp, 1, 3).ok()
    else {
        return false;
    };
    let Err(rejection) =
        descriptor.bind_region_carrier(prepared, [7_u8, 8].as_slice().into())
    else {
        return false;
    };
    let cause_matches = match rejection.cause() {
        ResourceCarrierError::LengthMismatch {
            role: ResourceCarrierRole::Regions,
            prepared: 3,
            actual: 2,
        } => 0,
        _ => 1,
    };
    cause_matches == 0
        && rejection.bytes() == [7, 8]
        && rejection.into_bytes().as_ref() == [7, 8]
}

fn payload_cases() -> bool {
    let Some(descriptor) = super::access::stable_descriptor() else {
        return false;
    };
    let stamp = descriptor.carriers().payload().selected_stamp();
    let Some(prepared) = descriptor.prepare_payload_carrier(stamp, 1, 5).ok()
    else {
        return false;
    };
    let Some(payload) = descriptor
        .bind_payload_carrier(prepared, b"hello".as_slice().into())
        .ok()
    else {
        return false;
    };
    payload_preparation_fields(prepared)
        && payload.stamp() == stamp
        && payload.regions().value() == 1
        && payload.size().value() == 5
        && payload.bytes() == b"hello"
        && payload.into_bytes().as_ref() == b"hello"
}

const fn payload_preparation_fields(
    prepared: ResourcePayloadPreparation,
) -> bool {
    prepared.stamp().type_identity() == PAYLOAD_TYPE
        && prepared.stamp().schema_version() == 1
        && prepared.regions().value() == 1
        && prepared.size().value() == 5
}

fn bound_exhausted<T>(
    result: &Result<T, ResourceCarrierError>,
    role: ResourceCarrierRole,
    requested: u64,
    maximum: u64,
) -> bool {
    match result {
        Err(ResourceCarrierError::Bound {
            role: found,
            cause:
                BoundError::Exhausted { requested: actual, maximum: limit, .. },
        }) => *found == role && *actual == requested && *limit == maximum,
        Err(_) | Ok(_) => false,
    }
}

fn carrier_contract(type_identity: u128) -> Option<ResourceCarrierContract> {
    let schema = selection(type_identity)?;
    let safety = carrier_safety(type_identity, CopyModel::MoveOnly)?;
    ResourceCarrierContract::new(schema, safety).ok()
}

fn invalid_carrier_contract(
    type_identity: u128,
) -> Result<ResourceCarrierContract, ResourceCarrierError> {
    let Some(schema) = selection(type_identity) else {
        return Err(ResourceCarrierError::IncoherentSafety);
    };
    let Some(safety) = carrier_safety(type_identity, CopyModel::Copy) else {
        return Err(ResourceCarrierError::IncoherentSafety);
    };
    ResourceCarrierContract::new(schema, safety)
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
