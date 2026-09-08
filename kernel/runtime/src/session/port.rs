use reovim_kernel_interface::interaction::delivery::{
    intent as intent_delivery, session as session_delivery,
    update as update_delivery,
};
use reovim_kernel_interface::interaction::descriptor;
use reovim_kernel_interface::interaction::intent::InteractionIntent;
use reovim_kernel_interface::interaction::port::InteractionSessionPort;

use super::{Phase, Session};

impl InteractionSessionPort for Session {
    fn descriptor(&self) -> &descriptor::InteractionSessionDescriptor {
        &self.binding.session
    }

    fn negotiate(
        &mut self,
        request: session_delivery::InteractionNegotiationRequest,
    ) -> session_delivery::InteractionNegotiationOutcome {
        use session_delivery::{
            InteractionNegotiationFailure as Failure,
            InteractionNegotiationOutcome as Outcome,
        };
        if !request.session().same_generation(self.binding.session.stamp())
            || request.scope() != self.binding.session.scope()
            || request.capabilities() != self.binding.session.capabilities()
        {
            return Outcome::rejected(Failure::Replaced);
        }
        if !self.ready() {
            return Outcome::rejected(Failure::NotReady);
        }
        if self.phase == Phase::Opened {
            self.phase = Phase::Negotiated;
        }
        Outcome::negotiated(request.session().handle())
    }

    fn attach(
        &mut self,
        request: session_delivery::RepresentationAttachmentRequest,
    ) -> session_delivery::RepresentationAttachmentOutcome {
        use session_delivery::{
            RepresentationAttachmentFailure as Failure,
            RepresentationAttachmentOutcome as Outcome,
            RepresentationAttachmentReadyBindingOutcome as Binding,
        };
        let failure = if self.phase != Phase::Negotiated {
            Some(Failure::NotReady)
        } else if request.descriptor() != &self.binding.attachment {
            Some(Failure::Replaced)
        } else if self.initial.is_none() {
            Some(Failure::NotReady)
        } else {
            None
        };
        if let Some(cause) = failure {
            return Outcome::rejected(
                session_delivery::RepresentationAttachmentRejection::new(
                    cause, request,
                ),
            );
        }
        let initial = self.initial.take().expect("checked initial state");
        self.transferred(&initial);
        let issued = initial.stamp().identity();
        let Binding::Bound(ready) =
            session_delivery::RepresentationAttachmentReady::bind(
                &self.binding.session,
                request.into_descriptor(),
                initial,
            )
        else {
            panic!("Runtime prepared the selected initial snapshot");
        };
        self.issued.push(issued);
        self.phase = Phase::Attached;
        Outcome::ready(ready)
    }

    fn submit_intent(
        &mut self,
        intent: InteractionIntent,
    ) -> intent_delivery::InteractionIntentAdmission {
        self.admit(intent)
    }

    fn request_cancellation(
        &mut self,
        request: intent_delivery::InteractionCancellationRequest,
    ) -> intent_delivery::InteractionCancellationResult {
        self.cancel(&request)
    }

    fn take_outcome(
        &mut self,
        request: intent_delivery::InteractionOutcomeRequest,
    ) -> intent_delivery::InteractionOutcomeDelivery {
        self.deliver_outcome(&request)
    }

    fn reconcile_outcome(
        &mut self,
        request: intent_delivery::InteractionReconcileRequest,
    ) -> intent_delivery::InteractionReconcileResult {
        let cause = if request
            .request()
            .session()
            .matches(self.binding.session.stamp())
        {
            intent_delivery::InteractionReconcileUnavailable::UnknownRequest
        } else {
            intent_delivery::InteractionReconcileUnavailable::StaleSession
        };
        // This profile retains exact terminal records and issues no uncertainty
        // token. Caller-invented tokens cannot reclaim transferred ownership.
        intent_delivery::InteractionReconcileResult::unavailable(cause)
    }

    fn take_update(
        &mut self,
        request: update_delivery::RepresentationUpdateRequest,
    ) -> update_delivery::RepresentationUpdateDelivery {
        self.deliver_update(request)
    }

    fn acknowledge_update(
        &mut self,
        request: update_delivery::RepresentationUpdateAcknowledgement,
    ) -> update_delivery::RepresentationUpdateAcknowledgementResult {
        self.acknowledge(request)
    }

    fn request_resynchronization(
        &mut self,
        request: update_delivery::RepresentationResynchronizationRequest,
    ) -> update_delivery::RepresentationResynchronizationAdmission {
        use update_delivery::RepresentationResynchronizationFailure as Failure;
        update_delivery::RepresentationResynchronizationAdmission::NotAdmitted {
            cause: Failure::Unsupported,
            request,
        }
    }

    fn close(
        &mut self,
        request: session_delivery::InteractionCloseRequest,
    ) -> session_delivery::InteractionCloseOutcome {
        self.close_session(request)
    }
}
