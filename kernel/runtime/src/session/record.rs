use alloc::vec::Vec;
use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationOutcome, ExecutionOutcome,
};
use reovim_kernel_interface::event::EventSequence;
use reovim_kernel_interface::event::editing::EditingContext;
use reovim_kernel_interface::interaction::bounds::InteractionIntentCharge;
use reovim_kernel_interface::interaction::delivery::intent;
use reovim_kernel_interface::interaction::history::{
    InteractionHistoryOutcome, InteractionHistoryRequest,
};
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_kernel_interface::interaction::intent::InteractionIntent;
use reovim_kernel_interface::interaction::outcome::{
    InteractionOutcome, InteractionOutcomeCause,
};
use reovim_kernel_interface::interaction::session::InteractionRequestStamp;

/// One request namespace and one terminal reservation, with kind-bound data.
pub enum RequestBody {
    Edit {
        intent: InteractionIntent,
        // One fallibly reserved terminal slot, owned until Session close.
        // Pending is empty; completion performs one allocation-free push.
        outcome: Vec<InteractionOutcome>,
    },
    History {
        request: InteractionHistoryRequest,
        outcome: Option<InteractionHistoryOutcome>,
    },
}

pub struct RequestRecord {
    pub body: RequestBody,
    pub charge: InteractionIntentCharge,
    pub delivered: bool,
    pub cancellation: Option<CancellationCause>,
    pub parent: Option<EventSequence>,
}

impl RequestRecord {
    pub const fn identity(&self) -> InteractionRequestIdentity {
        match &self.body {
            RequestBody::Edit { intent, .. } => intent.stamp().request(),
            RequestBody::History { request, .. } => request.request(),
        }
    }

    pub const fn pending(&self) -> bool {
        match &self.body {
            RequestBody::Edit { outcome, .. } => outcome.is_empty(),
            RequestBody::History { outcome, .. } => outcome.is_none(),
        }
    }

    pub const fn edit(&self) -> Option<&InteractionIntent> {
        match &self.body {
            RequestBody::Edit { intent, .. } => Some(intent),
            RequestBody::History { .. } => None,
        }
    }

    pub const fn matches_edit(
        &self,
        requested: &InteractionRequestStamp,
    ) -> bool {
        match &self.body {
            RequestBody::Edit { intent, .. } => {
                intent.stamp().same_request(*requested)
            }
            RequestBody::History { .. } => false,
        }
    }

    pub const fn matches_history(
        &self,
        requested: &InteractionHistoryRequest,
    ) -> bool {
        match &self.body {
            RequestBody::History { request, .. } => {
                request.same_request(*requested)
            }
            RequestBody::Edit { .. } => false,
        }
    }

    pub const fn context(&self) -> EditingContext {
        let mut context = match &self.body {
            RequestBody::Edit { intent, .. } => crate::engine::context(intent),
            RequestBody::History { request, .. } => history_context(request),
        };
        context.parent = self.parent;
        context
    }

    pub fn reject(&mut self, cause: InteractionOutcomeCause) {
        match &mut self.body {
            RequestBody::Edit { intent, outcome } => {
                outcome.push(InteractionOutcome::not_accepted(
                    intent.stamp(),
                    cause,
                ));
            }
            RequestBody::History { request, outcome } => {
                *outcome = Some(InteractionHistoryOutcome::not_accepted(
                    *request, cause,
                ));
            }
        }
    }

    pub const fn cancel(
        &mut self,
        cause: CancellationCause,
    ) -> intent::InteractionCancellationResult {
        use intent::InteractionCancellationResult as Result;
        if !self.pending() {
            return Result::AlreadyTerminal;
        }
        if let Some(previous) = self.cancellation {
            return Result::AlreadyRequested(CancellationOutcome::new(
                previous,
                ExecutionOutcome::OwnedNonterminal,
            ));
        }
        self.cancellation = Some(cause);
        Result::Requested(CancellationOutcome::new(
            cause,
            ExecutionOutcome::OwnedNonterminal,
        ))
    }
}

pub const fn history_context(
    request: &InteractionHistoryRequest,
) -> EditingContext {
    EditingContext {
        session: Some(request.session()),
        request: Some(request.request()),
        correlation: Some(request.plan().correlation()),
        state: Some(request.target()),
        parent: None,
        cause: None,
    }
}
