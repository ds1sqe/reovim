use alloc::vec::Vec;

use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, FenceCause, InstallationOutcome,
};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::resource::state::ResourceWorkingState;
use reovim_kernel_interface::semantic::identity::{
    ProjectionGeneration, ProjectionIdentity,
};
use reovim_kernel_interface::semantic::policy::SemanticPolicyFailure;
use reovim_kernel_interface::semantic::snapshot::{
    RepresentationSnapshotBound as OutputBound,
    RepresentationSnapshotDescriptor as Descriptor,
    RepresentationSnapshotError as Error,
    RepresentationSnapshotPayload as Payload,
    RepresentationSnapshotPort as Port,
    RepresentationSnapshotRequest as Request, RepresentationSnapshotSchema,
};
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

use super::native::{models::ByteModel, support};
use super::snapshots::{SnapshotPolicy, descriptor};
use crate::semantic::support::{fixture_for_model, representation_stamp};

pub fn exercise() {
    foreign_state_and_projection_reject();
    output_failures_retain_payload();
    late_fences_preserve_snapshot();
    semantic_nonresults_are_explicit();
}

fn foreign_state_and_projection_reject() {
    let model = ByteModel::new();
    let fixture = fixture_for_model(&model.binding).expect("native fixture");
    let descriptor = descriptor(&fixture);
    let data = b"# A\n\xff".to_vec();
    let stamp = support::state_stamp(4, 101);
    let mut observed = support::view(&model.binding, &stamp, &data);
    observed.model.contract = support::selection(999);
    let charge = descriptor.call().bounds().prepare(8, 16, 16, 1).unwrap();
    let result = Request::new(
        &descriptor,
        &observed,
        projection(&descriptor),
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    );
    assert_eq!(result.unwrap_err(), Error::ModelMismatch);

    observed.model = model.binding;
    let original = projection(&descriptor);
    let replaced = ProjectionStamp::new(
        original.projection(),
        original.lifecycle(),
        representation_stamp(fixture.domain.stamp(), 2)
            .expect("replacement Representation"),
        original.predecessor(),
    );
    let result = Request::new(
        &descriptor,
        &observed,
        replaced,
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    );
    assert_eq!(result.unwrap_err(), Error::ProjectionMismatch);
    assert_eq!(data.as_slice(), b"# A\n\xff");
}

fn output_failures_retain_payload() {
    let model = ByteModel::new();
    let fixture = fixture_for_model(&model.binding).expect("native fixture");
    let provider =
        SnapshotPolicy { descriptor: descriptor(&fixture), model: &model };
    let data = b"# A\n\xff".to_vec();
    let stamp = support::state_stamp(4, 101);
    let observed = support::view(&model.binding, &stamp, &data);
    let bounded = request(&provider.descriptor, &observed, 1);
    let rejected = bounded.produce_with(&provider).unwrap_err();
    assert!(matches!(
        rejected.cause,
        Error::OutputBound { field: OutputBound::Bytes, maximum: 1, .. }
    ));
    assert_eq!(rejected.payload.unwrap().bytes.as_ref(), b"H\x01A");

    let descriptor = provider.descriptor.clone();
    let request = request(&descriptor, &observed, 16);
    let invalid = Faulty { inner: provider, failure: None };
    let rejected = request.produce_with(&invalid).unwrap_err();
    assert_eq!(rejected.cause, Error::OutputSchemaMismatch);
    assert_eq!(rejected.payload.unwrap().bytes.as_ref(), b"H\x01A");
    assert_eq!(data.as_slice(), b"# A\n\xff");
}

fn late_fences_preserve_snapshot() {
    let model = ByteModel::new();
    let fixture = fixture_for_model(&model.binding).expect("native fixture");
    let provider =
        SnapshotPolicy { descriptor: descriptor(&fixture), model: &model };
    let data = b"# A\n\xff".to_vec();
    let stamp = support::state_stamp(4, 101);
    let observed = support::view(&model.binding, &stamp, &data);
    let request = request(&provider.descriptor, &observed, 16);
    let snapshot = request.produce_with(&provider).unwrap();
    assert_eq!(
        snapshot.check_admission(
            &provider.descriptor,
            &stamp,
            &request.projection(),
            InstallationOutcome::Fenced(FenceCause::Replacement)
        ),
        Err(Error::Fenced(FenceCause::Replacement))
    );
    assert_eq!(
        snapshot.check_admission(
            &provider.descriptor,
            &support::state_stamp(5, 101),
            &request.projection(),
            InstallationOutcome::Current
        ),
        Err(Error::StateMismatch)
    );
    assert_eq!(snapshot.payload().bytes.as_ref(), b"H\x01A");
    assert_eq!(snapshot.target(), stamp);
    assert_eq!(data.as_slice(), b"# A\n\xff");
}

fn semantic_nonresults_are_explicit() {
    let model = ByteModel::new();
    let fixture = fixture_for_model(&model.binding).expect("native fixture");
    let provider =
        SnapshotPolicy { descriptor: descriptor(&fixture), model: &model };
    let stamp = support::state_stamp(4, 101);
    let data = b"# 0\n".to_vec();
    let observed = support::view(&model.binding, &stamp, &data);
    let invalid = request(&provider.descriptor, &observed, 16);
    let rejected = invalid.produce_with(&provider).unwrap_err();
    assert_eq!(
        rejected.cause,
        Error::Policy(SemanticPolicyFailure::Unrepresentable)
    );
    assert!(rejected.payload.is_none());
    let descriptor = provider.descriptor.clone();
    let unavailable = Faulty {
        inner: provider,
        failure: Some(SemanticPolicyFailure::StateUnavailable),
    };
    let request = request(&descriptor, &observed, 16);
    let rejected = request.produce_with(&unavailable).unwrap_err();
    assert_eq!(
        rejected.cause,
        Error::Policy(SemanticPolicyFailure::StateUnavailable)
    );
    assert!(rejected.payload.is_none());
    assert_eq!(data.as_slice(), b"# 0\n");
}

struct Faulty<'a> {
    inner: SnapshotPolicy<'a, ByteModel>,
    failure: Option<SemanticPolicyFailure>,
}

impl Port for Faulty<'_> {
    type State = Vec<u8>;

    fn descriptor(&self) -> &Descriptor {
        &self.inner.descriptor
    }

    fn snapshot(
        &self,
        request: &Request<'_, Self::State>,
    ) -> Result<Payload, SemanticPolicyFailure> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        let mut payload = self.inner.snapshot(request)?;
        payload.schema = RepresentationSnapshotSchema::new(999, 1);
        Ok(payload)
    }
}

fn request<'a>(
    descriptor: &'a Descriptor,
    observed: &'a ResourceWorkingState<'a, Vec<u8>>,
    bytes: u64,
) -> Request<'a, Vec<u8>> {
    let charge = descriptor.call().bounds().prepare(8, 16, bytes, 1).unwrap();
    Request::new(
        descriptor,
        observed,
        projection(descriptor),
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .unwrap()
}

const fn projection(descriptor: &Descriptor) -> ProjectionStamp {
    ProjectionStamp::new(
        ProjectionIdentity::new(1801),
        LifecycleIdentity::new(1802, 1803, Generation::new(1)),
        descriptor.representation(),
        ProjectionGeneration::new(3),
    )
}
