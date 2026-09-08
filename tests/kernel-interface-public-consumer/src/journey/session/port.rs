use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionCancellationRequest, InteractionCancellationResult,
    InteractionIntentAdmission, InteractionOutcomeDelivery,
    InteractionOutcomeRequest, InteractionReconcileRequest,
    InteractionReconcileResult,
};
use reovim_kernel_interface::interaction::delivery::session::{
    InteractionCloseOutcome, InteractionCloseRequest,
    InteractionNegotiationFailure, InteractionNegotiationOutcome,
    InteractionNegotiationRequest, RepresentationAttachmentFailure,
    RepresentationAttachmentOutcome, RepresentationAttachmentReady,
    RepresentationAttachmentReadyBindingOutcome,
    RepresentationAttachmentRejection, RepresentationAttachmentRequest,
};
use reovim_kernel_interface::interaction::delivery::update::{
    RepresentationResynchronizationAdmission,
    RepresentationResynchronizationFailure,
    RepresentationResynchronizationRequest,
    RepresentationUpdateAcknowledgement,
    RepresentationUpdateAcknowledgementResult, RepresentationUpdateDelivery,
    RepresentationUpdateRequest,
};
use reovim_kernel_interface::interaction::descriptor;
use reovim_kernel_interface::interaction::intent::InteractionIntent;
use reovim_kernel_interface::interaction::port::InteractionSessionPort;

use super::{AttachmentState, Family, Session};

mod admission;
mod delivery;
mod teardown;

impl<Model: Family> InteractionSessionPort for Session<Model>
where
    Model::State: Clone,
{
    fn descriptor(&self) -> &descriptor::InteractionSessionDescriptor {
        &self.fixture.session
    }

    fn negotiate(
        &mut self,
        request: InteractionNegotiationRequest,
    ) -> InteractionNegotiationOutcome {
        if !request.session().same_generation(self.fixture.session.stamp()) {
            return InteractionNegotiationOutcome::rejected(
                InteractionNegotiationFailure::Replaced,
            );
        }
        if self.closed {
            return InteractionNegotiationOutcome::rejected(
                InteractionNegotiationFailure::Closed,
            );
        }
        if !self.ready() {
            return InteractionNegotiationOutcome::rejected(
                InteractionNegotiationFailure::NotReady,
            );
        }
        self.attachment_state = AttachmentState::Negotiated;
        InteractionNegotiationOutcome::negotiated(request.session().handle())
    }

    fn attach(
        &mut self,
        request: RepresentationAttachmentRequest,
    ) -> RepresentationAttachmentOutcome {
        if !self.ready() || !self.is_negotiated() || self.is_attached() {
            return RepresentationAttachmentOutcome::rejected(
                RepresentationAttachmentRejection::new(
                    RepresentationAttachmentFailure::NotReady,
                    request,
                ),
            );
        }
        if request.descriptor() != &self.fixture.attachment {
            return RepresentationAttachmentOutcome::rejected(
                RepresentationAttachmentRejection::new(
                    RepresentationAttachmentFailure::Replaced,
                    request,
                ),
            );
        }
        let initial = self.make_update(3, 3, 0);
        let ready = RepresentationAttachmentReady::bind(
            &self.fixture.session,
            request.into_descriptor(),
            initial,
        );
        let RepresentationAttachmentReadyBindingOutcome::Bound(ready) = ready
        else {
            panic!("actual initial snapshot matches selected endpoint");
        };
        self.attachment_state = AttachmentState::Attached;
        RepresentationAttachmentOutcome::ready(ready)
    }

    fn submit_intent(
        &mut self,
        intent: InteractionIntent,
    ) -> InteractionIntentAdmission {
        admission::submit(self, intent)
    }

    fn request_cancellation(
        &mut self,
        request: InteractionCancellationRequest,
    ) -> InteractionCancellationResult {
        admission::cancel(self, &request)
    }

    fn take_outcome(
        &mut self,
        request: InteractionOutcomeRequest,
    ) -> InteractionOutcomeDelivery {
        delivery::outcome(self, &request)
    }

    fn reconcile_outcome(
        &mut self,
        request: InteractionReconcileRequest,
    ) -> InteractionReconcileResult {
        delivery::reconcile(self, &request)
    }

    fn take_update(
        &mut self,
        request: RepresentationUpdateRequest,
    ) -> RepresentationUpdateDelivery {
        delivery::update(self, request)
    }

    fn acknowledge_update(
        &mut self,
        acknowledgement: RepresentationUpdateAcknowledgement,
    ) -> RepresentationUpdateAcknowledgementResult {
        delivery::acknowledge(self, acknowledgement)
    }

    fn request_resynchronization(
        &mut self,
        request: RepresentationResynchronizationRequest,
    ) -> RepresentationResynchronizationAdmission {
        // This script emits full Representation snapshots, never a delta gap.
        // Notification gaps use the independently scoped change subscription.
        let cause = if self.closed {
            RepresentationResynchronizationFailure::Closed
        } else {
            RepresentationResynchronizationFailure::UnknownGap
        };
        RepresentationResynchronizationAdmission::NotAdmitted { cause, request }
    }

    fn close(
        &mut self,
        request: InteractionCloseRequest,
    ) -> InteractionCloseOutcome {
        teardown::close(self, request)
    }
}
