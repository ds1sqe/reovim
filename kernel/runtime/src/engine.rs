use reovim_kernel_core::{Buffer, BufferError, StagedEdit};
use reovim_kernel_interface::bound::CheckedCount;
use reovim_kernel_interface::buffer::plan::BufferTransactionPlan;
use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, FenceCause, InstallationOutcome,
};
use reovim_kernel_interface::event::editing::{EditingContext, EditingStage};
use reovim_kernel_interface::interaction::intent::InteractionIntent;
use reovim_kernel_interface::interaction::notification;
use reovim_kernel_interface::interaction::outcome::{
    InteractionOutcome, InteractionOutcomeCause,
};
use reovim_kernel_interface::lifecycle::LifecycleState;
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::{
    ResourceStateError, ResourceStateInput, ResourceStateLimits,
    ResourceStateModel, ResourceStatePreparation,
};
use reovim_kernel_interface::semantic::bounds::SemanticPolicyCallCharge;
use reovim_kernel_interface::semantic::descriptor::SemanticPolicyCallDescriptor;
use reovim_kernel_interface::semantic::domain::{
    DomainPolicyPort, DomainValidationRequest,
};
use reovim_kernel_interface::semantic::representation::{
    RepresentationLoweringRequest, RepresentationLoweringStatus,
    RepresentationPolicyPort,
};
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;
use reovim_kernel_interface::semantic::state::SemanticPolicyInput;

use crate::session::RequestBody;
use crate::{Runtime, RuntimeError};

impl<A, M, D, R, S> Runtime<A, M, D, R, S>
where
    A: ResourceAccessPort,
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    /// Drives at most the supplied number of open/edit/snapshot work items.
    /// Retrieval methods never execute providers. Trusted provider panics are
    /// process-fatal; this portable composition promises no unwind recovery.
    ///
    /// # Errors
    /// Rejects zero/excess work or invalid provider output during opening.
    pub fn progress(
        &mut self,
        work: CheckedCount,
    ) -> Result<u64, RuntimeError> {
        if work.value() == 0 || work.value() > self.config.limits.work.value() {
            return Err(RuntimeError::Capacity);
        }
        if !matches!(
            self.phase,
            LifecycleState::Ready | LifecycleState::Closing
        ) {
            return Ok(0);
        }
        let mut completed = 0;
        while completed < work.value() && self.progress_one()? {
            completed += 1;
        }
        Ok(completed)
    }

    fn progress_one(&mut self) -> Result<bool, RuntimeError> {
        if self.progress_open()? {
            return Ok(true);
        }
        if self.phase != LifecycleState::Ready {
            return Ok(false);
        }
        let count = self.sessions.len();
        let pending = (0..count).find_map(|offset| {
            let index = (self.next_session + offset) % count;
            self.sessions[index]
                .requests
                .iter()
                .position(super::session::RequestRecord::pending)
                .map(|request| (index, request))
        });
        let Some((index, request)) = pending else {
            return Ok(false);
        };
        self.execute(index, request);
        self.next_session = (index + 1) % count;
        Ok(true)
    }

    fn execute(&mut self, session: usize, request: usize) {
        match &self.sessions[session].requests[request].body {
            RequestBody::Edit { .. } => self.execute_edit(session, request),
            RequestBody::History { .. } => {
                self.execute_history(session, request);
            }
        }
    }

    fn execute_edit(&mut self, session: usize, request: usize) {
        let record = &self.sessions[session].requests[request];
        let mut context = record.context();
        let result = self.prepare(session, request);
        if result.is_ok() {
            context.parent = self
                .telemetry
                .emit(EditingStage::Prepared, &context)
                .sequence();
        }
        let fence = if self.providers_current() {
            InstallationOutcome::Current
        } else {
            InstallationOutcome::Fenced(FenceCause::Replacement)
        };
        let accepted = result.and_then(|staged| {
            let installed = self
                .buffer
                .as_mut()
                .ok_or(InteractionOutcomeCause::Closed)?
                .commit_edit(staged, fence)
                .map_err(|rejected| buffer_failure(rejected.cause))?;
            let commit = installed.commit;
            drop(installed.retired);
            Ok(commit)
        });
        self.finish_edit(
            session,
            request,
            &accepted.map(|value| value.result),
            &context,
        );
    }

    fn prepare(
        &self,
        session: usize,
        request: usize,
    ) -> Result<StagedEdit<M::State>, InteractionOutcomeCause> {
        let session = &self.sessions[session];
        let record = &session.requests[request];
        if !self.providers_current() {
            return Err(InteractionOutcomeCause::Replaced);
        }
        if session.ending() {
            return Err(InteractionOutcomeCause::Closing);
        }
        if let Some(cause) = record.cancellation {
            return Err(cancellation_failure(cause));
        }
        let buffer =
            self.buffer.as_ref().ok_or(InteractionOutcomeCause::Closed)?;
        prepare_edit(
            buffer,
            &self.model,
            &self.domain,
            &self.representation,
            record.edit().expect("selected an edit record"),
            self.config.buffer.preparation,
        )
    }

    fn finish_edit(
        &mut self,
        session: usize,
        request: usize,
        result: &Result<
            reovim_kernel_interface::buffer::BufferStateStamp,
            InteractionOutcomeCause,
        >,
        context: &EditingContext,
    ) {
        let record = &mut self.sessions[session].requests[request];
        let RequestBody::Edit { intent, outcome: retained } = &mut record.body
        else {
            unreachable!("edit execution retains its request kind");
        };
        let outcome = match *result {
            Ok(state) => InteractionOutcome::accepted(
                intent.stamp(),
                intent.semantic().stamp(),
                state,
            )
            .expect("Core result preserves the admitted plan binding"),
            Err(cause) => {
                InteractionOutcome::not_accepted(intent.stamp(), cause)
            }
        };
        retained.push(outcome);
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
            notification::BufferChangeNotification::from_edit(outcome, intent)
                .expect("retained request and Core result are unchanged");
        self.publish(&notice, &context);
    }
}

pub const fn cancellation_failure(
    cause: reovim_kernel_interface::cancellation::CancellationCause,
) -> InteractionOutcomeCause {
    use reovim_kernel_interface::cancellation::CancellationCause as Cause;
    match cause {
        Cause::ExplicitRequest => InteractionOutcomeCause::Cancelled,
        Cause::DeadlineReached => InteractionOutcomeCause::Deadline,
        Cause::Replacement => InteractionOutcomeCause::Replaced,
        Cause::OwnerClosing => InteractionOutcomeCause::Closing,
    }
}

fn prepare_edit<M, D, R>(
    buffer: &Buffer<M::State>,
    model: &M,
    domain: &D,
    representation: &R,
    intent: &InteractionIntent,
    limits: ResourceStateLimits,
) -> Result<StagedEdit<M::State>, InteractionOutcomeCause>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
{
    let lease = buffer.observe();
    let state = lease.view();
    if !state.stamp.same_state(*intent.semantic().stamp().target()) {
        return Err(InteractionOutcomeCause::StaleState);
    }
    let input =
        SemanticPolicyInput { intent: intent.semantic(), state: &state };
    let plan = lower(model, domain, representation, input)?;
    let preparation = ResourceStatePreparation::new(
        model.binding(),
        ResourceStateInput::Edit {
            observed: &state,
            descriptor: model.plan_descriptor(),
            plan: &plan,
            observations: &[],
        },
        limits,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .map_err(state_failure)?;
    let expected = preparation.stamp();
    let candidate = preparation
        .prepare_with(model)
        .map_err(|rejected| state_failure(rejected.cause))?;
    buffer
        .stage_edit(&expected, candidate)
        .map_err(|rejected| buffer_failure(rejected.cause))
}

fn lower<M, D, R>(
    model: &M,
    domain: &D,
    representation: &R,
    input: SemanticPolicyInput<'_, M::State>,
) -> Result<BufferTransactionPlan, InteractionOutcomeCause>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
{
    let validation = DomainValidationRequest::new(
        domain.descriptor(),
        representation.descriptor(),
        input,
        charge(domain.descriptor().validation())?,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .map_err(|_| InteractionOutcomeCause::InvalidIntent)?;
    let candidate = validation
        .invoke(domain)
        .map_err(|_| InteractionOutcomeCause::ProviderContractViolation)?;
    let receipt = validation
        .finish(domain.descriptor(), candidate, InstallationOutcome::Current)
        .receipt()
        .ok_or(InteractionOutcomeCause::PolicyFailure)?;
    let lowering = RepresentationLoweringRequest::new(
        representation.descriptor(),
        input,
        &receipt,
        model.plan_descriptor(),
        charge(representation.descriptor().lowering())?,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .map_err(|_| InteractionOutcomeCause::InvalidIntent)?;
    let candidate = lowering
        .invoke(representation)
        .map_err(|_| InteractionOutcomeCause::ProviderContractViolation)?;
    let outcome = candidate.finish(
        lowering,
        representation.descriptor(),
        InstallationOutcome::Current,
    );
    if outcome.status() != RepresentationLoweringStatus::Lowered {
        return Err(InteractionOutcomeCause::PolicyFailure);
    }
    outcome
        .into_plan()
        .ok_or(InteractionOutcomeCause::ProviderContractViolation)
}

pub fn charge(
    descriptor: &SemanticPolicyCallDescriptor,
) -> Result<SemanticPolicyCallCharge, InteractionOutcomeCause> {
    let bounds = descriptor.bounds();
    bounds
        .prepare(
            bounds.work().per_object().value(),
            bounds.temporary_bytes().per_object().value(),
            bounds.result_bytes().per_object().value(),
            1,
        )
        .map_err(|_| InteractionOutcomeCause::Pressure)
}

pub const fn buffer_failure(cause: BufferError) -> InteractionOutcomeCause {
    match cause {
        BufferError::Capacity | BufferError::IdentityExhausted => {
            InteractionOutcomeCause::Pressure
        }
        BufferError::State(cause) => state_failure(cause),
        BufferError::InvalidConfiguration => {
            InteractionOutcomeCause::ProviderContractViolation
        }
    }
}

pub const fn state_failure(
    cause: ResourceStateError,
) -> InteractionOutcomeCause {
    use ResourceStateError as Error;
    match cause {
        Error::Fenced(cause) => fence_failure(cause),
        Error::Pressure => InteractionOutcomeCause::Pressure,
        Error::PreconditionFailed => InteractionOutcomeCause::Conflict,
        Error::ModelMismatch => InteractionOutcomeCause::Replaced,
        Error::StateMismatch | Error::PreparationMismatch => {
            InteractionOutcomeCause::StaleState
        }
        Error::Unavailable | Error::ProviderFailure => {
            InteractionOutcomeCause::PolicyFailure
        }
        _ => InteractionOutcomeCause::ProviderContractViolation,
    }
}

const fn fence_failure(cause: FenceCause) -> InteractionOutcomeCause {
    match cause {
        FenceCause::Cancellation => InteractionOutcomeCause::Cancelled,
        FenceCause::Deadline => InteractionOutcomeCause::Deadline,
        FenceCause::CallerTimeout => InteractionOutcomeCause::CallerTimeout,
        FenceCause::Replacement => InteractionOutcomeCause::Replaced,
        FenceCause::Closing => InteractionOutcomeCause::Closing,
        FenceCause::Closed => InteractionOutcomeCause::Closed,
    }
}

pub const fn context(intent: &InteractionIntent) -> EditingContext {
    EditingContext {
        session: Some(intent.stamp().session()),
        request: Some(intent.stamp().request()),
        correlation: Some(intent.semantic().stamp().plan().correlation()),
        state: Some(*intent.semantic().stamp().target()),
        parent: None,
        cause: None,
    }
}
