use std::collections::BTreeMap;

use reovim_kernel_interface::interaction::update::RepresentationUpdateLineage;
use reovim_kernel_interface::semantic::identity::ProjectionGeneration;
use reovim_kernel_interface::semantic::policy::SemanticPolicyFailure;
use reovim_kernel_interface_public_consumer::first_edit;
use reovim_kernel_interface_public_consumer::first_edit::native::models::Key;
use reovim_kernel_interface_public_consumer::first_edit::{heading, keyed};

#[test]
fn heading_data_controls_validation_lowering_and_derived_snapshot() {
    let result = heading(b"# a\n\xff", b'b').expect("editable heading");
    assert_eq!(result.initial, b"# a\n\xff");
    assert_eq!(result.prepared, b"# b\n\xff");
    assert_eq!(result.operations.as_ref(), [2, b'b']);
    assert_eq!(result.preconditions.as_ref(), b"a");
    assert_eq!(result.initial_snapshot.as_ref(), [b'H', 1, b'a']);
    assert_eq!(result.prepared_snapshot.as_ref(), [b'H', 1, b'b']);
    assert_eq!(
        result.prepared_update.carrier().expect("full update").bytes(),
        result.prepared_snapshot.as_ref()
    );
    assert_eq!(result.restored, result.initial);
    assert_eq!(result.restored_snapshot, result.initial_snapshot);
    assert_eq!(
        result.prepared_update.lineage(),
        RepresentationUpdateLineage::FullSnapshot {
            supersedes: ProjectionGeneration::new(3),
            result: ProjectionGeneration::new(4),
        }
    );

    let alternate = heading(b"## c\n\xff", b'd').expect("second-level heading");
    assert_eq!(alternate.operations.as_ref(), [3, b'd']);
    assert_eq!(alternate.preconditions.as_ref(), b"c");
    assert_eq!(alternate.prepared_snapshot.as_ref(), [b'H', 2, b'd']);
    assert_eq!(alternate.prepared, b"## d\n\xff");
}

#[test]
fn actual_content_can_reject_an_otherwise_valid_heading_intent() {
    assert_eq!(
        heading(b"plain\xff", b'b').err(),
        Some(SemanticPolicyFailure::NoCorrespondence)
    );
    assert_eq!(
        heading(b"# \xff\n", b'b').err(),
        Some(SemanticPolicyFailure::Unrepresentable)
    );
    assert_eq!(
        heading(b"# a\n", 0xff).err(),
        Some(SemanticPolicyFailure::Unrepresentable)
    );
}

#[test]
fn absent_native_key_rejects_semantic_edit() {
    assert_eq!(
        keyed(&[1], 17).err(),
        Some(SemanticPolicyFailure::NoCorrespondence)
    );
}

#[test]
fn validation_receipt_requires_the_complete_unchanged_semantic_request() {
    first_edit::exercise_receipt_fences();
}

#[test]
fn policy_input_state_bounds_and_late_fences_remain_exact() {
    first_edit::exercise_request_fences();
}

#[test]
fn derived_snapshot_fences_and_output_pressure_preserve_ownership() {
    first_edit::exercise_snapshot_fences();
}

#[test]
fn keyed_toggle_lowers_current_value_and_preserves_unrelated_key() {
    let result = keyed(&[0, 1], 17).expect("existing Alpha");
    assert_eq!(
        result.initial,
        BTreeMap::from([(Key::Alpha, false), (Key::Omega, true)])
    );
    assert_eq!(
        result.prepared,
        BTreeMap::from([(Key::Alpha, true), (Key::Omega, true)])
    );
    assert_eq!(result.operations.as_ref(), [17, 1]);
    assert_eq!(result.preconditions.as_ref(), [0]);
    assert_eq!(result.initial_snapshot.as_ref(), [b'K', 17, 0, 93, 1]);
    assert_eq!(result.prepared_snapshot.as_ref(), [b'K', 17, 1, 93, 1]);
    assert_eq!(
        result.prepared_update.carrier().expect("full update").bytes(),
        result.prepared_snapshot.as_ref()
    );
    assert_eq!(result.restored, result.initial);
    assert_eq!(result.restored_snapshot, result.initial_snapshot);

    let alternate = keyed(&[1, 1], 17).expect("toggle enabled Alpha");
    assert_eq!(alternate.operations.as_ref(), [17, 0]);
    assert_eq!(alternate.preconditions.as_ref(), [1]);
    assert_eq!(
        alternate.prepared,
        BTreeMap::from([(Key::Alpha, false), (Key::Omega, true)])
    );
}
