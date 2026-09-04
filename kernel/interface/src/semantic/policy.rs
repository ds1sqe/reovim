//! Semantic policy failure and contract-violation vocabulary.

use core::fmt;

use crate::buffer::plan::BufferPlanError;
use crate::cancellation::{CancellationCause, FenceCause};

use super::bounds::SemanticBoundViolation;

/// Expected semantic non-result returned without Buffer state change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticPolicyFailure {
    /// The requested semantic capability is unavailable.
    UnsupportedCapability,
    /// A selected semantic schema is unsupported by this provider.
    UnsupportedSchema,
    /// Encoded semantic meaning is invalid under the selected schema.
    InvalidIntent,
    /// No source correspondence exists for the requested semantic location.
    NoCorrespondence,
    /// More than one correspondence exists and policy cannot choose safely.
    AmbiguousCorrespondence,
    /// The semantic or source state is stale at the policy cut.
    StaleState,
    /// The selected Representation is not editable.
    ReadOnly,
    /// Declared policy capacity cannot admit or complete this request.
    Pressure,
    /// Cooperative cancellation was observed.
    Cancelled,
    /// The separately governed deadline was observed as reached.
    DeadlineReached,
    /// The caller ended its wait without proving provider execution terminal.
    CallerTimedOut,
    /// A newer policy or Projection generation replaced this request.
    Replaced,
    /// Policy admission is closed while its owner shuts down.
    Closing,
    /// The policy owner is closed.
    Closed,
    /// Bounded provider code returned another typed local failure.
    ProviderFailure,
    /// The selected boundary caught a provider panic and fenced installation.
    ProviderPanicked,
}

impl fmt::Display for SemanticPolicyFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedCapability => "semantic capability is unsupported",
            Self::UnsupportedSchema => "semantic schema is unsupported",
            Self::InvalidIntent => "semantic Intent is invalid",
            Self::NoCorrespondence => "semantic location has no correspondence",
            Self::AmbiguousCorrespondence => {
                "semantic location has ambiguous correspondence"
            }
            Self::StaleState => "semantic request state is stale",
            Self::ReadOnly => "Representation is read-only",
            Self::Pressure => "semantic policy capacity is exhausted",
            Self::Cancelled => "semantic policy work was cancelled",
            Self::DeadlineReached => "semantic policy deadline was reached",
            Self::CallerTimedOut => "semantic policy caller timed out",
            Self::Replaced => "semantic policy generation was replaced",
            Self::Closing => "semantic policy is closing",
            Self::Closed => "semantic policy is closed",
            Self::ProviderFailure => "semantic policy provider failed",
            Self::ProviderPanicked => "semantic policy provider panicked",
        })
    }
}

impl SemanticPolicyFailure {
    /// Maps an installation fence without claiming provider execution stopped.
    #[must_use]
    pub const fn from_fence(cause: FenceCause) -> Self {
        match cause {
            FenceCause::Cancellation => Self::Cancelled,
            FenceCause::Deadline => Self::DeadlineReached,
            FenceCause::CallerTimeout => Self::CallerTimedOut,
            FenceCause::Replacement => Self::Replaced,
            FenceCause::Closing => Self::Closing,
            FenceCause::Closed => Self::Closed,
        }
    }
}

impl core::error::Error for SemanticPolicyFailure {}

/// Provider output or retained-request contract mismatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticPolicyContractError {
    /// The invoked provider exposes another current descriptor.
    ProviderDescriptorMismatch,
    /// The retained Intent names another Domain generation.
    IntentDomainMismatch,
    /// The retained Intent names another Representation generation.
    IntentRepresentationMismatch,
    /// The retained Intent names another coordinate-space incarnation.
    IntentCoordinateMismatch,
    /// Domain validation output names another retained Intent.
    ValidationStampMismatch,
    /// A validation receipt belongs to another retained Intent.
    ValidationReceiptMismatch,
    /// The selected Buffer-plan contract or carriers differ.
    PlanDescriptorMismatch,
    /// The future plan stamp names another plan-owner generation.
    PlanOwnerMismatch,
    /// A returned plan carries another request or authority stamp.
    ReturnedPlanStampMismatch,
    /// A returned plan targets another observed Buffer state.
    ReturnedPlanTargetMismatch,
    /// A returned plan exceeds the callback's reserved result bytes.
    ReturnedPlanChargeExceeded {
        /// Bytes reserved before provider invocation.
        reserved: u64,
        /// Complete plan bytes returned by the provider.
        actual: u64,
    },
    /// A returned plan fails the exact selected Buffer-plan descriptor.
    ReturnedPlanInvalid(BufferPlanError),
    /// A lowering candidate contains neither or both plan and failure.
    LoweringCandidateShape,
}

impl fmt::Display for SemanticPolicyContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ProviderDescriptorMismatch => {
                "semantic provider descriptor changed"
            }
            Self::IntentDomainMismatch => {
                "semantic Intent names another Domain generation"
            }
            Self::IntentRepresentationMismatch => {
                "semantic Intent names another Representation generation"
            }
            Self::IntentCoordinateMismatch => {
                "semantic Intent names another coordinate-space incarnation"
            }
            Self::ValidationStampMismatch => {
                "Domain validation names another semantic Intent"
            }
            Self::ValidationReceiptMismatch => {
                "Domain validation receipt names another semantic Intent"
            }
            Self::PlanDescriptorMismatch => {
                "Representation selected another Buffer-plan contract"
            }
            Self::PlanOwnerMismatch => {
                "semantic Intent names another Buffer-plan owner"
            }
            Self::ReturnedPlanStampMismatch => {
                "lowered plan carries another request stamp"
            }
            Self::ReturnedPlanTargetMismatch => {
                "lowered plan targets another Buffer state"
            }
            Self::ReturnedPlanChargeExceeded { .. } => {
                "lowered plan exceeds its reserved result bytes"
            }
            Self::ReturnedPlanInvalid(_) => {
                "lowered plan fails the selected Buffer-plan descriptor"
            }
            Self::LoweringCandidateShape => {
                "lowering candidate has an incoherent result shape"
            }
        })
    }
}

impl core::error::Error for SemanticPolicyContractError {}

/// Failure before a semantic policy callback may run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticPolicyRequestError {
    /// Retained request and selected descriptors do not match.
    Contract(SemanticPolicyContractError),
    /// A declared policy reservation exceeded its bounds.
    Bound(SemanticBoundViolation),
    /// Cancellation was already requested before invocation.
    Cancelled(CancellationCause),
    /// The separately governed deadline was already reached.
    DeadlineReached,
    /// The selected Representation does not permit edit lowering.
    ReadOnly,
}

impl fmt::Display for SemanticPolicyRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Contract(_) => "semantic policy request contracts differ",
            Self::Bound(_) => "semantic policy request exceeds its reservation",
            Self::Cancelled(_) => {
                "semantic policy request was cancelled before invocation"
            }
            Self::DeadlineReached => {
                "semantic policy deadline was reached before invocation"
            }
            Self::ReadOnly => "semantic policy request targets read-only state",
        })
    }
}

impl core::error::Error for SemanticPolicyRequestError {}
