use reovim_kernel_interface::bound::{BoundDescriptor, BoundValue, ZeroPolicy};
use reovim_kernel_interface::interaction::bounds::{
    InteractionSessionBounds, RepresentationResynchronizationBounds,
    RepresentationUpdateBounds, RepresentationUpdateFlowBounds,
    RepresentationUpdateShapeBounds,
};
use reovim_kernel_interface::interaction::{descriptor, notification};
use reovim_kernel_runtime::ClientBinding;
use reovim_resource_local::profile;

// One subscribed notification reserves all payload retention; a later accepted
// edit must still publish a control invalidation at the undelivered frontier.
pub fn restrict(binding: &mut ClientBinding) {
    let session = &binding.session;
    let bounds = session.bounds();
    let updates = bounds.updates();
    let bytes = u64::try_from(core::mem::size_of::<
        notification::BufferChangeNotification,
    >())
    .unwrap();
    let retained = BoundDescriptor::new(
        BoundValue::Bytes(profile::bytes(bytes)),
        BoundValue::Bytes(profile::bytes(bytes)),
        ZeroPolicy::Allow,
        updates.charge_owner(),
    )
    .unwrap();
    let shape = RepresentationUpdateShapeBounds::new(
        updates.items(),
        updates.depth(),
        updates.bytes(),
    );
    let resync = RepresentationResynchronizationBounds::new(
        updates.resynchronizations(),
        updates.resynchronization_work(),
        updates.resynchronization_bytes(),
    );
    let flow = RepresentationUpdateFlowBounds::new(
        updates.retained_updates(),
        retained,
        updates.credits(),
        updates.control(),
        resync,
    );
    let updates =
        RepresentationUpdateBounds::new(updates.charge_owner(), shape, flow)
            .unwrap();
    let bounds = InteractionSessionBounds::new(
        bounds.charge_owner(),
        bounds.requests().clone(),
        updates,
        bounds.close_work(),
    )
    .unwrap();
    binding.session = descriptor::InteractionSessionDescriptor::new(
        session.stamp(),
        session.scope(),
        session.capabilities(),
        bounds,
        session.update_contract().clone(),
        session.operations(),
    )
    .unwrap();
}
