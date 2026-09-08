//! Read-only semantic snapshots, before client-specific delivery binding.

use core::fmt;

use crate::bound::{BoundError, CheckedCount};
use crate::cancellation::FenceCause;

use super::bounds::SemanticBoundField;
use super::policy::SemanticPolicyFailure;

mod descriptor;
mod output;
mod request;
mod schema;

pub use descriptor::RepresentationSnapshotDescriptor;
pub use output::{
    RepresentationSnapshot, RepresentationSnapshotPayload,
    RepresentationSnapshotRejection,
};
pub use request::RepresentationSnapshotRequest;
pub use schema::RepresentationSnapshotSchema;

/// Semantic payload complexity, independent of source selection cardinality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationSnapshotLimits {
    /// Maximum encoded semantic objects, not selected source elements.
    pub items: CheckedCount,
    /// Maximum schema-defined nesting depth.
    pub depth: CheckedCount,
}

/// Exact snapshot output quantity exceeding its admitted reservation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationSnapshotBound {
    /// Encoded semantic objects.
    Items,
    /// Maximum schema-defined nesting depth.
    Depth,
    /// Complete owned result allocation.
    Bytes,
}

/// A snapshot failure never changes an accepted Buffer transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationSnapshotError {
    /// Native model, its generation or required revision schema differs.
    ModelMismatch,
    /// The current Buffer observation differs from interpreted state.
    StateMismatch,
    /// The selected provider's immutable descriptor changed.
    ProviderMismatch,
    /// The call reservation belongs to another accounting owner.
    CallOwnerMismatch,
    /// Invocation reservation violates a named call bound.
    CallBound {
        /// Exact call quantity rejected before provider work.
        field: SemanticBoundField,
        /// Typed arithmetic, unit or capacity failure.
        cause: BoundError,
    },
    /// Expected semantic non-result, including unavailable/unrepresentable.
    Policy(SemanticPolicyFailure),
    /// Selected Representation or observed Projection generation differs.
    ProjectionMismatch,
    /// Provider returned data under another selected output schema.
    OutputSchemaMismatch,
    /// A payload-bearing snapshot requires an encoded semantic object.
    EmptyEncoding,
    /// Output allocation cannot fit the portable byte count.
    OutputSizeOverflow,
    /// Claimed shape or actual allocation exceeds the admitted limit.
    OutputBound {
        /// Quantity whose reservation was exceeded.
        field: RepresentationSnapshotBound,
        /// Maximum admitted quantity.
        maximum: u64,
        /// Quantity returned by the provider.
        actual: u64,
    },
    /// Cancellation, replacement or owner shutdown fenced installation.
    Fenced(FenceCause),
}

impl fmt::Display for RepresentationSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Representation snapshot: {self:?}")
    }
}

impl core::error::Error for RepresentationSnapshotError {}

/// Selected read-only interpretation, independent of edit-lowering support.
///
/// Runtime invokes this bounded callback outside Buffer/Client authority. It
/// grants no I/O, executor, Session, attachment or presentation handles. Use
/// [`RepresentationSnapshotRequest::produce_with`] to validate the provider
/// and bind its result to the retained request. Declared work/temporary-memory
/// bounds are trusted provider obligations, not a sandbox.
pub trait RepresentationSnapshotPort {
    /// Native immutable state understood by this selected provider.
    type State;

    /// Returns the immutable descriptor of this exact provider generation.
    fn descriptor(&self) -> &RepresentationSnapshotDescriptor;

    /// Interprets observed state without mutating it or choosing delivery.
    ///
    /// # Errors
    ///
    /// Return unsupported schema/capability, unavailable native data or
    /// unrepresentable meaning explicitly. Failure cannot reverse an edit.
    fn snapshot(
        &self,
        request: &RepresentationSnapshotRequest<'_, Self::State>,
    ) -> Result<RepresentationSnapshotPayload, SemanticPolicyFailure>;
}
