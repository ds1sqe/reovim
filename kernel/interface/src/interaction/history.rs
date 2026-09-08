//! History requests use Kernel state, not fabricated semantic edit payloads.

use core::fmt;

use crate::buffer::plan::{BufferPlanDescriptor, BufferPlanStamp};
use crate::buffer::{
    BufferHistoryDirection, BufferHistoryEdge, BufferHistoryNodeIdentity,
    BufferStateStamp,
};

use super::descriptor::InteractionSessionDescriptor;
use super::identity::InteractionRequestIdentity;
use super::outcome::{
    InteractionOutcome, InteractionOutcomeError, accepted_transition,
};
use super::session::InteractionSessionHandle;

/// Structural mismatch while binding or rechecking a History request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionHistoryRequestError {
    /// The observed Buffer is outside the negotiated Resource/Buffer scope.
    ScopeMismatch,
    /// The plan names another or replaced Kernel plan owner.
    PlanOwnerMismatch,
    /// The request belongs to another Session incarnation or generation.
    StaleSession,
}

impl fmt::Display for InteractionHistoryRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ScopeMismatch => "History target is outside Session scope",
            Self::PlanOwnerMismatch => "History plan owner differs",
            Self::StaleSession => "History Session generation differs",
        })
    }
}

impl core::error::Error for InteractionHistoryRequestError {}

/// Tree traversal requested against the observed current node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionHistoryAction {
    /// Follow the current node's parent, if retained and eligible.
    Undo,
    /// Follow an explicit child or Core's selected child at admission.
    Redo {
        /// `None` asks Core to select and fence its preferred child before
        /// preparation; changing preference cannot redirect pending work.
        child: Option<BufferHistoryNodeIdentity>,
    },
}

impl InteractionHistoryAction {
    /// Returns the traversal direction without choosing an edge.
    #[must_use]
    pub const fn direction(self) -> BufferHistoryDirection {
        match self {
            Self::Undo => BufferHistoryDirection::Undo,
            Self::Redo { .. } => BufferHistoryDirection::Redo,
        }
    }

    const fn same_action(self, other: Self) -> bool {
        match (self, other) {
            (Self::Undo, Self::Undo)
            | (Self::Redo { child: None }, Self::Redo { child: None }) => true,
            (
                Self::Redo { child: Some(left) },
                Self::Redo { child: Some(right) },
            ) => left.value() == right.value(),
            _ => false,
        }
    }
}

/// Exact session-scoped request for one shared Buffer History transition.
///
/// The plan retains transaction, cancellation, deadline and causal identities.
/// Construction proves structural binding, not grants, readiness, retention
/// eligibility or reserved capacity; Runtime rechecks those before acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionHistoryRequest {
    session: InteractionSessionHandle,
    request: InteractionRequestIdentity,
    plan: BufferPlanStamp,
    target: BufferStateStamp,
    action: InteractionHistoryAction,
}

impl InteractionHistoryRequest {
    /// Binds one request without calling Domain or Representation policy.
    ///
    /// # Errors
    ///
    /// Returns a target-scope or plan-owner generation mismatch.
    pub const fn new(
        session: &InteractionSessionDescriptor,
        plans: &BufferPlanDescriptor,
        request: InteractionRequestIdentity,
        plan: BufferPlanStamp,
        target: BufferStateStamp,
        action: InteractionHistoryAction,
    ) -> Result<Self, InteractionHistoryRequestError> {
        let value = Self {
            session: session.stamp().handle(),
            request,
            plan,
            target,
            action,
        };
        match value.validate(session, plans) {
            Ok(()) => Ok(value),
            Err(error) => Err(error),
        }
    }

    /// Rechecks structural scope and generations against current descriptors.
    ///
    /// # Errors
    ///
    /// Returns the mismatching Session, plan owner, or target scope.
    pub const fn validate(
        self,
        session: &InteractionSessionDescriptor,
        plans: &BufferPlanDescriptor,
    ) -> Result<(), InteractionHistoryRequestError> {
        if !self.session.matches(session.stamp()) {
            return Err(InteractionHistoryRequestError::StaleSession);
        }
        if !self.plan.owner().same_generation(plans.owner()) {
            return Err(InteractionHistoryRequestError::PlanOwnerMismatch);
        }
        if !session.scope().contains(self.target) {
            return Err(InteractionHistoryRequestError::ScopeMismatch);
        }
        Ok(())
    }

    /// Returns the exact Session lifetime owning this request identity.
    #[must_use]
    pub const fn session(self) -> InteractionSessionHandle {
        self.session
    }

    /// Returns the Session-scoped request identity.
    #[must_use]
    pub const fn request(self) -> InteractionRequestIdentity {
        self.request
    }

    /// Returns the exact plan and its causal/cancellation identities.
    #[must_use]
    pub const fn plan(self) -> BufferPlanStamp {
        self.plan
    }

    /// Returns the observed Resource, Buffer version and current History node.
    #[must_use]
    pub const fn target(self) -> BufferStateStamp {
        self.target
    }

    /// Returns the traversal and optional explicit redo branch selection.
    #[must_use]
    pub const fn action(self) -> InteractionHistoryAction {
        self.action
    }

    /// Matches the complete immutable binding, not only its request number.
    #[must_use]
    pub const fn same_request(self, other: Self) -> bool {
        self.session.same_generation(other.session)
            && self.request.value() == other.request.value()
            && self.plan.same_plan(other.plan)
            && self.target.same_state(other.target)
            && self.action.same_action(other.action)
    }
}

/// History disposition retaining its real request without a semantic Intent.
pub type InteractionHistoryOutcome =
    InteractionOutcome<InteractionHistoryRequest>;

impl InteractionHistoryOutcome {
    /// Constructs the actual result after Core accepted undo or redo.
    ///
    /// This validates stamps and exact endpoints, not actual tree membership.
    /// An absent retained step must instead produce `NoHistory` rejection.
    ///
    /// # Errors
    ///
    /// Returns an identity, version, edge or requested-branch mismatch.
    pub const fn accepted_history(
        request: InteractionHistoryRequest,
        edge: BufferHistoryEdge,
        result: BufferStateStamp,
    ) -> Result<Self, InteractionOutcomeError> {
        if let InteractionHistoryAction::Redo { child: Some(child) } =
            request.action()
            && child.value() != edge.child().value()
        {
            return Err(InteractionOutcomeError::HistoryBranchMismatch);
        }
        let accepted = accepted_transition(
            &request.target(),
            &result,
            request.plan().transaction(),
            request.plan().history_transition(),
            edge,
            Some(request.action().direction()),
        );
        match accepted {
            Ok(value) => Ok(Self::from_accepted(request, &value)),
            Err(error) => Err(error),
        }
    }
}
