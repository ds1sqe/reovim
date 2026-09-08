use alloc::rc::Rc;

use reovim_kernel_interface::buffer::{
    BufferHistoryDirection, BufferHistoryEdge, BufferStateStamp,
};
use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationOutcome, ExecutionOutcome,
    InstallationOutcome,
};
use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionCancellationResult, InteractionOutcomeUnavailable,
    InteractionReconcileResult, InteractionReconcileUnavailable,
};
use reovim_kernel_interface::interaction::history::{
    InteractionHistoryAction, InteractionHistoryOutcome,
    InteractionHistoryRequest,
};
use reovim_kernel_interface::interaction::history_delivery::{
    InteractionHistoryAdmission, InteractionHistoryOutcomeDelivery,
    InteractionHistoryPort, history_charge,
};
use reovim_kernel_interface::interaction::identity::{
    InteractionReconcileIdentity, InteractionRequestIdentity,
};
use reovim_kernel_interface::interaction::{
    notification::BufferChangeNotification, outcome::InteractionOutcomeCause,
};
use reovim_kernel_interface::resource::state::{
    ResourceStateInput, ResourceStateTarget,
};

use super::{Session, completion::next_stamp};
use crate::first_edit::{Family, native::support as native};

impl<Model: Family> InteractionHistoryPort for Session<Model>
where
    Model::State: Clone,
{
    fn submit_history(
        &mut self,
        request: InteractionHistoryRequest,
    ) -> InteractionHistoryAdmission {
        if let Some(cause) = self.history_failure(&request) {
            return InteractionHistoryAdmission::not_admitted(request, cause);
        }
        if self.history_pending == Some(request) {
            return InteractionHistoryAdmission::admitted(request);
        }
        if let Some(outcome) = self.retained_history(&request) {
            self.history_outcome = Some(outcome);
        } else {
            self.history_pending = Some(request);
            self.cancelled = false;
        }
        InteractionHistoryAdmission::admitted(request)
    }

    fn take_history_outcome(
        &mut self,
        request: InteractionHistoryRequest,
    ) -> InteractionHistoryOutcomeDelivery {
        if !request.session().matches(self.fixture.session.stamp()) {
            return InteractionHistoryOutcomeDelivery::unavailable(
                InteractionOutcomeUnavailable::Replaced,
            );
        }
        if self
            .history_outcome
            .is_some_and(|value| value.request().same_request(request))
        {
            return InteractionHistoryOutcomeDelivery::delivered(
                self.history_outcome.take().unwrap(),
            );
        }
        let cause = if self.retained_history(&request).is_some() {
            InteractionOutcomeUnavailable::AlreadyDelivered
        } else if self.history_pending == Some(request) {
            InteractionOutcomeUnavailable::Pending
        } else {
            InteractionOutcomeUnavailable::UnknownRequest
        };
        InteractionHistoryOutcomeDelivery::unavailable(cause)
    }

    fn request_history_cancellation(
        &mut self,
        request: InteractionHistoryRequest,
        cause: CancellationCause,
    ) -> InteractionCancellationResult {
        if !request.session().matches(self.fixture.session.stamp()) {
            return InteractionCancellationResult::StaleSession;
        }
        if self.retained_history(&request).is_some() {
            return InteractionCancellationResult::AlreadyTerminal;
        }
        if self.history_pending != Some(request) {
            return InteractionCancellationResult::UnknownRequest;
        }
        let result =
            CancellationOutcome::new(cause, ExecutionOutcome::OwnedNonterminal);
        if self.cancelled {
            return InteractionCancellationResult::AlreadyRequested(result);
        }
        self.cancelled = true;
        InteractionCancellationResult::Requested(result)
    }

    fn reconcile_history_outcome(
        &mut self,
        request: InteractionHistoryRequest,
        _: InteractionReconcileIdentity,
    ) -> InteractionReconcileResult<InteractionHistoryRequest> {
        if !request.session().matches(self.fixture.session.stamp()) {
            return InteractionReconcileResult::unavailable(
                InteractionReconcileUnavailable::StaleSession,
            );
        }
        let cause = if self.history_pending == Some(request) {
            InteractionReconcileUnavailable::Pending
        } else {
            InteractionReconcileUnavailable::UnknownRequest
        };
        InteractionReconcileResult::unavailable(cause)
    }
}

impl<Model: Family> Session<Model>
where
    Model::State: Clone,
{
    pub fn history_request(
        &self,
        id: u128,
        action: InteractionHistoryAction,
    ) -> InteractionHistoryRequest {
        InteractionHistoryRequest::new(
            &self.fixture.session,
            &self.fixture.semantic.buffer,
            InteractionRequestIdentity::new(id),
            native::plan_stamp(id + 3),
            self.current_stamp(),
            action,
        )
        .unwrap()
    }

    fn retained_history(
        &self,
        request: &InteractionHistoryRequest,
    ) -> Option<InteractionHistoryOutcome> {
        self.history_completed
            .into_iter()
            .chain(self.history_older)
            .find(|outcome| outcome.request().same_request(*request))
    }

    fn history_failure(
        &self,
        request: &InteractionHistoryRequest,
    ) -> Option<InteractionOutcomeCause> {
        if self.closed {
            return Some(InteractionOutcomeCause::Closed);
        }
        if request
            .validate(&self.fixture.session, &self.fixture.semantic.buffer)
            .is_err()
        {
            return Some(InteractionOutcomeCause::Replaced);
        }
        if !self.ready() || !self.is_negotiated() {
            return Some(InteractionOutcomeCause::Denied);
        }
        if self
            .pending
            .iter()
            .chain(self.completed.as_ref().map(|entry| &entry.0))
            .any(|intent| intent.stamp().request() == request.request())
        {
            return Some(InteractionOutcomeCause::InvalidIntent);
        }
        if self
            .history_outcome
            .is_some_and(|outcome| !outcome.request().same_request(*request))
        {
            return Some(InteractionOutcomeCause::Pressure);
        }
        if self.history_pending == Some(*request)
            || self.retained_history(request).is_some()
        {
            return None;
        }
        if self
            .history_pending
            .into_iter()
            .chain(
                self.history_completed.map(InteractionHistoryOutcome::request),
            )
            .chain(self.history_older.map(InteractionHistoryOutcome::request))
            .any(|old| old.request() == request.request())
        {
            return Some(InteractionOutcomeCause::InvalidIntent);
        }
        if self.pending.is_some()
            || self.history_pending.is_some()
            || self.history_outcome.is_some()
            || self.outcome.is_some()
            || self.history_older.is_some()
        {
            return Some(InteractionOutcomeCause::Pressure);
        }
        if request.target() != self.current_stamp() {
            return Some(InteractionOutcomeCause::StaleState);
        }
        let Ok(charge) = history_charge(&self.fixture.session) else {
            return Some(InteractionOutcomeCause::Pressure);
        };
        let retained = u64::from(self.completed.is_some())
            + u64::from(self.history_completed.is_some())
            + u64::from(self.history_older.is_some());
        let Some(retained_bytes) =
            retained.checked_mul(charge.outcome_bytes().value())
        else {
            return Some(InteractionOutcomeCause::Pressure);
        };
        self.fixture
            .session
            .bounds()
            .requests()
            .admit_pending(0, 0, retained, retained_bytes, charge)
            .err()
            .map(|_| InteractionOutcomeCause::Pressure)
    }

    pub fn complete_history(
        &mut self,
        result: &BufferStateStamp,
        edge: BufferHistoryEdge,
    ) -> Result<BufferChangeNotification, InteractionOutcomeCause> {
        let request = self.history_pending.take().expect("admitted traversal");
        let accepted =
            InteractionHistoryOutcome::accepted_history(request, edge, *result);
        let prepared = accepted
            .map_err(|_| InteractionOutcomeCause::NoHistory)
            .and_then(|_| self.restore_history(&request, result, edge));
        let outcome = match prepared {
            Ok(()) => accepted.expect("validated tree edge"),
            Err(cause) => {
                InteractionHistoryOutcome::not_accepted(request, cause)
            }
        };
        self.history_older = self.history_completed.replace(outcome);
        self.history_outcome = Some(outcome);
        self.cancelled = false;
        BufferChangeNotification::from_history(outcome).map_err(|_| {
            outcome.cause().unwrap_or(InteractionOutcomeCause::InvariantFailure)
        })
    }

    fn restore_history(
        &self,
        request: &InteractionHistoryRequest,
        result: &BufferStateStamp,
        edge: BufferHistoryEdge,
    ) -> Result<(), InteractionOutcomeCause> {
        if self.cancelled || self.closed {
            return Err(InteractionOutcomeCause::Cancelled);
        }
        if edge != native::edge(900, 7) {
            return Err(InteractionOutcomeCause::NoHistory);
        }
        let (model, current, retained, stamp) = {
            let shared = self.shared.borrow();
            let retained = match request.action().direction() {
                BufferHistoryDirection::Undo => &shared.initial,
                BufferHistoryDirection::Redo => &shared.edited,
            };
            (
                Rc::clone(&shared.model),
                shared.current.clone().unwrap(),
                retained.clone().ok_or(InteractionOutcomeCause::NoHistory)?,
                shared.stamp,
            )
        };
        if stamp != request.target() {
            return Err(InteractionOutcomeCause::StaleState);
        }
        let target_node = edge.to(request.action().direction());
        let retained_version = match request.action().direction() {
            BufferHistoryDirection::Undo => 0,
            BufferHistoryDirection::Redo => 1,
        };
        let retained_stamp =
            next_stamp(&stamp, retained_version, target_node.value());
        let current = native::view(&model.binding(), &stamp, &current);
        let retained =
            native::view(&model.binding(), &retained_stamp, &retained);
        let preparation = native::request(
            &model.binding(),
            ResourceStateInput::Restore {
                current: &current,
                retained: &retained,
                plan: request.plan(),
                direction: request.action().direction(),
                edge,
            },
        );
        let candidate = preparation
            .prepare_with(model.as_ref())
            .map_err(|_| InteractionOutcomeCause::NoHistory)?;
        let binding = model.binding();
        let retired = {
            let mut shared = self.shared.borrow_mut();
            candidate
                .check_admission(
                    binding,
                    preparation.stamp(),
                    ResourceStateTarget::Buffer(shared.stamp),
                    preparation.limits(),
                    InstallationOutcome::Current,
                )
                .map_err(|_| InteractionOutcomeCause::StaleState)?;
            let old = shared.current.replace(candidate.into_output().state);
            shared.stamp = *result;
            old
        };
        drop(retired);
        Ok(())
    }
}
