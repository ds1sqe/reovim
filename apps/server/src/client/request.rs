use super::{Client, ClientError};
use reovim_kernel_interface::buffer::plan::{
    BufferCancellationIdentity, BufferCorrelationIdentity, BufferDeadline,
    BufferHistoryTransitionIdentity, BufferPlanStamp,
    BufferTransactionIdentity,
};
use reovim_kernel_interface::cancellation::{CancellationState, DeadlineState};
use reovim_kernel_interface::interaction::history::{
    InteractionHistoryAction, InteractionHistoryRequest,
};
use reovim_kernel_interface::interaction::identity::{
    InteractionCancellationIdentity, InteractionCorrelationIdentity,
    InteractionRequestIdentity,
};
use reovim_kernel_interface::interaction::intent::{
    InteractionIntent, InteractionIntentBindingOutcome,
};
use reovim_kernel_interface::interaction::session::{
    InteractionDeadline, InteractionRequestStamp, InteractionSemanticBinding,
};
use reovim_kernel_interface::semantic::identity::{
    SemanticIntentIdentity, SemanticRequestIdentity,
};
use reovim_kernel_interface::semantic::intent::SemanticIntentBindingOutcome;
use reovim_kernel_interface::semantic::stamp::{
    SemanticCallerStamp, SemanticIntentStamp,
};

impl Client {
    /// Creates a semantic request from current observation and caller identity.
    ///
    /// # Errors
    /// Rejects missing correspondence, malformed intent or bounded ID overflow.
    pub fn intent(
        &self,
        request: u128,
        payload: &[u8],
    ) -> Result<InteractionIntent, ClientError> {
        if self.payload.is_none() {
            return Err(ClientError::NotReady);
        }
        let target = self.state.ok_or(ClientError::NotReady)?;
        let caller = SemanticCallerStamp::new(
            self.session.stamp().lifecycle(),
            self.session.stamp().client().role(),
        );
        let plan = self.plan(request)?;
        let semantic = SemanticIntentStamp::new(
            SemanticIntentIdentity::new(plan.transaction().value()),
            caller,
            SemanticRequestIdentity::new(request),
            target,
            plan,
            self.projection,
            self.attachment.coordinate(),
        );
        let prepared = semantic
            .prepare_intent(
                &self.policy,
                self.policy.intent().selected_stamp(),
                1,
                0,
                u64::try_from(payload.len())
                    .map_err(|_| ClientError::Intent)?,
            )
            .map_err(|_| ClientError::Intent)?;
        let SemanticIntentBindingOutcome::Bound(semantic) =
            prepared.bind(&self.policy, payload.into())
        else {
            return Err(ClientError::Intent);
        };
        let binding = InteractionSemanticBinding::new(
            semantic.stamp().request(),
            semantic.stamp().intent(),
            plan.cancellation(),
            plan.correlation(),
            plan.deadline(),
            plan.transaction(),
            plan.history_transition(),
        );
        let stamp = InteractionRequestStamp::new(
            self.session.stamp().handle(),
            self.attachment.handle(),
            InteractionRequestIdentity::new(request),
            InteractionCancellationIdentity::new(request),
            InteractionCorrelationIdentity::new(request),
            InteractionDeadline::None,
            binding,
        )
        .map_err(|_| ClientError::Binding)?;
        let charge = self
            .session
            .bounds()
            .requests()
            .prepare_intent(semantic.shape().bytes().value())
            .map_err(|_| ClientError::Intent)?;
        let InteractionIntentBindingOutcome::Bound(intent) =
            InteractionIntent::bind(
                &self.session,
                &self.attachment,
                stamp,
                charge,
                semantic,
                CancellationState::Open,
                DeadlineState::Pending,
            )
        else {
            return Err(ClientError::Intent);
        };
        Ok(intent)
    }

    /// Requests shared History from an observed Buffer, without semantic data.
    /// A new request shares the exact identity namespace used by edits.
    ///
    /// # Errors
    /// Returns missing observation, stale binding or identity exhaustion.
    pub fn history(
        &self,
        request: u128,
        action: InteractionHistoryAction,
    ) -> Result<InteractionHistoryRequest, ClientError> {
        let target = self.state.ok_or(ClientError::NotReady)?;
        InteractionHistoryRequest::new(
            &self.session,
            &self.plans,
            InteractionRequestIdentity::new(request),
            self.plan(request)?,
            target,
            action,
        )
        .map_err(|_| ClientError::Binding)
    }

    fn plan(&self, request: u128) -> Result<BufferPlanStamp, ClientError> {
        let local =
            u64::try_from(request).map_err(|_| ClientError::Identity)?;
        let namespace = u64::try_from(self.session.stamp().session().value())
            .map_err(|_| ClientError::Identity)?;
        let identity = (u128::from(namespace) << 64) | u128::from(local);
        Ok(BufferPlanStamp::new(
            self.plans.owner(),
            BufferTransactionIdentity::new(identity),
            BufferHistoryTransitionIdentity::new(identity),
            BufferCancellationIdentity::new(identity),
            BufferCorrelationIdentity::new(identity),
            BufferDeadline::None,
        ))
    }
}
