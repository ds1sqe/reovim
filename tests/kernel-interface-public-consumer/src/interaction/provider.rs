use reovim_kernel_interface::bound::{CheckedCount, ZeroPolicy};
use reovim_kernel_interface::buffer::BufferStateStamp;
use reovim_kernel_interface::cancellation::{
    CancellationOutcome, ExecutionOutcome,
};
use reovim_kernel_interface::interaction::bounds;
use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionCancellationRequest, InteractionCancellationResult,
    InteractionIntentAdmission, InteractionOutcomeDelivery,
    InteractionOutcomeRequest, InteractionOutcomeUnavailable,
    InteractionReconcileRequest, InteractionReconcileResult,
    InteractionReconcileUnavailable,
};
use reovim_kernel_interface::interaction::delivery::session::{
    InteractionCloseOutcome, InteractionCloseRequest,
    InteractionCloseRetainedState, InteractionIncompleteClose,
    InteractionNegotiationFailure, InteractionNegotiationOutcome,
    InteractionNegotiationRequest, InteractionNegotiationStatus,
    RepresentationAttachmentFailure, RepresentationAttachmentOutcome,
    RepresentationAttachmentReady, RepresentationAttachmentReadyBindingOutcome,
    RepresentationAttachmentRejection, RepresentationAttachmentRequest,
    RepresentationAttachmentRequestBindingOutcome,
    RepresentationAttachmentStatus,
};
use reovim_kernel_interface::interaction::delivery::update::{
    RepresentationResynchronizationAdmission,
    RepresentationResynchronizationFailure,
    RepresentationResynchronizationRequest,
    RepresentationUpdateAcknowledgement,
    RepresentationUpdateAcknowledgementResult, RepresentationUpdateApplication,
    RepresentationUpdateDelivery, RepresentationUpdateDeliveryStatus,
    RepresentationUpdateRequest, RepresentationUpdateUnavailable,
};
use reovim_kernel_interface::interaction::descriptor::{
    InteractionSessionDescriptor, RepresentationAttachmentDescriptor,
};
use reovim_kernel_interface::interaction::identity;
use reovim_kernel_interface::interaction::intent::InteractionIntent;
use reovim_kernel_interface::interaction::outcome::{
    InteractionOutcome, InteractionOutcomeCause,
};
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::session::InteractionRequestStamp;
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateIdentityStamp,
    RepresentationUpdateLineage,
};

use super::support;

struct ScriptedInteractionSession {
    descriptor: InteractionSessionDescriptor,
    attachment: Option<RepresentationAttachmentDescriptor>,
    initial: Option<RepresentationUpdate>,
    pending: Option<InteractionIntent>,
    outcome: Option<InteractionOutcome>,
    update: Option<RepresentationUpdate>,
    delivered_outcome: Option<InteractionRequestStamp>,
    delivered_update: Option<RepresentationUpdateIdentityStamp>,
    acknowledged_update: Option<RepresentationUpdateIdentityStamp>,
    delivered_control: Option<RepresentationUpdateIdentityStamp>,
    acknowledged_control: Option<RepresentationUpdateIdentityStamp>,
    resynchronization: Option<RepresentationResynchronizationRequest>,
    resynchronization_totals:
        Option<bounds::RepresentationResynchronizationTotals>,
    cancellation_requested: bool,
    credits: u64,
    negotiated: bool,
    closed: bool,
    close_outcome: Option<InteractionCloseOutcome>,
}

impl ScriptedInteractionSession {
    fn new(fixture: &support::InteractionFixture) -> Option<Self> {
        Some(Self {
            descriptor: fixture.session.clone(),
            attachment: None,
            initial: Some(support::initial_update(fixture)?),
            pending: None,
            outcome: None,
            update: None,
            delivered_outcome: None,
            delivered_update: None,
            acknowledged_update: None,
            delivered_control: None,
            acknowledged_control: None,
            resynchronization: None,
            resynchronization_totals: None,
            cancellation_requested: false,
            credits: 0,
            negotiated: false,
            closed: false,
            close_outcome: None,
        })
    }

    fn complete_accepted(&mut self, result: &BufferStateStamp) -> bool {
        let Some(intent) = self.pending.as_ref() else { return false };
        let outcome = InteractionOutcome::accepted(
            intent.stamp(),
            intent.semantic().stamp(),
            *result,
        );
        let Some(outcome) = outcome.ok() else { return false };
        self.pending = None;
        self.outcome = Some(outcome);
        true
    }

    fn queue_update(&mut self, update: RepresentationUpdate) -> bool {
        if self.update.is_some() {
            return false;
        }
        self.update = Some(update);
        true
    }

    fn complete_resynchronization(
        &mut self,
        candidate: &mut Option<RepresentationUpdate>,
    ) -> bool {
        let Some(update) = candidate.as_ref() else { return false };
        let Some(request) = self.resynchronization.as_ref() else {
            return false;
        };
        let Some(attachment) = self.attachment.as_ref() else {
            return false;
        };
        if self.closed
            || self.update.is_some()
            || update.validate(&self.descriptor, attachment).is_err()
            || request.validate_result(update).is_err()
        {
            return false;
        }
        self.update = candidate.take();
        self.resynchronization = None;
        self.resynchronization_totals = None;
        true
    }
}

impl ::reovim_kernel_interface::interaction::port::InteractionSessionPort
    for ScriptedInteractionSession
{
    fn descriptor(&self) -> &InteractionSessionDescriptor {
        &self.descriptor
    }

    fn negotiate(
        &mut self,
        request: InteractionNegotiationRequest,
    ) -> InteractionNegotiationOutcome {
        if self.closed {
            return InteractionNegotiationOutcome::rejected(
                InteractionNegotiationFailure::Closed,
            );
        }
        if !request.session().same_generation(self.descriptor.stamp()) {
            return InteractionNegotiationOutcome::rejected(
                InteractionNegotiationFailure::Replaced,
            );
        }
        self.negotiated = true;
        InteractionNegotiationOutcome::negotiated(request.session().handle())
    }

    fn attach(
        &mut self,
        request: RepresentationAttachmentRequest,
    ) -> RepresentationAttachmentOutcome {
        if !self.negotiated || self.closed {
            let cause = attachment_unavailable_cause(self.closed);
            return RepresentationAttachmentOutcome::rejected(
                RepresentationAttachmentRejection::new(cause, request),
            );
        }
        let Some(initial) = self.initial.take() else {
            return RepresentationAttachmentOutcome::rejected(
                RepresentationAttachmentRejection::new(
                    RepresentationAttachmentFailure::RuntimeFailure,
                    request,
                ),
            );
        };
        let descriptor = request.descriptor().clone();
        match RepresentationAttachmentReady::bind(
            &self.descriptor,
            descriptor,
            initial,
        ) {
            RepresentationAttachmentReadyBindingOutcome::Bound(ready) => {
                self.attachment = Some(ready.descriptor().clone());
                self.credits = ready.descriptor().initial_credit().value();
                RepresentationAttachmentOutcome::ready(ready)
            }
            RepresentationAttachmentReadyBindingOutcome::Rejected(
                rejection,
            ) => {
                let _ = rejection.into_parts();
                RepresentationAttachmentOutcome::rejected(
                    RepresentationAttachmentRejection::new(
                        RepresentationAttachmentFailure::RuntimeFailure,
                        request,
                    ),
                )
            }
        }
    }

    fn submit_intent(
        &mut self,
        intent: InteractionIntent,
    ) -> InteractionIntentAdmission {
        if self.closed {
            return InteractionIntentAdmission::not_admitted(
                intent,
                InteractionOutcomeCause::Closed,
            );
        }
        if self.pending.is_some() {
            return InteractionIntentAdmission::not_admitted(
                intent,
                InteractionOutcomeCause::Pressure,
            );
        }
        let admitted = InteractionIntentAdmission::admitted(&intent);
        self.pending = Some(intent);
        admitted
    }

    fn request_cancellation(
        &mut self,
        request: InteractionCancellationRequest,
    ) -> InteractionCancellationResult {
        let Some(pending) = self.pending.as_ref() else {
            return InteractionCancellationResult::UnknownRequest;
        };
        if !pending.stamp().same_request(request.request()) {
            return InteractionCancellationResult::UnknownRequest;
        }
        if self.cancellation_requested {
            return InteractionCancellationResult::AlreadyRequested(
                CancellationOutcome::new(
                    request.cause(),
                    ExecutionOutcome::OwnedNonterminal,
                ),
            );
        }
        self.cancellation_requested = true;
        InteractionCancellationResult::Requested(CancellationOutcome::new(
            request.cause(),
            ExecutionOutcome::OwnedNonterminal,
        ))
    }

    fn take_outcome(
        &mut self,
        request: InteractionOutcomeRequest,
    ) -> InteractionOutcomeDelivery {
        let Some(outcome) = self.outcome else {
            return missing_outcome(self, &request);
        };
        if !outcome.request().same_request(request.request()) {
            return InteractionOutcomeDelivery::unavailable(
                InteractionOutcomeUnavailable::UnknownRequest,
            );
        }
        self.outcome = None;
        self.delivered_outcome = Some(outcome.request());
        InteractionOutcomeDelivery::delivered(outcome)
    }

    fn reconcile_outcome(
        &mut self,
        request: InteractionReconcileRequest,
    ) -> InteractionReconcileResult {
        if self.closed {
            return InteractionReconcileResult::unavailable(
                InteractionReconcileUnavailable::Closed,
            );
        }
        if self.pending.as_ref().is_some_and(|intent| {
            intent.stamp().same_request(request.request())
        }) {
            return InteractionReconcileResult::unavailable(
                InteractionReconcileUnavailable::Pending,
            );
        }
        InteractionReconcileResult::unavailable(
            InteractionReconcileUnavailable::UnknownRequest,
        )
    }

    fn take_update(
        &mut self,
        request: RepresentationUpdateRequest,
    ) -> RepresentationUpdateDelivery {
        if self.closed {
            return RepresentationUpdateDelivery::unavailable(
                RepresentationUpdateUnavailable::Closed,
            );
        }
        if update_already_delivered(self, request.expected()) {
            return RepresentationUpdateDelivery::unavailable(
                RepresentationUpdateUnavailable::AlreadyDelivered,
            );
        }
        let Some(update) = self.update.as_ref() else {
            return RepresentationUpdateDelivery::unavailable(
                RepresentationUpdateUnavailable::Pending,
            );
        };
        let control = !update.lineage().has_payload();
        if (!control && self.credits == 0)
            || (control && self.delivered_control.is_some())
        {
            return RepresentationUpdateDelivery::unavailable(
                RepresentationUpdateUnavailable::NoCredit,
            );
        }
        if request.expected().value() < update.stamp().cursor().value() {
            return RepresentationUpdateDelivery::unavailable(
                RepresentationUpdateUnavailable::CursorBehind,
            );
        }
        if request.expected().value() > update.stamp().cursor().value() {
            return RepresentationUpdateDelivery::unavailable(
                RepresentationUpdateUnavailable::CursorAhead,
            );
        }
        let stamp = update.stamp().identity();
        match RepresentationUpdateDelivery::try_bind(&request, &mut self.update)
        {
            Ok(delivery) => {
                self.record_delivery(stamp, control);
                delivery
            }
            Err(cause) => RepresentationUpdateDelivery::unavailable(
                RepresentationUpdateUnavailable::ProviderContractViolation(
                    cause,
                ),
            ),
        }
    }

    fn acknowledge_update(
        &mut self,
        acknowledgement: RepresentationUpdateAcknowledgement,
    ) -> RepresentationUpdateAcknowledgementResult {
        if self.closed {
            return RepresentationUpdateAcknowledgementResult::Closed;
        }
        if self.delivered_control == Some(acknowledgement.stamp()) {
            self.delivered_control = None;
            self.acknowledged_control = Some(acknowledgement.stamp());
            return RepresentationUpdateAcknowledgementResult::Accepted;
        }
        if self.acknowledged_control == Some(acknowledgement.stamp()) {
            return RepresentationUpdateAcknowledgementResult::AlreadyRecorded;
        }
        if self
            .acknowledged_update
            .is_some_and(|stamp| stamp.same_update(acknowledgement.stamp()))
        {
            return RepresentationUpdateAcknowledgementResult::AlreadyRecorded;
        }
        let Some(delivered) = self.delivered_update else {
            return RepresentationUpdateAcknowledgementResult::UnknownUpdate;
        };
        if !delivered.same_update(acknowledgement.stamp()) {
            return RepresentationUpdateAcknowledgementResult::UnknownUpdate;
        }
        self.acknowledged_update = Some(delivered);
        self.credits = self.credits.saturating_add(1);
        RepresentationUpdateAcknowledgementResult::Accepted
    }

    fn request_resynchronization(
        &mut self,
        request: RepresentationResynchronizationRequest,
    ) -> RepresentationResynchronizationAdmission {
        if self.closed {
            return RepresentationResynchronizationAdmission::NotAdmitted {
                cause: RepresentationResynchronizationFailure::Closed,
                request,
            };
        }
        if self.resynchronization.is_some() {
            return RepresentationResynchronizationAdmission::NotAdmitted {
                cause: RepresentationResynchronizationFailure::DuplicateRequest,
                request,
            };
        }
        if !self.known_gap(&request) {
            return RepresentationResynchronizationAdmission::NotAdmitted {
                cause: RepresentationResynchronizationFailure::UnknownGap,
                request,
            };
        }
        let charge = request.charge();
        let totals = self
            .descriptor
            .bounds()
            .updates()
            .admit_resynchronization(0, 0, 0, charge);
        let Ok(totals) = totals else {
            return RepresentationResynchronizationAdmission::NotAdmitted {
                cause: RepresentationResynchronizationFailure::Pressure,
                request,
            };
        };
        let admitted =
            RepresentationResynchronizationAdmission::admitted(&request);
        self.resynchronization = Some(request);
        self.resynchronization_totals = Some(totals);
        admitted
    }

    fn close(
        &mut self,
        request: InteractionCloseRequest,
    ) -> InteractionCloseOutcome {
        if !request.session().matches(self.descriptor.stamp()) {
            return InteractionCloseOutcome::StaleSession;
        }
        if let Some(outcome) = self.close_outcome {
            return outcome;
        }
        self.closed = true;
        let outcome = if self.has_retained_obligations() {
            self.incomplete_close().map_or(
                InteractionCloseOutcome::StaleSession,
                InteractionCloseOutcome::Incomplete,
            )
        } else {
            InteractionCloseOutcome::Closed
        };
        self.close_outcome = Some(outcome);
        outcome
    }
}

impl ScriptedInteractionSession {
    const fn record_delivery(
        &mut self,
        stamp: RepresentationUpdateIdentityStamp,
        control: bool,
    ) {
        if control {
            self.delivered_control = Some(stamp);
        } else {
            self.credits -= 1;
            self.delivered_update = Some(stamp);
        }
    }

    fn known_gap(
        &self,
        request: &RepresentationResynchronizationRequest,
    ) -> bool {
        self.delivered_control.or(self.acknowledged_control).is_some_and(
            |gap| {
                gap.stream().same_generation(request.stream())
                    && gap.update() == request.gap()
                    && gap.cursor() == request.cursor()
            },
        )
    }

    fn has_retained_obligations(&self) -> bool {
        self.pending.is_some()
            || self.outcome.is_some()
            || self.update.is_some()
            || self.resynchronization.is_some()
            || self.delivered_control.is_some()
            || self.has_unacknowledged_update()
    }

    fn has_unacknowledged_update(&self) -> bool {
        self.delivered_update.is_some_and(|delivered| {
            !self
                .acknowledged_update
                .is_some_and(|value| value.same_update(delivered))
        })
    }

    fn incomplete_close(&self) -> Option<InteractionIncompleteClose> {
        let pending = checked_flag(self.pending.is_some())?;
        let outcomes = checked_flag(self.outcome.is_some())?;
        let updates = checked_flag(self.update.is_some())?;
        let unacknowledged = CheckedCount::new(
            u64::from(self.has_unacknowledged_update())
                + u64::from(self.delivered_control.is_some()),
            ZeroPolicy::Allow,
        )
        .ok()?;
        let reconciliations = checked_flag(false)?;
        let execution =
            if self.pending.is_some() || self.resynchronization.is_some() {
                ExecutionOutcome::OwnedNonterminal
            } else {
                ExecutionOutcome::Stopped
            };
        let retained = InteractionCloseRetainedState::new(
            pending,
            outcomes,
            updates,
            unacknowledged,
            reconciliations,
            self.resynchronization_totals,
        );
        InteractionIncompleteClose::new(
            self.descriptor.bounds(),
            execution,
            retained,
        )
        .ok()
    }
}

pub fn exercise() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    outcome_first_journey(&fixture)
        && update_first_journey(&fixture)
        && close_faults(&fixture)
        && resynchronization_close_faults(&fixture)
        && control_pressure(&fixture)
}

fn control_pressure(fixture: &support::InteractionFixture) -> bool {
    let Some(mut provider) = ScriptedInteractionSession::new(fixture) else {
        return false;
    };
    if !negotiate_and_attach(fixture, &mut provider) {
        return false;
    }
    // Inject exhausted data credit; the control reserve remains available.
    provider.credits = 0;
    let Some(gap) = support::gap_update(fixture) else { return false };
    let stamp = gap.stamp().identity();
    let Some(request) = RepresentationUpdateRequest::new(
        &fixture.session,
        &fixture.attachment,
        stamp.cursor(),
    )
    .ok() else {
        return false;
    };
    if !provider.queue_update(gap) {
        return false;
    }
    let gap_delivered = provider.take_update(request).status()
        == RepresentationUpdateDeliveryStatus::Delivered;
    let control_released = acknowledge(fixture, &mut provider, stamp);
    let Some(delta) = support::delta_update(fixture) else { return false };
    let Some(data) = RepresentationUpdateRequest::new(
        &fixture.session,
        &fixture.attachment,
        delta.stamp().cursor(),
    )
    .ok() else {
        return false;
    };
    provider.queue_update(delta)
        && gap_delivered
        && control_released
        && provider.credits == 0
        && provider.take_update(data).status()
            == RepresentationUpdateDeliveryStatus::Unavailable(
                RepresentationUpdateUnavailable::NoCredit,
            )
}

fn outcome_first_journey(fixture: &support::InteractionFixture) -> bool {
    let Some(mut provider) = ScriptedInteractionSession::new(fixture) else {
        return false;
    };
    if !negotiate_and_attach(fixture, &mut provider) {
        return false;
    }
    let Some(intent) = support::make_intent(fixture, 4_000, 4_001, 4_002)
    else {
        return false;
    };
    let request_stamp = intent.stamp();
    let port: &mut dyn InteractionSessionPort = &mut provider;
    let admission = port.submit_intent(intent);
    if admission.admitted_request() != Some(request_stamp) {
        return false;
    }
    if !cancel_and_reconcile_pending(fixture, &mut provider, &request_stamp) {
        return false;
    }
    let Some(result) = crate::buffer::state(1, 1, 1) else { return false };
    if !provider.complete_accepted(&result) {
        return false;
    }
    let Some(delta) = support::delta_update(fixture) else { return false };
    let delta_stamp = delta.stamp().identity();
    if !provider.queue_update(delta) {
        return false;
    }
    outcome_and_update(fixture, &mut provider, &request_stamp, delta_stamp)
}

fn negotiate_and_attach(
    fixture: &support::InteractionFixture,
    provider: &mut ScriptedInteractionSession,
) -> bool {
    let Some(negotiation) = InteractionNegotiationRequest::new(
        &fixture.session,
        fixture.session.stamp(),
        fixture.session.scope(),
        fixture.session.capabilities(),
    )
    .ok() else {
        return false;
    };
    let port: &mut dyn InteractionSessionPort = provider;
    if port.negotiate(negotiation).status()
        != InteractionNegotiationStatus::Negotiated
    {
        return false;
    }
    let binding = RepresentationAttachmentRequest::bind(
        &fixture.session,
        fixture.attachment.clone(),
    );
    let RepresentationAttachmentRequestBindingOutcome::Bound(attachment) =
        binding
    else {
        return false;
    };
    port.attach(attachment).status() == RepresentationAttachmentStatus::Ready
}

fn outcome_and_update(
    fixture: &support::InteractionFixture,
    provider: &mut ScriptedInteractionSession,
    request_stamp: &InteractionRequestStamp,
    delta_stamp: RepresentationUpdateIdentityStamp,
) -> bool {
    let Some(outcome_request) =
        InteractionOutcomeRequest::new(&fixture.session, *request_stamp).ok()
    else {
        return false;
    };
    let Some(update_request) = RepresentationUpdateRequest::new(
        &fixture.session,
        &fixture.attachment,
        delta_stamp.cursor(),
    )
    .ok() else {
        return false;
    };
    let port: &mut dyn InteractionSessionPort = provider;
    let outcome_ok = port.take_outcome(outcome_request).status()
        == reovim_kernel_interface::interaction::delivery::intent::
            InteractionOutcomeDeliveryStatus::Delivered;
    let outcome_duplicate = port.take_outcome(outcome_request).status()
        == reovim_kernel_interface::interaction::delivery::intent::
            InteractionOutcomeDeliveryStatus::Unavailable(
                InteractionOutcomeUnavailable::AlreadyDelivered,
            );
    let update = port.take_update(update_request);
    let update_ok = update.status()
        == RepresentationUpdateDeliveryStatus::Delivered
        && update.update().is_some_and(|value| {
            value.validate(&fixture.session, &fixture.attachment).is_ok()
        });
    let acknowledgement = RepresentationUpdateAcknowledgement::new(
        &fixture.session,
        &fixture.attachment,
        delta_stamp,
        RepresentationUpdateApplication::Installed,
    );
    let Some(acknowledgement) = acknowledgement.ok() else {
        return false;
    };
    let acknowledged = port.acknowledge_update(acknowledgement)
        == RepresentationUpdateAcknowledgementResult::Accepted;
    let acknowledged_twice = port.acknowledge_update(acknowledgement)
        == RepresentationUpdateAcknowledgementResult::AlreadyRecorded;
    let update_duplicate = port.take_update(update_request).status()
        == RepresentationUpdateDeliveryStatus::Unavailable(
            RepresentationUpdateUnavailable::AlreadyDelivered,
        );
    let updates_resynchronized =
        gap_resynchronization_journey(fixture, provider);
    let Some(close) = InteractionCloseRequest::new(&fixture.session, 4).ok()
    else {
        return false;
    };
    outcome_ok
        && outcome_duplicate
        && update_ok
        && acknowledged
        && acknowledged_twice
        && update_duplicate
        && updates_resynchronized
        && provider.close(close) == InteractionCloseOutcome::Closed
        && provider.close(close) == InteractionCloseOutcome::Closed
}

fn gap_resynchronization_journey(
    fixture: &support::InteractionFixture,
    provider: &mut ScriptedInteractionSession,
) -> bool {
    let Some(gap) = support::gap_update(fixture) else { return false };
    let gap_stamp = gap.stamp().identity();
    if !provider.queue_update(gap) {
        return false;
    }
    let Some(request) = RepresentationUpdateRequest::new(
        &fixture.session,
        &fixture.attachment,
        gap_stamp.cursor(),
    )
    .ok() else {
        return false;
    };
    let delivery = provider.take_update(request);
    let Some(gap) = delivery.into_update() else { return false };
    if !acknowledge(fixture, provider, gap_stamp) {
        return false;
    }
    let first = RepresentationResynchronizationRequest::new(
        &fixture.session,
        &fixture.attachment,
        &gap,
    );
    let duplicate = RepresentationResynchronizationRequest::new(
        &fixture.session,
        &fixture.attachment,
        &gap,
    );
    let (Some(first), Some(duplicate)) = (first.ok(), duplicate.ok()) else {
        return false;
    };
    if !admit_and_reject_duplicate(provider, first, duplicate) {
        return false;
    }
    let Some(update) = support::resync_update(fixture) else { return false };
    let stamp = update.stamp().identity();
    let mut candidate = Some(update);
    if !provider.complete_resynchronization(&mut candidate) {
        return false;
    }
    let Some(request) = RepresentationUpdateRequest::new(
        &fixture.session,
        &fixture.attachment,
        stamp.cursor(),
    )
    .ok() else {
        return false;
    };
    provider.take_update(request).status()
        == RepresentationUpdateDeliveryStatus::Delivered
        && acknowledge(fixture, provider, stamp)
}

fn admit_and_reject_duplicate(
    provider: &mut ScriptedInteractionSession,
    first: RepresentationResynchronizationRequest,
    duplicate: RepresentationResynchronizationRequest,
) -> bool {
    let admitted = provider.request_resynchronization(first);
    let admitted_ok = match admitted {
        RepresentationResynchronizationAdmission::Admitted(receipt) => {
            receipt.resynchronization().value() == 1_303
                && receipt.charge().requests().value() == 1
                && receipt.charge().work().value() == 8
                && receipt.charge().bytes().value() == 64
        }
        RepresentationResynchronizationAdmission::NotAdmitted { .. } => false,
    };
    let rejected = provider.request_resynchronization(duplicate);
    let duplicate_ok = match rejected {
        RepresentationResynchronizationAdmission::NotAdmitted {
            cause,
            request,
        } => {
            cause == RepresentationResynchronizationFailure::DuplicateRequest
                && request.resynchronization().value() == 1_303
        }
        RepresentationResynchronizationAdmission::Admitted(_) => false,
    };
    admitted_ok && duplicate_ok
}

fn acknowledge(
    fixture: &support::InteractionFixture,
    provider: &mut ScriptedInteractionSession,
    stamp: RepresentationUpdateIdentityStamp,
) -> bool {
    let acknowledgement = RepresentationUpdateAcknowledgement::new(
        &fixture.session,
        &fixture.attachment,
        stamp,
        RepresentationUpdateApplication::Installed,
    );
    let Some(acknowledgement) = acknowledgement.ok() else {
        return false;
    };
    provider.acknowledge_update(acknowledgement)
        == RepresentationUpdateAcknowledgementResult::Accepted
}

fn cancel_and_reconcile_pending(
    fixture: &support::InteractionFixture,
    provider: &mut ScriptedInteractionSession,
    request: &InteractionRequestStamp,
) -> bool {
    let cancellation = InteractionCancellationRequest::new(
        &fixture.session,
        *request,
        reovim_kernel_interface::cancellation::CancellationCause::
            ExplicitRequest,
    );
    let Some(cancellation) = cancellation.ok() else { return false };
    let reconcile = InteractionReconcileRequest::new(
        &fixture.session,
        *request,
        identity::InteractionReconcileIdentity::new(4_003),
    );
    let Some(reconcile) = reconcile.ok() else { return false };
    let port: &mut dyn InteractionSessionPort = provider;
    let expected = CancellationOutcome::new(
        reovim_kernel_interface::cancellation::CancellationCause::
            ExplicitRequest,
        ExecutionOutcome::OwnedNonterminal,
    );
    port.request_cancellation(cancellation)
        == InteractionCancellationResult::Requested(expected)
        && port.request_cancellation(cancellation)
            == InteractionCancellationResult::AlreadyRequested(expected)
        && port.reconcile_outcome(reconcile).status()
            == reovim_kernel_interface::interaction::delivery::intent::
                InteractionReconcileStatus::Unavailable(
                    InteractionReconcileUnavailable::Pending,
                )
}

fn update_first_journey(fixture: &support::InteractionFixture) -> bool {
    let Some(mut provider) = ScriptedInteractionSession::new(fixture) else {
        return false;
    };
    if !negotiate_and_attach(fixture, &mut provider) {
        return false;
    }
    let Some(intent) = support::make_intent(fixture, 4_100, 4_101, 4_102)
    else {
        return false;
    };
    let request_stamp = intent.stamp();
    if provider.submit_intent(intent).admitted_request() != Some(request_stamp)
    {
        return false;
    }
    let Some(result) = crate::buffer::state(1, 1, 1) else { return false };
    if !provider.complete_accepted(&result) {
        return false;
    }
    let Some(delta) = support::delta_update(fixture) else { return false };
    let delta_stamp = delta.stamp().identity();
    if !provider.queue_update(delta) {
        return false;
    }
    update_then_outcome(fixture, &mut provider, &request_stamp, delta_stamp)
}

fn close_faults(fixture: &support::InteractionFixture) -> bool {
    let Some(mut provider) = ScriptedInteractionSession::new(fixture) else {
        return false;
    };
    if !negotiate_and_attach(fixture, &mut provider) {
        return false;
    }
    let Some(intent) = support::make_intent(fixture, 4_200, 4_201, 4_202)
    else {
        return false;
    };
    if provider.submit_intent(intent).admitted_request().is_none() {
        return false;
    }
    let close = InteractionCloseRequest::new(&fixture.session, 4);
    let Some(close) = close.ok() else { return false };
    let first = provider.close(close);
    let second = provider.close(close);
    let pending_visible = match first {
        InteractionCloseOutcome::Incomplete(report) => {
            report.execution() == ExecutionOutcome::OwnedNonterminal
                && report.pending_intents().value() == 1
        }
        _ => false,
    };
    let Some(replacement) = support::replacement_session(fixture) else {
        return false;
    };
    let stale = InteractionCloseRequest::new(&replacement, 4);
    let Some(stale) = stale.ok() else { return false };
    pending_visible
        && first == second
        && provider.close(stale) == InteractionCloseOutcome::StaleSession
}

fn resynchronization_close_faults(
    fixture: &support::InteractionFixture,
) -> bool {
    let Some(mut incomplete) = ScriptedInteractionSession::new(fixture) else {
        return false;
    };
    if !negotiate_and_attach(fixture, &mut incomplete)
        || !admit_gap_resynchronization(fixture, &mut incomplete)
    {
        return false;
    }
    let Some(close) = InteractionCloseRequest::new(&fixture.session, 4).ok()
    else {
        return false;
    };
    let first = incomplete.close(close);
    let visible = resynchronization_is_visible(first);
    if !visible || incomplete.close(close) != first {
        return false;
    }
    let Some(mut invalid) = ScriptedInteractionSession::new(fixture) else {
        return false;
    };
    if !negotiate_and_attach(fixture, &mut invalid)
        || !admit_gap_resynchronization(fixture, &mut invalid)
    {
        return false;
    }
    let Some(update) = wrong_resynchronization_update(fixture) else {
        return false;
    };
    let stamp = update.stamp().identity();
    let mut candidate = Some(update);
    let rejected = !invalid.complete_resynchronization(&mut candidate);
    let owner_preserved = rejected
        && candidate
            .as_ref()
            .is_some_and(|update| update.stamp().identity().same_update(stamp));
    owner_preserved && resynchronization_is_visible(invalid.close(close))
}

fn admit_gap_resynchronization(
    fixture: &support::InteractionFixture,
    provider: &mut ScriptedInteractionSession,
) -> bool {
    let Some(gap) = support::gap_update(fixture) else { return false };
    let stamp = gap.stamp().identity();
    if !provider.queue_update(gap) {
        return false;
    }
    let Some(request) = RepresentationUpdateRequest::new(
        &fixture.session,
        &fixture.attachment,
        stamp.cursor(),
    )
    .ok() else {
        return false;
    };
    let Some(gap) = provider.take_update(request).into_update() else {
        return false;
    };
    if !acknowledge(fixture, provider, stamp) {
        return false;
    }
    let request = RepresentationResynchronizationRequest::new(
        &fixture.session,
        &fixture.attachment,
        &gap,
    );
    let Some(request) = request.ok() else { return false };
    match provider.request_resynchronization(request) {
        RepresentationResynchronizationAdmission::Admitted(_) => true,
        RepresentationResynchronizationAdmission::NotAdmitted { .. } => false,
    }
}

fn wrong_resynchronization_update(
    fixture: &support::InteractionFixture,
) -> Option<RepresentationUpdate> {
    let target = crate::buffer::state(1, 1, 1)?;
    let stamp = support::update_stamp(
        fixture,
        4_300,
        identity::RepresentationUpdateCursor::new(4),
        &target,
        &support::projection(fixture, 4),
    )?;
    let lineage = RepresentationUpdateLineage::Resynchronization {
        result:
            reovim_kernel_interface::semantic::identity::ProjectionGeneration::
                new(6),
        resynchronization:
            identity::RepresentationResynchronizationIdentity::new(4_301),
    };
    support::payload_update(fixture, stamp, lineage, support::UPDATE_BYTES)
}

fn resynchronization_is_visible(outcome: InteractionCloseOutcome) -> bool {
    let InteractionCloseOutcome::Incomplete(report) = outcome else {
        return false;
    };
    let Some(totals) = report.resynchronization() else { return false };
    report.execution() == ExecutionOutcome::OwnedNonterminal
        && totals.requests().value() == 1
        && totals.work().value() == 8
        && totals.bytes().value() == 64
}

fn update_then_outcome(
    fixture: &support::InteractionFixture,
    provider: &mut ScriptedInteractionSession,
    request_stamp: &InteractionRequestStamp,
    update_stamp: RepresentationUpdateIdentityStamp,
) -> bool {
    let update_request = RepresentationUpdateRequest::new(
        &fixture.session,
        &fixture.attachment,
        update_stamp.cursor(),
    );
    let Some(update_request) = update_request.ok() else { return false };
    let outcome_request =
        InteractionOutcomeRequest::new(&fixture.session, *request_stamp);
    let Some(outcome_request) = outcome_request.ok() else { return false };
    let port: &mut dyn InteractionSessionPort = provider;
    let update_first = port.take_update(update_request).status()
        == RepresentationUpdateDeliveryStatus::Delivered;
    let acknowledgement = RepresentationUpdateAcknowledgement::new(
        &fixture.session,
        &fixture.attachment,
        update_stamp,
        RepresentationUpdateApplication::Installed,
    );
    let Some(acknowledgement) = acknowledgement.ok() else {
        return false;
    };
    let acknowledged = port.acknowledge_update(acknowledgement)
        == RepresentationUpdateAcknowledgementResult::Accepted;
    let outcome_second = port.take_outcome(outcome_request).status()
        == reovim_kernel_interface::interaction::delivery::intent::
            InteractionOutcomeDeliveryStatus::Delivered;
    let close = InteractionCloseRequest::new(&fixture.session, 4);
    let Some(close) = close.ok() else { return false };
    update_first
        && acknowledged
        && outcome_second
        && port.close(close) == InteractionCloseOutcome::Closed
}

fn missing_outcome(
    provider: &ScriptedInteractionSession,
    request: &InteractionOutcomeRequest,
) -> InteractionOutcomeDelivery {
    if provider
        .delivered_outcome
        .is_some_and(|stamp| stamp.same_request(request.request()))
    {
        return InteractionOutcomeDelivery::unavailable(
            InteractionOutcomeUnavailable::AlreadyDelivered,
        );
    }
    if provider
        .pending
        .as_ref()
        .is_some_and(|intent| intent.stamp().same_request(request.request()))
    {
        return InteractionOutcomeDelivery::unavailable(
            InteractionOutcomeUnavailable::Pending,
        );
    }
    InteractionOutcomeDelivery::unavailable(
        InteractionOutcomeUnavailable::UnknownRequest,
    )
}

fn update_already_delivered(
    provider: &ScriptedInteractionSession,
    cursor: reovim_kernel_interface::interaction::identity::
        RepresentationUpdateCursor,
) -> bool {
    provider
        .delivered_update
        .is_some_and(|stamp| stamp.cursor().value() == cursor.value())
}

fn checked_flag(value: bool) -> Option<CheckedCount> {
    CheckedCount::new(u64::from(value), ZeroPolicy::Allow).ok()
}

const fn attachment_unavailable_cause(
    closed: bool,
) -> RepresentationAttachmentFailure {
    if closed {
        RepresentationAttachmentFailure::Closed
    } else {
        RepresentationAttachmentFailure::NotReady
    }
}
