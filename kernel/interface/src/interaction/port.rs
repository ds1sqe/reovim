//! Public transport-neutral Interaction Session port.

use super::delivery::intent::{
    InteractionCancellationRequest, InteractionCancellationResult,
    InteractionIntentAdmission, InteractionOutcomeDelivery,
    InteractionOutcomeRequest, InteractionReconcileRequest,
    InteractionReconcileResult,
};
use super::delivery::session::{
    InteractionCloseOutcome, InteractionCloseRequest,
    InteractionNegotiationOutcome, InteractionNegotiationRequest,
    RepresentationAttachmentOutcome, RepresentationAttachmentRequest,
};
use super::delivery::update::{
    RepresentationResynchronizationAdmission,
    RepresentationResynchronizationRequest,
    RepresentationUpdateAcknowledgement,
    RepresentationUpdateAcknowledgementResult, RepresentationUpdateDelivery,
    RepresentationUpdateRequest,
};
use super::descriptor::InteractionSessionDescriptor;
use super::intent::InteractionIntent;

/// Public semantic endpoint provided by one Kernel Runtime Session.
///
/// All stateful calls require one exclusive mutable endpoint owner. Assembly
/// and Protocol Bindings preserve that serialization for a Session generation;
/// deferred work uses the independent pending and retention bounds.
pub trait InteractionSessionPort {
    /// Returns the immutable descriptor governing this exact endpoint.
    fn descriptor(&self) -> &InteractionSessionDescriptor;

    /// Negotiates one exact Session without probing another provider.
    fn negotiate(
        &mut self,
        request: InteractionNegotiationRequest,
    ) -> InteractionNegotiationOutcome;

    /// Attaches one Representation stream and transfers initial state.
    fn attach(
        &mut self,
        request: RepresentationAttachmentRequest,
    ) -> RepresentationAttachmentOutcome;

    /// Attempts bounded non-waiting ownership transfer of one Intent.
    fn submit_intent(
        &mut self,
        intent: InteractionIntent,
    ) -> InteractionIntentAdmission;

    /// Requests cooperative cancellation without claiming terminality.
    fn request_cancellation(
        &mut self,
        request: InteractionCancellationRequest,
    ) -> InteractionCancellationResult;

    /// Takes one terminal Outcome without waiting.
    fn take_outcome(
        &mut self,
        request: InteractionOutcomeRequest,
    ) -> InteractionOutcomeDelivery;

    /// Reconciles one exact uncertain or unavailable Outcome.
    fn reconcile_outcome(
        &mut self,
        request: InteractionReconcileRequest,
    ) -> InteractionReconcileResult;

    /// Takes one ordered Representation Update without waiting.
    fn take_update(
        &mut self,
        request: RepresentationUpdateRequest,
    ) -> RepresentationUpdateDelivery;

    /// Records one Client Update observation and returns its credit.
    fn acknowledge_update(
        &mut self,
        acknowledgement: RepresentationUpdateAcknowledgement,
    ) -> RepresentationUpdateAcknowledgementResult;

    /// Attempts bounded ownership transfer of full resynchronization work.
    fn request_resynchronization(
        &mut self,
        request: RepresentationResynchronizationRequest,
    ) -> RepresentationResynchronizationAdmission;

    /// Closes admission and resolves or exposes all retained ownership.
    fn close(
        &mut self,
        request: InteractionCloseRequest,
    ) -> InteractionCloseOutcome;
}
