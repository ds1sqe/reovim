//! Bounded callback profile shared by the concrete Domain/Representation pair.

use reovim_kernel_interface::bound::ZeroPolicy;
use reovim_kernel_interface::identity::InstanceId;
use reovim_kernel_interface::semantic::bounds::SemanticPolicyCallBounds;
use reovim_kernel_interface::semantic::descriptor::{
    SemanticPolicyCallDescriptor, SemanticPolicyConcurrency,
};
use reovim_resource_local::profile::{byte_bound, count_bound};

/// Declares one serialized, bounded callback without outward calls.
///
/// # Panics
/// Only if fixed reference-profile bounds become incoherent.
#[must_use]
pub fn call(owner: u128) -> SemanticPolicyCallDescriptor {
    let bounds = SemanticPolicyCallBounds::new(
        InstanceId::new(owner),
        count_bound(owner, 64, ZeroPolicy::Reject),
        byte_bound(owner, 32, ZeroPolicy::Allow),
        byte_bound(owner, 64, ZeroPolicy::Allow),
        count_bound(owner, 1, ZeroPolicy::Reject),
    )
    .expect("bounded basic semantic callback");
    SemanticPolicyCallDescriptor::new(
        SemanticPolicyConcurrency::Serialized,
        &bounds,
    )
    .expect("one active semantic callback")
}
