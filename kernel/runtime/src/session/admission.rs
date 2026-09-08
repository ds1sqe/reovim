use reovim_kernel_interface::buffer::plan::BufferDeadline;
use reovim_kernel_interface::cancellation::{
    CancellationOutcome, ExecutionOutcome,
};
use reovim_kernel_interface::interaction::delivery::intent as delivery;
use reovim_kernel_interface::interaction::intent::InteractionIntent;
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeCause;

use super::{Phase, RequestRecord, Session};

impl Session {
    pub(super) fn admit(
        &mut self,
        intent: InteractionIntent,
    ) -> delivery::InteractionIntentAdmission {
        if let Some(cause) = self.admission_failure(&intent) {
            self.record_refusal(&intent, cause);
            return delivery::InteractionIntentAdmission::not_admitted(
                intent, cause,
            );
        }
        let admitted = delivery::InteractionIntentAdmission::admitted(&intent);
        if let Some(record) = self.requests.iter_mut().find(|record| {
            record.intent.stamp().request() == intent.stamp().request()
        }) {
            record.delivered = false;
            return admitted;
        }
        self.request_bytes += intent.charge().intent_bytes().value();
        let parent = self
            .telemetry
            .emit(
                reovim_kernel_interface::event::editing::EditingStage::Admitted,
                &crate::engine::context(&intent),
            )
            .sequence();
        self.requests.push(RequestRecord {
            intent,
            outcome: None,
            delivered: false,
            cancellation: None,
            parent,
        });
        admitted
    }

    fn record_refusal(
        &self,
        intent: &InteractionIntent,
        cause: InteractionOutcomeCause,
    ) {
        let mut context = crate::engine::context(intent);
        context.cause = Some(cause);
        let _ = self.telemetry.emit(
            reovim_kernel_interface::event::editing::EditingStage::Rejected,
            &context,
        );
    }

    fn admission_failure(
        &self,
        intent: &InteractionIntent,
    ) -> Option<InteractionOutcomeCause> {
        use InteractionOutcomeCause as Cause;
        if self.phase != Phase::Attached {
            return Some(Cause::Denied);
        }
        if !intent.stamp().session().matches(self.binding.session.stamp())
            || intent.stamp().attachment() != self.binding.attachment.handle()
        {
            return Some(Cause::Replaced);
        }
        if let Some(cause) = self.live_claim_failure(intent) {
            return Some(cause);
        }
        if let Some(record) = self.requests.iter().find(|record| {
            record.intent.stamp().request() == intent.stamp().request()
        }) {
            return (!record.intent.same_request(intent))
                .then_some(Cause::InvalidIntent);
        }
        if self.requests.iter().any(|record| {
            record.intent.semantic().stamp().intent()
                == intent.semantic().stamp().intent()
        }) {
            return Some(Cause::InvalidIntent);
        }
        if intent.semantic().stamp().plan().deadline() != BufferDeadline::None {
            // No deadline governor is selected by this initial profile.
            return Some(Cause::Unsupported);
        }
        if self.invalidated {
            return Some(Cause::StaleState);
        }
        if !self.current.is_some_and(|state| {
            state.same_state(*intent.semantic().stamp().target())
        }) || !self
            .projection
            .same_state(*intent.semantic().stamp().projection())
        {
            return Some(Cause::StaleState);
        }
        self.check_request_capacity(intent)
    }

    fn live_claim_failure(
        &self,
        intent: &InteractionIntent,
    ) -> Option<InteractionOutcomeCause> {
        use InteractionOutcomeCause as Cause;
        let session = &self.binding.session;
        let semantic = intent.semantic().stamp();
        if semantic.caller().authority() != session.stamp().lifecycle()
            || semantic.caller().role() != session.stamp().client().role()
            || !session.scope().contains(*semantic.target())
        {
            return Some(Cause::Denied);
        }
        if semantic.coordinate() != self.binding.attachment.coordinate()
            || semantic.projection().representation()
                != self.binding.attachment.representation()
            || !intent.stamp().semantic().matches_intent(semantic)
        {
            return Some(Cause::InvalidIntent);
        }
        if session.bounds().requests().validate_charge(intent.charge()).is_err()
            || intent.charge().intent_bytes()
                != intent.semantic().shape().bytes()
        {
            return Some(Cause::Pressure);
        }
        None
    }

    fn check_request_capacity(
        &self,
        intent: &InteractionIntent,
    ) -> Option<InteractionOutcomeCause> {
        let bounds = self.binding.session.bounds().requests();
        let charge = intent.charge();
        let bytes =
            self.request_bytes.checked_add(charge.intent_bytes().value());
        let pending = self.requests.iter().filter(|r| r.outcome.is_none());
        let pending_count = pending.clone().count() as u64;
        let pending_bytes =
            pending.map(|r| r.intent.charge().intent_bytes().value()).sum();
        let retained = self.requests.len() as u64 - pending_count;
        let outcome_bytes =
            retained.checked_mul(charge.outcome_bytes().value());
        if self.requests.len() == self.request_capacity
            || bytes.is_none_or(|value| {
                value > bounds.pending_intent_bytes().aggregate().value()
            })
            || outcome_bytes.is_none_or(|value| {
                bounds
                    .admit_pending(
                        pending_count,
                        pending_bytes,
                        retained,
                        value,
                        charge,
                    )
                    .is_err()
            })
        {
            return Some(InteractionOutcomeCause::Pressure);
        }
        None
    }

    pub(super) fn cancel(
        &mut self,
        request: &delivery::InteractionCancellationRequest,
    ) -> delivery::InteractionCancellationResult {
        use delivery::InteractionCancellationResult as Result;
        use reovim_kernel_interface::interaction::descriptor;
        if !request.request().session().matches(self.binding.session.stamp()) {
            return Result::StaleSession;
        }
        if self.binding.session.capabilities().cancellation()
            != descriptor::InteractionCapabilitySupport::Supported
        {
            return Result::Denied;
        }
        let Some(record) = self.requests.iter_mut().find(|record| {
            record.intent.stamp().same_request(request.request())
        }) else {
            return Result::UnknownRequest;
        };
        if record.outcome.is_some() {
            return Result::AlreadyTerminal;
        }
        let outcome = CancellationOutcome::new(
            request.cause(),
            ExecutionOutcome::OwnedNonterminal,
        );
        if let Some(cause) = record.cancellation {
            return Result::AlreadyRequested(CancellationOutcome::new(
                cause,
                ExecutionOutcome::OwnedNonterminal,
            ));
        }
        record.cancellation = Some(request.cause());
        Result::Requested(outcome)
    }
}
