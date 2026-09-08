use reovim_kernel_interface::buffer::plan::BufferDeadline;
use reovim_kernel_interface::cancellation::CancellationCause;
use reovim_kernel_interface::event::editing::EditingStage;
use reovim_kernel_interface::interaction::delivery::intent as delivery;
use reovim_kernel_interface::interaction::descriptor;
use reovim_kernel_interface::interaction::history::InteractionHistoryRequest;
use reovim_kernel_interface::interaction::history_delivery::{
    InteractionHistoryAdmission, InteractionHistoryOutcomeDelivery,
    InteractionHistoryPort, history_charge,
};
use reovim_kernel_interface::interaction::identity;
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeCause;

use super::record::history_context;
use super::{Phase, RequestBody, Session};

impl InteractionHistoryPort for Session {
    fn submit_history(
        &mut self,
        request: InteractionHistoryRequest,
    ) -> InteractionHistoryAdmission {
        if let Some(cause) = self.history_failure(&request) {
            let mut context = history_context(&request);
            context.cause = Some(cause);
            let _ = self.telemetry.emit(EditingStage::Rejected, &context);
            return InteractionHistoryAdmission::not_admitted(request, cause);
        }
        if let Some(record) = self
            .requests
            .iter_mut()
            .find(|record| record.identity() == request.request())
        {
            record.delivered = false;
            return InteractionHistoryAdmission::admitted(request);
        }
        let charge = history_charge(&self.binding.session)
            .expect("admission checked the unchanged History reservation");
        self.retain(
            RequestBody::History { request, outcome: None },
            charge,
            &history_context(&request),
        );
        InteractionHistoryAdmission::admitted(request)
    }

    fn take_history_outcome(
        &mut self,
        request: InteractionHistoryRequest,
    ) -> InteractionHistoryOutcomeDelivery {
        use delivery::InteractionOutcomeUnavailable as Missing;
        if request.validate(&self.binding.session, &self.plans).is_err() {
            return InteractionHistoryOutcomeDelivery::unavailable(
                Missing::Replaced,
            );
        }
        if self.phase == Phase::Closed {
            return InteractionHistoryOutcomeDelivery::unavailable(
                Missing::Closed,
            );
        }
        let Some(record) = self
            .requests
            .iter_mut()
            .find(|record| record.matches_history(&request))
        else {
            return InteractionHistoryOutcomeDelivery::unavailable(
                Missing::UnknownRequest,
            );
        };
        if record.delivered {
            return InteractionHistoryOutcomeDelivery::unavailable(
                Missing::AlreadyDelivered,
            );
        }
        let RequestBody::History { outcome, .. } = &record.body else {
            unreachable!("exact History lookup selects only History");
        };
        let Some(outcome) = *outcome else {
            return InteractionHistoryOutcomeDelivery::unavailable(
                Missing::Pending,
            );
        };
        record.delivered = true;
        InteractionHistoryOutcomeDelivery::delivered(outcome)
    }

    fn request_history_cancellation(
        &mut self,
        request: InteractionHistoryRequest,
        cause: CancellationCause,
    ) -> delivery::InteractionCancellationResult {
        use delivery::InteractionCancellationResult as Result;
        if request.validate(&self.binding.session, &self.plans).is_err() {
            return Result::StaleSession;
        }
        if self.binding.session.capabilities().cancellation()
            != descriptor::InteractionCapabilitySupport::Supported
        {
            return Result::Denied;
        }
        if self.phase == Phase::Closed {
            return Result::Closed;
        }
        let Some(record) = self
            .requests
            .iter_mut()
            .find(|record| record.matches_history(&request))
        else {
            return Result::UnknownRequest;
        };
        record.cancel(cause)
    }

    fn reconcile_history_outcome(
        &mut self,
        request: InteractionHistoryRequest,
        _reconcile: identity::InteractionReconcileIdentity,
    ) -> delivery::InteractionReconcileResult<InteractionHistoryRequest> {
        let cause =
            if request.validate(&self.binding.session, &self.plans).is_ok() {
                delivery::InteractionReconcileUnavailable::UnknownRequest
            } else {
                delivery::InteractionReconcileUnavailable::StaleSession
            };
        // Retain until close; no expiry or uncertainty token is issued here.
        delivery::InteractionReconcileResult::unavailable(cause)
    }
}

impl Session {
    fn history_failure(
        &self,
        request: &InteractionHistoryRequest,
    ) -> Option<InteractionOutcomeCause> {
        use InteractionOutcomeCause as Cause;
        if self.phase == Phase::Closed {
            return Some(Cause::Closed);
        }
        if self.phase == Phase::Closing {
            return Some(Cause::Closing);
        }
        if !matches!(self.phase, Phase::Negotiated | Phase::Attached) {
            return Some(Cause::Denied);
        }
        if request.validate(&self.binding.session, &self.plans).is_err() {
            return Some(Cause::Denied);
        }
        if let Some(record) = self
            .requests
            .iter()
            .find(|record| record.identity() == request.request())
        {
            return (!record.matches_history(request))
                .then_some(Cause::InvalidIntent);
        }
        if request.plan().deadline() != BufferDeadline::None {
            return Some(Cause::Unsupported);
        }
        if self.current != Some(request.target()) {
            return Some(Cause::StaleState);
        }
        let Ok(charge) = history_charge(&self.binding.session) else {
            return Some(Cause::Pressure);
        };
        self.check_request_capacity(charge)
    }
}
