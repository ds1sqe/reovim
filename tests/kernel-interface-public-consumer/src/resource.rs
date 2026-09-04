use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::operation::ContractSelection;
use reovim_kernel_interface::resource::{
    ResourceAccessStamp, ResourceIdentity, ResourceRevision,
    ResourceSnapshotIdentity, ResourceSnapshotStamp, ResourceValueSchemas,
};

fn selection(identity: u128) -> Option<ContractSelection> {
    ContractSelection::new(identity, 1, 1, 1).ok()
}

fn snapshot(generation: u64, revision: u128) -> Option<ResourceSnapshotStamp> {
    let resource = ResourceIdentity::new(11);
    let lifecycle = LifecycleIdentity::new(12, 13, Generation::new(generation));
    let access = ResourceAccessStamp::new(resource, lifecycle);
    let revision = ResourceRevision::new(selection(104)?, revision);
    Some(ResourceSnapshotStamp::new(
        access,
        revision,
        ResourceSnapshotIdentity::new(15),
    ))
}

pub const fn const_cases() -> bool {
    ResourceIdentity::new(11).value() == 11
        && ResourceSnapshotIdentity::new(15).value() == 15
}

pub fn exercise() -> bool {
    let Some(address) = selection(101) else { return false };
    let Some(extent) = selection(102) else { return false };
    let Some(unit) = selection(103) else { return false };
    let Some(revision) = selection(104) else { return false };
    let schemas = ResourceValueSchemas::new(address, extent, unit, revision);
    let Some(exact) = snapshot(1, 14) else { return false };
    let same_revision = ResourceSnapshotStamp::new(
        exact.access(),
        exact.revision(),
        ResourceSnapshotIdentity::new(16),
    );
    let Some(replaced) = snapshot(2, 14) else { return false };
    schemas.address().identity() == 101
        && schemas.extent().identity() == 102
        && schemas.unit().identity() == 103
        && schemas.revision().identity() == 104
        && exact.access().resource().value() == 11
        && exact.access().lifecycle().instance() == 12
        && exact.revision().schema().identity() == 104
        && exact.revision().token() == 14
        && exact.snapshot().value() == 15
        && exact.same_snapshot(exact)
        && exact.same_revision(same_revision)
        && !exact.same_snapshot(same_revision)
        && !exact.same_revision(replaced)
        && exact.access().same_incarnation(replaced.access())
        && !exact.access().same_generation(replaced.access())
        && crate::derive_witness::<
            ::reovim_kernel_interface::resource::ResourceIdentity,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::resource::ResourceValueSchemas,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::resource::ResourceAccessStamp,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::resource::ResourceRevision,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::resource::ResourceSnapshotIdentity,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::resource::ResourceSnapshotStamp,
        >()
        && crate::traits(exact.access().resource())
        && crate::traits(exact.snapshot())
}
