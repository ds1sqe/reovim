use reovim_kernel_interface::bound::ZeroPolicy;
use reovim_kernel_interface::resource::ResourceAccessStamp;
use reovim_kernel_interface::resource::access::ResourceAccessDescriptor;
use reovim_kernel_interface::resource::bounds::{
    ResourceAccessBounds, ResourceCompletionScope, ResourcePayloadAccounting,
    ResourceStateBounds, ResourceTransferBounds,
};
use reovim_kernel_interface::resource::capability::{
    ChangeDiscovery, DestinationCreation, HistoricalAvailability,
    ResourceCapabilities, ResourceMutationGrammar, SnapshotConsistency,
    SourceAccessAuthority, SourceStability,
};
use reovim_kernel_interface::resource::carrier::{
    ResourceCarrierContract, ResourceCarrierContracts,
};
use reovim_kernel_interface::resource::state::ResourceStateModelBinding;

use crate::profile::{byte_bound, count_bound, safety, schema};

pub(super) fn build(
    model: &ResourceStateModelBinding,
    access: ResourceAccessStamp,
) -> ResourceAccessDescriptor {
    let capabilities = ResourceCapabilities::new(
        SourceAccessAuthority::ObserveOnly,
        SourceStability::Stable,
        SnapshotConsistency::WholeResource,
        HistoricalAvailability::CurrentOnly,
        ResourceMutationGrammar::None,
        DestinationCreation::NotGranted,
        ChangeDiscovery::None,
    )
    .expect("immutable source capability profile");
    let regions = ResourceCarrierContract::new(schema(0x3001), safety(0x3001))
        .expect("whole native resource selector");
    let payload_id = model.payload.type_identity();
    let payload =
        ResourceCarrierContract::new(schema(payload_id), safety(payload_id))
            .expect("selected native payload");
    let carriers = ResourceCarrierContracts::new(&regions, &payload)
        .expect("distinct Resource carrier roles");
    ResourceAccessDescriptor::new(
        schema(0x3000),
        access.lifecycle(),
        access.lifecycle(),
        model.schemas,
        capabilities,
        &carriers,
        &bounds(access.lifecycle().instance(), capabilities),
    )
}

fn bounds(owner: u128, caps: ResourceCapabilities) -> ResourceAccessBounds {
    let transfer = ResourceTransferBounds::new(
        count_bound(owner, 1, ZeroPolicy::Reject),
        byte_bound(owner, 1, ZeroPolicy::Reject),
        byte_bound(owner, crate::MAX_SOURCE_BYTES, ZeroPolicy::Allow),
    )
    .expect("bounded source transfer");
    let residency = 2 * crate::MAX_SOURCE_BYTES
        + size_of::<super::MemoryResource>() as u64
        + 1;
    let state = ResourceStateBounds::new(
        byte_bound(owner, residency, ZeroPolicy::Reject),
        count_bound(owner, 1, ZeroPolicy::Allow),
        count_bound(owner, 1, ZeroPolicy::Allow),
        count_bound(owner, 1, ZeroPolicy::Allow),
        count_bound(owner, 1, ZeroPolicy::Allow),
    )
    .expect("one live deferred operation");
    ResourceAccessBounds::new(
        &transfer,
        &state,
        ResourcePayloadAccounting::OwnedByAccess,
        ResourceCompletionScope::AccessIncarnation,
        caps,
    )
    .expect("immutable source bounds")
}
