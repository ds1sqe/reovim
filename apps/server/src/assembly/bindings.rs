use reovim_kernel_interface::bound::{BoundDescriptor, BoundValue, ZeroPolicy};
use reovim_kernel_interface::identity::InstanceId;
use reovim_kernel_interface::interaction::bounds::{
    InteractionRequestBounds, InteractionSessionBounds,
    RepresentationResynchronizationBounds, RepresentationUpdateBounds,
    RepresentationUpdateFlowBounds, RepresentationUpdateShapeBounds,
};
use reovim_kernel_interface::interaction::descriptor::{
    InteractionCapabilitySupport, InteractionSessionCapabilities,
    InteractionSessionDescriptor, InteractionSessionOperations,
    RepresentationAttachmentDescriptor, RepresentationAttachmentSelection,
};
use reovim_kernel_interface::interaction::identity::{
    ClientIdentity, InteractionSessionIdentity,
    RepresentationAttachmentIdentity, RepresentationUpdateStreamIdentity,
};
use reovim_kernel_interface::interaction::session::{
    InteractionClientStamp, InteractionResourceScope, InteractionSessionStamp,
    RepresentationAttachmentHandle, RepresentationUpdateStreamStamp,
};
use reovim_kernel_interface::interaction::update::RepresentationUpdateContract;
use reovim_kernel_interface::semantic::descriptor;
use reovim_kernel_interface::semantic::identity::{
    ProjectionGeneration, ProjectionIdentity,
};
use reovim_kernel_interface::semantic::snapshot;
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;
use reovim_kernel_runtime::ClientBinding;
use reovim_resource_local::profile;

use reovim_kernel_interface::lifecycle::LifecycleIdentity;

/// The executable reference reserves bounded repeated requests per endpoint.
pub const REQUESTS: u64 = 128;
const REQUEST_BYTES: u64 = 64;
const OUTCOME_BYTES: u64 = 4096;
const UPDATE_BYTES: u64 = 4096;
const UPDATES: u64 = 16;

/// Selects the reference client's separately bounded public endpoint.
///
/// # Errors
/// Rejects an exhausted identity namespace or incompatible semantic selection.
pub fn client_binding(
    client_lifecycle: LifecycleIdentity,
    scope: InteractionResourceScope,
    representation: &descriptor::RepresentationPolicyDescriptor,
    snapshot: &snapshot::RepresentationSnapshotDescriptor,
) -> Result<ClientBinding, super::AssemblyError> {
    let client_id = client_lifecycle.instance();
    client_id.checked_add(10).ok_or(super::AssemblyError::Identity)?;
    let lifecycle = offset(client_lifecycle, 1);
    let client = InteractionClientStamp::new(
        ClientIdentity::new(client_id),
        lifecycle,
        profile::schema(0x2010),
    );
    let stamp = InteractionSessionStamp::new(
        InteractionSessionIdentity::new(client_id + 2),
        profile::schema(0x2011),
        offset(client_lifecycle, 3),
        client,
    );
    let supported = InteractionCapabilitySupport::Supported;
    let unsupported = InteractionCapabilitySupport::Unsupported;
    let capabilities = InteractionSessionCapabilities::new(
        supported,
        supported,
        supported,
        unsupported,
        unsupported,
    )
    .map_err(|_| super::AssemblyError::Policy)?;
    let update_contract = RepresentationUpdateContract::new(
        snapshot.output(),
        profile::safety(snapshot.output().identity()),
    )
    .map_err(|_| super::AssemblyError::Policy)?;
    let session = InteractionSessionDescriptor::new(
        stamp,
        scope,
        capabilities,
        bounds(stamp.lifecycle().instance()),
        update_contract,
        InteractionSessionOperations::required(),
    )
    .map_err(|_| super::AssemblyError::Policy)?;
    let handle = RepresentationAttachmentHandle::new(
        RepresentationAttachmentIdentity::new(client_id + 4),
        offset(client_lifecycle, 5),
        stamp.handle(),
    );
    let stream = RepresentationUpdateStreamStamp::new(
        RepresentationUpdateStreamIdentity::new(client_id + 6),
        offset(client_lifecycle, 7),
        handle,
    );
    let projection = ProjectionStamp::new(
        ProjectionIdentity::new(client_id + 8),
        offset(client_lifecycle, 9),
        representation.stamp(),
        ProjectionGeneration::new(0),
    );
    let selection = RepresentationAttachmentSelection::new(
        handle,
        scope,
        representation.stamp(),
        projection,
        representation.coordinate().stamp(),
        stream,
        4,
    );
    let attachment = RepresentationAttachmentDescriptor::new(
        &session,
        representation,
        selection,
    )
    .map_err(|_| super::AssemblyError::Policy)?;
    Ok(ClientBinding {
        authorization: offset(client_lifecycle, 10),
        session,
        attachment,
    })
}

const fn offset(base: LifecycleIdentity, offset: u128) -> LifecycleIdentity {
    LifecycleIdentity::new(
        base.instance() + offset,
        base.incarnation(),
        base.generation(),
    )
}

fn bounds(owner: u128) -> InteractionSessionBounds {
    let requests = InteractionRequestBounds::new(
        InstanceId::new(owner),
        count(owner, 1, REQUESTS, ZeroPolicy::Reject),
        bytes(
            owner,
            REQUEST_BYTES,
            REQUEST_BYTES * REQUESTS,
            ZeroPolicy::Reject,
        ),
        count(owner, 1, REQUESTS, ZeroPolicy::Reject),
        bytes(
            owner,
            OUTCOME_BYTES,
            OUTCOME_BYTES * REQUESTS,
            ZeroPolicy::Reject,
        ),
    )
    .expect("request and terminal reservations");
    let shape = RepresentationUpdateShapeBounds::new(
        count(owner, 64, 64, ZeroPolicy::Reject),
        count(owner, 8, 8, ZeroPolicy::Allow),
        bytes(owner, UPDATE_BYTES, UPDATE_BYTES * UPDATES, ZeroPolicy::Reject),
    );
    let resynchronization = RepresentationResynchronizationBounds::new(
        count(owner, 1, 1, ZeroPolicy::Reject),
        count(owner, 64, 64, ZeroPolicy::Reject),
        bytes(owner, UPDATE_BYTES, UPDATE_BYTES, ZeroPolicy::Reject),
    );
    let flow = RepresentationUpdateFlowBounds::new(
        count(owner, 1, UPDATES, ZeroPolicy::Reject),
        bytes(owner, UPDATE_BYTES, UPDATE_BYTES * UPDATES, ZeroPolicy::Allow),
        count(owner, 4, 4, ZeroPolicy::Reject),
        count(owner, 1, 1, ZeroPolicy::Reject),
        resynchronization,
    );
    let updates =
        RepresentationUpdateBounds::new(InstanceId::new(owner), shape, flow)
            .expect("independent update reservations");
    InteractionSessionBounds::new(
        InstanceId::new(owner),
        requests,
        updates,
        count(owner, 64, 64, ZeroPolicy::Reject),
    )
    .expect("bounded session")
}

fn count(
    owner: u128,
    per: u64,
    total: u64,
    zero: ZeroPolicy,
) -> BoundDescriptor {
    BoundDescriptor::new(
        BoundValue::Count(profile::count(per)),
        BoundValue::Count(profile::count(total)),
        zero,
        InstanceId::new(owner),
    )
    .expect("selected count reservation")
}

fn bytes(
    owner: u128,
    per: u64,
    total: u64,
    zero: ZeroPolicy,
) -> BoundDescriptor {
    BoundDescriptor::new(
        BoundValue::Bytes(profile::bytes(per)),
        BoundValue::Bytes(profile::bytes(total)),
        zero,
        InstanceId::new(owner),
    )
    .expect("selected byte reservation")
}
