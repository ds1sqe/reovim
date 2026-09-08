use reovim_kernel_core::BufferCommit;
use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, FenceCause, InstallationOutcome,
};
use reovim_kernel_interface::event::editing::{EditingContext, EditingStage};
use reovim_kernel_interface::interaction::history::{
    InteractionHistoryAction, InteractionHistoryOutcome,
};
use reovim_kernel_interface::interaction::notification;
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeCause;
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::{
    ResourceStateInput, ResourceStateModel, ResourceStatePreparation,
};
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;

use crate::Runtime;
use crate::engine::{buffer_failure, cancellation_failure, state_failure};
use crate::session::RequestBody;

impl<A, M, D, R, S> Runtime<A, M, D, R, S>
where
    A: ResourceAccessPort,
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    pub(crate) fn execute_history(&mut self, session: usize, request: usize) {
        let mut context = self.sessions[session].requests[request].context();
        let result = self.restore(session, request, &mut context);
        self.finish_history(session, request, &result, &context);
    }

    fn restore(
        &mut self,
        session: usize,
        index: usize,
        context: &mut EditingContext,
    ) -> Result<BufferCommit, InteractionOutcomeCause> {
        self.history_current(session, index)?;
        let RequestBody::History { request, .. } =
            &self.sessions[session].requests[index].body
        else {
            unreachable!("History execution keeps the real History request");
        };
        let child = match request.action() {
            InteractionHistoryAction::Undo => None,
            InteractionHistoryAction::Redo { child } => child,
        };
        let buffer =
            self.buffer.as_ref().ok_or(InteractionOutcomeCause::Closed)?;
        let selection = buffer
            .select_history(
                &request.target(),
                request.plan(),
                request.action().direction(),
                child,
            )
            .map_err(buffer_failure)?
            .ok_or(InteractionOutcomeCause::NoHistory)?;
        let current = selection.current().view();
        let retained = selection.retained().view();
        let preparation = ResourceStatePreparation::new(
            self.model.binding(),
            ResourceStateInput::Restore {
                current: &current,
                retained: &retained,
                plan: selection.plan(),
                direction: selection.direction(),
                edge: selection.edge(),
            },
            self.config.buffer.preparation,
            CancellationState::Open,
            DeadlineState::Pending,
        )
        .map_err(state_failure)?;
        let expected = preparation.stamp();
        let candidate = preparation
            .prepare_with(&self.model)
            .map_err(|rejected| state_failure(rejected.cause))?;
        let staged = buffer
            .stage_history(&selection, &expected, candidate)
            .map_err(|rejected| buffer_failure(rejected.cause))?;
        context.parent =
            self.telemetry.emit(EditingStage::Prepared, context).sequence();
        let fence = self
            .history_current(session, index)
            .map_or_else(history_fence, |()| InstallationOutcome::Current);
        let installed = self
            .buffer
            .as_mut()
            .ok_or(InteractionOutcomeCause::Closed)?
            .commit_history(staged, fence)
            .map_err(|rejected| buffer_failure(rejected.cause))?;
        let commit = installed.commit;
        // The old materialization is not a canonical History node. Retire its
        // last owner after Core's mutable installation call has returned.
        drop(installed.retired);
        Ok(commit)
    }

    fn history_current(
        &self,
        session: usize,
        request: usize,
    ) -> Result<(), InteractionOutcomeCause> {
        let session = &self.sessions[session];
        if session.ending() {
            return Err(InteractionOutcomeCause::Closing);
        }
        if let Some(cause) = session.requests[request].cancellation {
            return Err(cancellation_failure(cause));
        }
        if self.model.binding() != self.config.buffer.model
            || self.model.plan_descriptor() != &session.plans
        {
            return Err(InteractionOutcomeCause::Replaced);
        }
        Ok(())
    }

    fn finish_history(
        &mut self,
        session: usize,
        index: usize,
        result: &Result<BufferCommit, InteractionOutcomeCause>,
        context: &EditingContext,
    ) {
        let RequestBody::History { request, outcome: retained } =
            &mut self.sessions[session].requests[index].body
        else {
            unreachable!("terminal ownership retains its original kind");
        };
        let outcome = match result {
            Ok(commit) => InteractionHistoryOutcome::accepted_history(
                *request,
                commit.edge,
                commit.result,
            )
            .expect("Core accepted the selected History edge"),
            Err(cause) => {
                InteractionHistoryOutcome::not_accepted(*request, *cause)
            }
        };
        *retained = Some(outcome);
        let mut context = *context;
        let Some(accepted) = outcome.accepted_state() else {
            context.cause = outcome.cause();
            let _ = self.telemetry.emit(EditingStage::Rejected, &context);
            return;
        };
        context.state = Some(accepted.result());
        context.parent =
            self.telemetry.emit(EditingStage::Committed, &context).sequence();
        let notice =
            notification::BufferChangeNotification::from_history(outcome)
                .expect(
                    "actual accepted traversal preserves the complete request",
                );
        self.publish(&notice, &context);
    }
}

const fn history_fence(cause: InteractionOutcomeCause) -> InstallationOutcome {
    let cause = match cause {
        InteractionOutcomeCause::Cancelled => FenceCause::Cancellation,
        InteractionOutcomeCause::Deadline => FenceCause::Deadline,
        InteractionOutcomeCause::Closing => FenceCause::Closing,
        _ => FenceCause::Replacement,
    };
    InstallationOutcome::Fenced(cause)
}
