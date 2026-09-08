//! Resource-native preparation outside authoritative Buffer ownership.
//!
//! Models own native data and operations, never Buffer or History authority.
//! These contracts do not execute an editor or enforce a provider sandbox.

use core::fmt;

use crate::bound::{CheckedByteCount, CheckedCount};
use crate::buffer::BufferStateStamp;
use crate::buffer::plan::{BufferPlanDescriptor, BufferPlanError};
use crate::cancellation::FenceCause;
use crate::lifecycle::LifecycleIdentity;
use crate::operation::ContractSelection;

use super::ResourceValueSchemas;
use super::carrier::ResourceCarrierStamp;

mod prepared;
mod request;
mod validation;

pub use prepared::{ResourcePreparedState, ResourceStateRejection};
pub use request::{
    ResourcePreparationStamp, ResourceStateInput, ResourceStatePreparation,
    ResourceStateTarget, ResourceStateTransition,
};

/// Exact native model selection, including replacement and value schemas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceStateModelBinding {
    /// Selected state/operation semantics; not a universal storage encoding.
    pub contract: ContractSelection,
    /// Provider instance and replacement fence.
    pub lifecycle: LifecycleIdentity,
    /// Resource-native address, extent, unit and revision schemas.
    pub schemas: ResourceValueSchemas,
    /// Exact encoded observation family admitted before native decoding.
    pub payload: ResourceCarrierStamp,
}

/// Immutable data paired with the exact authority observation it describes.
///
/// `State` may be a persistent root or sparse retained data. Implementations
/// must not mutate reachable state through interior mutability. No universal
/// copy, serialization, thread-transfer or native-address model is imposed.
pub struct ResourceWorkingState<'a, State> {
    /// Selected native model and schemas.
    pub model: ResourceStateModelBinding,
    /// Exact Buffer, source and History observation.
    pub stamp: BufferStateStamp,
    /// Read-only native data; a stamp alone is not a state observation.
    pub state: &'a State,
}

impl<State> fmt::Debug for ResourceWorkingState<'_, State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceWorkingState")
            .field("model", &self.model)
            .field("stamp", &self.stamp)
            .finish_non_exhaustive()
    }
}

/// Per-preparation reservations supplied before invoking native policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceStateLimits {
    /// Maximum conservatively charged retained memory, including shared roots.
    pub resident_bytes: CheckedByteCount,
    /// Maximum work under the selected model's documented work-unit rules.
    pub work_units: CheckedCount,
    /// Maximum borrowed observations, checked before scanning their headers.
    pub observation_count: CheckedCount,
    /// Maximum total encoded observation bytes supplied to native work.
    pub observation_bytes: CheckedByteCount,
}

impl ResourceStateLimits {
    pub(super) const fn admits(self, usage: ResourceStateUsage) -> bool {
        usage.resident_bytes.value() <= self.resident_bytes.value()
            && usage.work_units.value() <= self.work_units.value()
    }
}

/// Native output accounting, distinct from borrowed input reservations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceStateUsage {
    /// Conservatively charged retained bytes, including shared roots.
    pub resident_bytes: CheckedByteCount,
    /// Work consumed under the selected model's documented unit rules.
    pub work_units: CheckedCount,
}

/// A model's owned immutable result and declared resource usage.
///
/// This declaration is a trusted provider obligation, not a measurement of
/// arbitrary Rust heap graphs. Core separately reserves aggregate capacity.
/// Rejected output must be released outside commit ownership.
pub struct ResourceStateOutput<State> {
    /// Prepared data; never a mutable alias of the observed state.
    pub state: State,
    /// Retained memory and consumed model-defined work units.
    pub usage: ResourceStateUsage,
}

impl<State> fmt::Debug for ResourceStateOutput<State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceStateOutput")
            .field("usage", &self.usage)
            .finish_non_exhaustive()
    }
}

/// Typed failure before an authoritative state transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceStateError {
    /// Provider, replacement generation or selected schemas differ.
    ModelMismatch,
    /// Snapshot revision does not use the model's revision schema.
    RevisionSchemaMismatch,
    /// Encoded observation type/version differs before native decoding.
    PayloadSchemaMismatch,
    /// The supplied operation contract is not the model-selected descriptor.
    PlanDescriptorMismatch,
    /// Observed, retained or current authority no longer matches the request.
    StateMismatch,
    /// Plan identity or the Core-selected History relation changed.
    PreparationMismatch,
    /// A borrowed plan failed structural validation before native decoding.
    Plan(BufferPlanError),
    /// Preparation or installation authority ended for this cause.
    Fenced(FenceCause),
    /// Declared memory or model-work reservation was exhausted.
    Pressure,
    /// Native operations are malformed under the selected model contract.
    MalformedOperation,
    /// A native precondition did not hold in the observed immutable state.
    PreconditionFailed,
    /// Required resident data or reconstruction evidence is unavailable.
    Unavailable,
    /// The selected trusted provider failed without producing a candidate.
    ProviderFailure,
}

impl fmt::Display for ResourceStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Resource state preparation: {self:?}")
    }
}

impl core::error::Error for ResourceStateError {}

/// Resource-owned native evaluation, invoked by Runtime outside commit.
///
/// Invoke through [`ResourceStatePreparation::prepare_with`] to validate the
/// selected provider before native decoding and bind its owned result.
/// Preparation is synchronous, pure and bounded by the request reservation.
/// It performs no I/O or authoritative mutation. Unavailable data returns a
/// typed rejection; Runtime may acquire a bounded observation and try again.
/// Models never select History eligibility or successor Buffer versions.
pub trait ResourceStateModel {
    /// Immutable native state, with no Kernel-prescribed addressing scheme.
    type State;

    /// Returns the exact provider generation and supported native schemas.
    fn binding(&self) -> ResourceStateModelBinding;

    /// Returns the exact lowered-operation contract selected for this model.
    fn plan_descriptor(&self) -> &BufferPlanDescriptor;

    /// Evaluates initial data, lowered operations or selected reconstruction.
    ///
    /// # Errors
    ///
    /// Returns a native, precondition, availability, pressure or provider
    /// failure without modifying borrowed state. No candidate means no edit.
    fn prepare(
        &self,
        request: &ResourceStatePreparation<'_, Self::State>,
    ) -> Result<ResourceStateOutput<Self::State>, ResourceStateError>;
}
