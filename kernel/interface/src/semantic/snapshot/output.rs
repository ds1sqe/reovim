use alloc::boxed::Box;
use core::fmt;

use crate::bound::CheckedCount;
use crate::buffer::BufferStateStamp;
use crate::cancellation::InstallationOutcome;
use crate::semantic::stamp::ProjectionStamp;

use super::{
    RepresentationSnapshotBound, RepresentationSnapshotDescriptor,
    RepresentationSnapshotError, RepresentationSnapshotSchema,
};

/// Provider-owned full semantic encoding, not a client rendering payload.
///
/// The schema must encode empty state explicitly when relevant. A semantic
/// object describing an empty collection is not a fabricated source element.
/// Zero objects or bytes cannot become an existing payload-bearing Update.
pub struct RepresentationSnapshotPayload {
    /// Exact selected output schema, including its admitted version.
    pub schema: RepresentationSnapshotSchema,
    /// Encoded semantic objects, independent of selected-source cardinality.
    pub items: CheckedCount,
    /// Maximum schema-defined nesting depth.
    pub depth: CheckedCount,
    /// Complete immutable encoding returned within the result reservation.
    pub bytes: Box<[u8]>,
}

impl fmt::Debug for RepresentationSnapshotPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationSnapshotPayload")
            .field("schema", &self.schema)
            .field("items", &self.items)
            .field("depth", &self.depth)
            .finish_non_exhaustive()
    }
}

/// Interpretation bound to the immutable request, still without authority.
///
/// Projection/Interaction owners assign future generations, lineage, Session
/// and delivery identities after revalidation. No provider may choose those.
pub struct RepresentationSnapshot {
    descriptor: RepresentationSnapshotDescriptor,
    target: BufferStateStamp,
    projection: ProjectionStamp,
    payload: RepresentationSnapshotPayload,
}

impl RepresentationSnapshot {
    pub(super) fn new(
        descriptor: &RepresentationSnapshotDescriptor,
        target: &BufferStateStamp,
        projection: &ProjectionStamp,
        payload: RepresentationSnapshotPayload,
    ) -> Self {
        Self {
            descriptor: descriptor.clone(),
            target: *target,
            projection: *projection,
            payload,
        }
    }

    /// Returns the exact interpreted Buffer state, including its History node.
    #[must_use]
    pub const fn target(&self) -> BufferStateStamp {
        self.target
    }

    /// Returns the observed Projection, not a provider-selected successor.
    #[must_use]
    pub const fn projection(&self) -> ProjectionStamp {
        self.projection
    }

    /// Borrows the semantic payload without changing its state identity.
    #[must_use]
    pub const fn payload(&self) -> &RepresentationSnapshotPayload {
        &self.payload
    }

    /// Rechecks current owner state without callbacks or payload destruction.
    ///
    /// The Projection owner observes these inputs under the SAME
    /// serialization that installs the result. It also retains the invocation
    /// reservation and checks grants/aggregate delivery capacity separately.
    /// A check before obtaining that authority cannot authorize installation.
    /// No Client position policy or acknowledgement gates Buffer acceptance.
    ///
    /// # Errors
    ///
    /// Returns the exact changed descriptor, target, Projection or fence.
    /// The caller retains the entire snapshot on rejection.
    pub fn check_admission(
        &self,
        descriptor: &RepresentationSnapshotDescriptor,
        target: &BufferStateStamp,
        projection: &ProjectionStamp,
        installation: InstallationOutcome,
    ) -> Result<(), RepresentationSnapshotError> {
        if let InstallationOutcome::Fenced(cause) = installation {
            return Err(RepresentationSnapshotError::Fenced(cause));
        }
        if self.descriptor != *descriptor {
            return Err(RepresentationSnapshotError::ProviderMismatch);
        }
        if !self.target.same_state(*target) {
            return Err(RepresentationSnapshotError::StateMismatch);
        }
        if !self.projection.same_state(*projection) {
            return Err(RepresentationSnapshotError::ProjectionMismatch);
        }
        Ok(())
    }

    /// Transfers payload ownership for existing Interaction update binding.
    ///
    /// Ownership transfer is not admission or permission to relabel state.
    #[must_use]
    pub fn into_payload(self) -> RepresentationSnapshotPayload {
        self.payload
    }
}

impl fmt::Debug for RepresentationSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationSnapshot")
            .field("target", &self.target)
            .field("projection", &self.projection)
            .field("payload", &self.payload)
            .finish_non_exhaustive()
    }
}

/// Explicit failure preserving any output already returned by the provider.
#[derive(Debug)]
pub struct RepresentationSnapshotRejection {
    /// Reason the output cannot be used.
    pub cause: RepresentationSnapshotError,
    /// Owned output, retained on descriptor, schema or bound failure.
    pub payload: Option<RepresentationSnapshotPayload>,
}

pub(super) fn validate_payload(
    descriptor: &RepresentationSnapshotDescriptor,
    payload: &RepresentationSnapshotPayload,
    result_bytes: u64,
) -> Result<(), RepresentationSnapshotError> {
    if payload.schema != descriptor.output_schema() {
        return Err(RepresentationSnapshotError::OutputSchemaMismatch);
    }
    if payload.items.value() == 0 || payload.bytes.is_empty() {
        return Err(RepresentationSnapshotError::EmptyEncoding);
    }
    let bytes = u64::try_from(payload.bytes.len())
        .map_err(|_| RepresentationSnapshotError::OutputSizeOverflow)?;
    check_bound(RepresentationSnapshotBound::Bytes, result_bytes, bytes)?;
    check_bound(
        RepresentationSnapshotBound::Items,
        descriptor.limits().items.value(),
        payload.items.value(),
    )?;
    check_bound(
        RepresentationSnapshotBound::Depth,
        descriptor.limits().depth.value(),
        payload.depth.value(),
    )
}

const fn check_bound(
    field: RepresentationSnapshotBound,
    maximum: u64,
    actual: u64,
) -> Result<(), RepresentationSnapshotError> {
    if actual > maximum {
        return Err(RepresentationSnapshotError::OutputBound {
            field,
            maximum,
            actual,
        });
    }
    Ok(())
}
