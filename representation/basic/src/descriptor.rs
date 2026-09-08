use reovim_kernel_interface::bound::ZeroPolicy;
use reovim_kernel_interface::buffer::plan::BufferPlanDescriptor;
use reovim_kernel_interface::identity::{IncarnationId, InstanceId};
use reovim_kernel_interface::semantic::bounds::SemanticIntentBounds;
use reovim_kernel_interface::semantic::descriptor::{
    DomainPolicyDescriptor, RepresentationBehavior,
    RepresentationPolicyDescriptor, SemanticBufferPlanContract,
    SemanticCoordinateSpaceDescriptor, SemanticCorrespondence,
    SemanticEditability, SemanticFidelity,
};
use reovim_kernel_interface::semantic::identity::{
    CoordinateSpaceIdentity, RepresentationIdentity,
};
use reovim_kernel_interface::semantic::intent::SemanticIntentContract;
use reovim_kernel_interface::semantic::stamp::{
    CoordinateSpaceStamp, RepresentationPolicyStamp,
};
use reovim_resource_local::profile::{byte_bound, count_bound, safety, schema};

pub fn build(
    base: u128,
    domain: &DomainPolicyDescriptor,
    plan: &BufferPlanDescriptor,
) -> RepresentationPolicyDescriptor {
    let owner = domain.stamp().lifecycle();
    let bounds = SemanticIntentBounds::new(
        InstanceId::new(owner.instance()),
        count_bound(owner.instance(), 1, ZeroPolicy::Reject),
        count_bound(owner.instance(), 0, ZeroPolicy::Allow),
        byte_bound(owner.instance(), 1, ZeroPolicy::Reject),
    )
    .expect("one semantic instruction");
    let coordinate = SemanticCoordinateSpaceDescriptor::new(
        CoordinateSpaceStamp::new(
            CoordinateSpaceIdentity::new(base + 2),
            schema(base + 3),
            IncarnationId::new(owner.incarnation()),
        ),
        schema(base + 4),
        schema(base + 5),
        schema(base + 6),
        &bounds,
    );
    let intent =
        SemanticIntentContract::new(schema(base + 7), safety(base + 7))
            .expect("owned basic semantic instruction");
    let behavior = RepresentationBehavior::new(
        SemanticFidelity::ConstrainedEditable,
        SemanticEditability::Transactional,
        SemanticCorrespondence::Bidirectional,
    )
    .expect("constrained reversible reference view");
    RepresentationPolicyDescriptor::new(
        domain,
        RepresentationPolicyStamp::new(
            RepresentationIdentity::new(base),
            schema(base + 1),
            owner,
            domain.stamp(),
        ),
        &coordinate,
        &intent,
        behavior,
        SemanticBufferPlanContract::from_descriptor(plan),
        &reovim_domain_basic::profile::call(owner.instance()),
    )
    .expect("selected native model and lowering profile")
}
