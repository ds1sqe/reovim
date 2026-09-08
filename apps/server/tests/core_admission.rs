use reovim_kernel_core::{Buffer, BufferError};
use reovim_kernel_interface::cancellation::{FenceCause, InstallationOutcome};
use reovim_kernel_interface::resource::state::{
    ResourceStateError, ResourceStateInput,
};
use reovim_resource_local::ByteModel;

#[path = "support/core.rs"]
mod support;
use support::{config, life, observation, opened, preparation, prepared};

#[test]
fn repeated_acceptance_owns_versions_and_retains_immutable_native_leases() {
    let model = ByteModel::new(life(1), life(2));
    let (_source, mut buffer) = opened(&model, b"# a\n\xff", 8, 4096);
    let initial = buffer.observe();
    let cloned_handle = initial.clone(); // ByteState deliberately is not Clone.
    for (request, byte) in [(10, b'b'), (11, b'c'), (12, b'd')] {
        let prior = buffer.stamp();
        let (expected, candidate) = prepared(&model, &buffer, request, byte);
        let staged = buffer.stage_edit(&expected, candidate).unwrap();
        let commit = buffer
            .commit_edit(staged, InstallationOutcome::Current)
            .unwrap()
            .commit;
        assert_eq!(commit.prior, prior);
        assert_eq!(
            commit.result.version().value(),
            prior.version().value() + 1
        );
        assert_eq!(commit.edge.parent(), prior.history().node());
        assert_eq!(commit.edge.child(), commit.result.history().node());
        assert_ne!(commit.edge.parent(), commit.edge.child());
        assert_eq!(
            buffer.observe().view().state.as_bytes(),
            [b'#', b' ', byte, b'\n', 0xff]
        );
    }
    assert_eq!(initial.view().state.as_bytes(), b"# a\n\xff");
    assert_eq!(cloned_handle.view().state.as_bytes(), b"# a\n\xff");
    assert_eq!(initial.stamp(), cloned_handle.stamp());
}

#[test]
fn stale_and_fenced_candidates_never_change_accepted_state() {
    let model = ByteModel::new(life(1), life(2));
    let (_source, mut buffer) = opened(&model, b"# a\n\xff", 8, 4096);
    let (first, a) = prepared(&model, &buffer, 10, b'b');
    let (second, b) = prepared(&model, &buffer, 11, b'c');
    let a = buffer.stage_edit(&first, a).unwrap();
    let b = buffer.stage_edit(&second, b).unwrap();
    buffer.commit_edit(a, InstallationOutcome::Current).unwrap();
    let accepted = buffer.stamp();
    let rejected =
        buffer.commit_edit(b, InstallationOutcome::Current).unwrap_err();
    assert_eq!(
        rejected.cause,
        BufferError::State(ResourceStateError::StateMismatch)
    );
    drop(rejected); // Returned ownership leaves the authority call first.
    let (expected, candidate) = prepared(&model, &buffer, 12, b'd');
    let staged = buffer.stage_edit(&expected, candidate).unwrap();
    let fenced = buffer
        .commit_edit(
            staged,
            InstallationOutcome::Fenced(FenceCause::Cancellation),
        )
        .unwrap_err();
    assert_eq!(
        fenced.cause,
        BufferError::State(ResourceStateError::Fenced(
            FenceCause::Cancellation
        ))
    );
    drop(fenced);
    assert_eq!(buffer.stamp(), accepted);
    assert_eq!(buffer.observe().view().state.as_bytes(), b"# b\n\xff");
}

#[test]
fn node_and_native_byte_pressure_return_the_unchanged_candidate() {
    let model = ByteModel::new(life(1), life(2));
    let (_initial_source, initial) = observation(&model, b"# a\n\xff");
    let request = preparation(&model, ResourceStateInput::Initial(&initial));
    let native_charge =
        request.prepare_with(&model).unwrap().usage().resident_bytes;
    for (nodes, resident) in [(1, 4096), (8, native_charge.value())] {
        let (_source, buffer) = opened(&model, b"# a\n\xff", nodes, resident);
        let initial_stamp = buffer.stamp();
        let (expected, candidate) = prepared(&model, &buffer, 10, b'b');
        let rejected = buffer.stage_edit(&expected, candidate).unwrap_err();
        assert_eq!(rejected.cause, BufferError::Capacity);
        assert_eq!(rejected.value.state().as_bytes(), b"# b\n\xff");
        assert_eq!(buffer.stamp(), initial_stamp);
        assert_eq!(buffer.observe().view().state.as_bytes(), b"# a\n\xff");
    }
}

#[test]
fn changed_preparation_and_initial_fences_retain_native_ownership() {
    let model = ByteModel::new(life(1), life(2));
    let (_source, buffer) = opened(&model, b"# a\n\xff", 8, 4096);
    let (expected, candidate) = prepared(&model, &buffer, 10, b'b');
    let (different, _) = prepared(&model, &buffer, 11, b'c');
    assert_ne!(expected, different);
    let rejected = buffer.stage_edit(&different, candidate).unwrap_err();
    assert_eq!(
        rejected.cause,
        BufferError::State(ResourceStateError::PreparationMismatch)
    );
    assert_eq!(rejected.value.state().as_bytes(), b"# b\n\xff");
    let (_other_source, observed) = observation(&model, b"# a\n\xff");
    let request = preparation(&model, ResourceStateInput::Initial(&observed));
    let candidate = request.prepare_with(&model).unwrap();
    let config = config(&model, &observed, 8, 4096);
    let rejected = Buffer::new(
        &config,
        &request.stamp(),
        candidate,
        InstallationOutcome::Fenced(FenceCause::Closing),
    )
    .unwrap_err();
    assert_eq!(
        rejected.cause,
        BufferError::State(ResourceStateError::Fenced(FenceCause::Closing))
    );
    assert_eq!(rejected.value.state().as_bytes(), b"# a\n\xff");
}
