use reovim_domain_basic::{HeadingDomain, KeyedDomain};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::resource::state::ResourceStateError;
use reovim_representation_basic::{HeadingRepresentation, KeyedRepresentation};
use reovim_resource_local::{ByteModel, KeyedModel};

#[test]
fn representation_selection_rejects_wrong_native_generation_without_panicking()
{
    let original = ByteModel::new(life(1), life(2));
    let replacement = ByteModel::new(life(3), life(4));
    let domain = HeadingDomain::new(&original);
    assert!(matches!(
        HeadingRepresentation::new(&domain, &replacement),
        Err(ResourceStateError::ModelMismatch),
    ));
    assert!(HeadingRepresentation::new(&domain, &original).is_ok());

    let original = KeyedModel::new(life(1), life(2));
    let replacement = KeyedModel::new(life(3), life(4));
    let domain = KeyedDomain::new(&original);
    assert!(matches!(
        KeyedRepresentation::new(&domain, &replacement),
        Err(ResourceStateError::ModelMismatch),
    ));
    assert!(KeyedRepresentation::new(&domain, &original).is_ok());
}

const fn life(id: u128) -> LifecycleIdentity {
    LifecycleIdentity::new(id, id, Generation::new(1))
}
