use alloc::vec::Vec;

use reovim_kernel_interface::bound::ZeroPolicy;
use reovim_kernel_interface::buffer::bounds::BufferPlanBounds;
use reovim_kernel_interface::buffer::carrier::{
    BufferCarrierContract, BufferCarrierContracts,
};
use reovim_kernel_interface::buffer::plan::BufferPlanDescriptor;
use reovim_kernel_interface::identity::InstanceId;
use reovim_kernel_interface::lifecycle::LifecycleIdentity;
use reovim_kernel_interface::resource::ResourceValueSchemas;
use reovim_kernel_interface::resource::carrier::ResourceCarrierStamp;
use reovim_kernel_interface::resource::state::{
    ResourceStateError, ResourceStateModelBinding,
};

use crate::profile::{byte_bound, count_bound, safety, schema};

pub fn binding(
    base: u128,
    owner: LifecycleIdentity,
) -> ResourceStateModelBinding {
    ResourceStateModelBinding {
        contract: schema(base),
        lifecycle: owner,
        schemas: ResourceValueSchemas::new(
            schema(base + 1),
            schema(base + 2),
            schema(base + 3),
            schema(base + 4),
        ),
        payload: ResourceCarrierStamp::new(base + 5, 1),
    }
}

pub fn plan(base: u128, owner: LifecycleIdentity) -> BufferPlanDescriptor {
    let carrier = |id| {
        BufferCarrierContract::new(schema(id), safety(id))
            .expect("reference data carrier")
    };
    let carriers = BufferCarrierContracts::new(
        &carrier(base + 7),
        &carrier(base + 8),
        &carrier(base + 9),
    )
    .expect("distinct native roles");
    let id = owner.instance();
    let bounds = BufferPlanBounds::new(
        InstanceId::new(id),
        count_bound(id, 1, ZeroPolicy::Reject),
        count_bound(id, 1, ZeroPolicy::Allow),
        byte_bound(id, 1, ZeroPolicy::Reject),
        byte_bound(id, 9, ZeroPolicy::Reject),
        byte_bound(id, 1, ZeroPolicy::Allow),
        byte_bound(id, 11, ZeroPolicy::Reject),
    )
    .expect("single native replacement limits");
    BufferPlanDescriptor::new(schema(base + 6), owner, &carriers, &bounds)
}

pub fn copy_bytes(source: &[u8]) -> Result<Vec<u8>, ResourceStateError> {
    let size = u64::try_from(source.len())
        .map_err(|_| ResourceStateError::Pressure)?;
    if size > crate::MAX_SOURCE_BYTES {
        return Err(ResourceStateError::Pressure);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(source.len())
        .map_err(|_| ResourceStateError::Pressure)?;
    result.extend_from_slice(source);
    Ok(result)
}
