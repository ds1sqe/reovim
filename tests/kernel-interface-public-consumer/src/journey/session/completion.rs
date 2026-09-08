use alloc::rc::Rc;

use reovim_kernel_interface::buffer::{
    BufferHistoryNodeIdentity, BufferHistoryStamp, BufferStateStamp,
    BufferVersion,
};
use reovim_kernel_interface::cancellation::InstallationOutcome;
use reovim_kernel_interface::interaction::identity::RepresentationUpdateCursor;
use reovim_kernel_interface::interaction::outcome::{
    InteractionOutcome, InteractionOutcomeCause,
};
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateBindingOutcome,
    RepresentationUpdateLineage, RepresentationUpdatePayloadClaim,
};
use reovim_kernel_interface::interaction::{
    intent::InteractionIntent, notification::BufferChangeNotification,
};
use reovim_kernel_interface::resource::state::{
    ResourceStateInput, ResourceStateTarget,
};
use reovim_kernel_interface::semantic::identity::ProjectionGeneration;
use reovim_kernel_interface::semantic::policy::SemanticPolicyFailure;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotSchema;

use super::Session;
use crate::first_edit::{self, Family, native::support as native};
use crate::interaction::support;

impl<Model: Family> Session<Model>
where
    Model::State: Clone,
{
    /// Cases supply the accepted stamp; this is not a general History engine.
    pub fn complete_edit(
        &mut self,
        result: &BufferStateStamp,
    ) -> Result<BufferChangeNotification, InteractionOutcomeCause> {
        let intent = self.pending.take().expect("admitted fixture edit");
        let accepted = InteractionOutcome::accepted(
            intent.stamp(),
            intent.semantic().stamp(),
            *result,
        );
        let prepared = accepted
            .map_err(|_| InteractionOutcomeCause::ProviderContractViolation)
            .and_then(|_| self.evaluate_edit(&intent, result));
        let outcome = match prepared {
            Ok(()) => accepted.expect("validated before native installation"),
            Err(cause) => {
                InteractionOutcome::not_accepted(intent.stamp(), cause)
            }
        };
        self.outcome = Some(outcome);
        let notification = BufferChangeNotification::from_edit(
            outcome, &intent,
        )
        .map_err(|_| {
            outcome.cause().unwrap_or(InteractionOutcomeCause::InvariantFailure)
        });
        self.completed = Some((intent, outcome));
        self.cancelled = false;
        notification
    }

    fn evaluate_edit(
        &self,
        intent: &InteractionIntent,
        result: &BufferStateStamp,
    ) -> Result<(), InteractionOutcomeCause> {
        if self.cancelled {
            return Err(InteractionOutcomeCause::Cancelled);
        }
        let (model, observed, stamp) = {
            let shared = self.shared.borrow();
            (
                Rc::clone(&shared.model),
                shared.current.clone().expect("opened state"),
                shared.stamp,
            )
        };
        if stamp != *intent.semantic().stamp().target() {
            return Err(InteractionOutcomeCause::StaleState);
        }
        let view = native::view(&model.binding(), &stamp, &observed);
        let plan = first_edit::semantic_plan(
            model.as_ref(),
            &self.fixture.semantic,
            &view,
            intent.semantic(),
        )
        .map_err(policy_cause)?;
        let request = native::request(
            &model.binding(),
            ResourceStateInput::Edit {
                observed: &view,
                descriptor: &self.fixture.semantic.buffer,
                plan: &plan,
                observations: &[],
            },
        );
        let candidate = request
            .prepare_with(model.as_ref())
            .map_err(|_| InteractionOutcomeCause::Conflict)?;
        let retained = candidate.state().clone();
        let binding = model.binding();
        let retired = {
            let mut shared = self.shared.borrow_mut();
            candidate
                .check_admission(
                    binding,
                    request.stamp(),
                    ResourceStateTarget::Buffer(shared.stamp),
                    request.limits(),
                    InstallationOutcome::Current,
                )
                .map_err(|_| InteractionOutcomeCause::StaleState)?;
            let old = shared.current.replace(candidate.into_output().state);
            let history_old = shared.edited.replace(retained);
            shared.stamp = *result;
            (old, history_old)
        };
        drop(retired);
        Ok(())
    }

    pub fn make_update(
        &self,
        predecessor: u64,
        result: u64,
        cursor: u64,
    ) -> RepresentationUpdate {
        let (model, data, stamp) = {
            let shared = self.shared.borrow();
            (
                Rc::clone(&shared.model),
                shared.current.clone().expect("opened state"),
                shared.stamp,
            )
        };
        let view = native::view(&model.binding(), &stamp, &data);
        let projection = support::projection(&self.fixture, predecessor);
        let snapshot = first_edit::produce_snapshot(
            model.as_ref(),
            &self.fixture.semantic,
            &view,
            &projection,
        );
        let payload = snapshot.into_payload();
        let selected = self.fixture.session.update_contract().selected_stamp();
        assert_eq!(
            payload.schema,
            RepresentationSnapshotSchema::new(
                selected.type_identity(),
                selected.schema_version()
            )
        );
        let stamp = support::update_stamp(
            &self.fixture,
            9000 + u128::from(cursor),
            RepresentationUpdateCursor::new(cursor),
            &stamp,
            &projection,
        )
        .unwrap();
        let lineage = RepresentationUpdateLineage::FullSnapshot {
            supersedes: ProjectionGeneration::new(predecessor),
            result: ProjectionGeneration::new(result),
        };
        let claim = RepresentationUpdatePayloadClaim::new(
            selected,
            payload.items.value(),
            payload.depth.value(),
            u64::try_from(payload.bytes.len()).unwrap(),
        );
        let prepared = self
            .fixture
            .session
            .prepare_update(&self.fixture.attachment, stamp, lineage, claim)
            .unwrap();
        let RepresentationUpdateBindingOutcome::Bound(carrier) =
            prepared.bind(payload.bytes)
        else {
            panic!("actual snapshot binding")
        };
        carrier.into_update()
    }
}

pub const fn next_stamp(
    old: &BufferStateStamp,
    version: u64,
    node: u128,
) -> BufferStateStamp {
    BufferStateStamp::new(
        old.source(),
        old.buffer(),
        BufferVersion::new(version),
        BufferHistoryStamp::new(
            old.history().lifecycle(),
            BufferHistoryNodeIdentity::new(node),
        ),
    )
}

const fn policy_cause(cause: SemanticPolicyFailure) -> InteractionOutcomeCause {
    match cause {
        SemanticPolicyFailure::NoCorrespondence => {
            InteractionOutcomeCause::NoCorrespondence
        }
        SemanticPolicyFailure::InvalidIntent
        | SemanticPolicyFailure::Unrepresentable => {
            InteractionOutcomeCause::InvalidIntent
        }
        _ => InteractionOutcomeCause::PolicyFailure,
    }
}
