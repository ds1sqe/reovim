use reovim_kernel_interface::bound::{CheckedCount, ZeroPolicy};
use reovim_kernel_interface::cancellation::ExecutionOutcome;
use reovim_kernel_interface::event::editing::{EditingContext, EditingStage};
use reovim_kernel_interface::interaction::delivery::{
    session as delivery, update,
};
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeCause;

use super::{CreditClass, Phase, Session};

impl Session {
    pub(super) fn close_session(
        &mut self,
        request: delivery::InteractionCloseRequest,
    ) -> delivery::InteractionCloseOutcome {
        if !request.session().matches(self.binding.session.stamp()) {
            return delivery::InteractionCloseOutcome::StaleSession;
        }
        if self.phase == Phase::Closed {
            return delivery::InteractionCloseOutcome::Closed;
        }
        self.phase = Phase::Closing;
        if self.recovery_pending() {
            self.recovery_failed(
                update::RepresentationResynchronizationFailure::Closing,
            );
        }
        let _ = self.telemetry.emit(
            EditingStage::Closing,
            &EditingContext {
                session: Some(request.session()),
                state: self.current,
                ..EditingContext::default()
            },
        );
        for record in
            self.requests.iter_mut().filter(|r| r.pending()).take(
                usize::try_from(request.work().value()).unwrap_or(usize::MAX),
            )
        {
            record.reject(InteractionOutcomeCause::Closing);
        }
        if self.requests.iter().any(|r| !r.delivered) || self.recovery.is_some()
        {
            return self.incomplete();
        }
        // This full-snapshot profile explicitly discards projection delivery
        // on detach; terminal editing outcomes must transfer first.
        self.initial = None;
        self.updates.clear();
        self.issued.clear();
        self.control = None;
        self.gap = None;
        self.notices.clear();
        self.notice_gap = None;
        self.subscription_closed = true;
        self.requests.clear();
        self.phase = Phase::Closed;
        let _ = self.telemetry.emit(
            EditingStage::Closed,
            &EditingContext {
                session: Some(request.session()),
                state: self.current,
                ..EditingContext::default()
            },
        );
        delivery::InteractionCloseOutcome::Closed
    }

    fn incomplete(&self) -> delivery::InteractionCloseOutcome {
        let pending =
            self.requests.iter().filter(|r| r.pending()).count() as u64;
        let retained = self
            .requests
            .iter()
            .filter(|r| !r.pending() && !r.delivered)
            .count() as u64;
        let state = delivery::InteractionCloseRetainedState::new(
            count(pending),
            count(retained),
            count(self.updates.len() as u64),
            count(
                self.issued
                    .iter()
                    .filter(|issued| issued.class == CreditClass::Data)
                    .count() as u64,
            ),
            count(0),
            self.recovery.as_ref().map(|work| work.totals),
        );
        let execution = if pending == 0 {
            ExecutionOutcome::Stopped
        } else {
            ExecutionOutcome::OwnedNonterminal
        };
        delivery::InteractionCloseOutcome::Incomplete(
            delivery::InteractionIncompleteClose::new(
                self.binding.session.bounds(),
                execution,
                state,
            )
            .expect("actual retained reservations stay within Session bounds"),
        )
    }
}

fn count(value: u64) -> CheckedCount {
    CheckedCount::new(value, ZeroPolicy::Allow).expect("count permits zero")
}
