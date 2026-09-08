use core::fmt;

use crate::cancellation::{CancellationState, DeadlineState, FenceCause};
use crate::resource::state::ResourceWorkingState;
use crate::semantic::bounds::{
    SemanticBoundViolation, SemanticPolicyCallCharge,
};
use crate::semantic::stamp::ProjectionStamp;
use crate::semantic::state::validate_working_state;

use super::output::validate_payload;
use super::{
    RepresentationSnapshot, RepresentationSnapshotDescriptor,
    RepresentationSnapshotError, RepresentationSnapshotPort,
    RepresentationSnapshotRejection,
};

/// Bounded immutable snapshot request, without client delivery state.
pub struct RepresentationSnapshotRequest<'a, State> {
    descriptor: &'a RepresentationSnapshotDescriptor,
    observed: &'a ResourceWorkingState<'a, State>,
    projection: ProjectionStamp,
    charge: SemanticPolicyCallCharge,
}

impl<'a, State> RepresentationSnapshotRequest<'a, State> {
    /// Validates state/model and callback reservation before interpretation.
    ///
    /// # Errors
    ///
    /// Returns a binding, projection, reservation or pre-invocation fence.
    pub fn new(
        descriptor: &'a RepresentationSnapshotDescriptor,
        observed: &'a ResourceWorkingState<'a, State>,
        projection: ProjectionStamp,
        charge: SemanticPolicyCallCharge,
        cancellation: CancellationState,
        deadline: DeadlineState,
    ) -> Result<Self, RepresentationSnapshotError> {
        check_fences(cancellation, deadline)?;
        descriptor
            .call()
            .bounds()
            .validate_charge(charge)
            .map_err(call_bound)?;
        validate_working_state(descriptor.state_model(), observed)
            .map_err(|_| RepresentationSnapshotError::ModelMismatch)?;
        if !projection
            .representation()
            .same_generation(descriptor.representation())
        {
            return Err(RepresentationSnapshotError::ProjectionMismatch);
        }
        Ok(Self { descriptor, observed, projection, charge })
    }

    /// Returns selected interpretation and output terms.
    #[must_use]
    pub const fn descriptor(&self) -> &RepresentationSnapshotDescriptor {
        self.descriptor
    }

    /// Returns native immutable data and its exact authority observation.
    #[must_use]
    pub const fn observed(&self) -> &ResourceWorkingState<'a, State> {
        self.observed
    }

    /// Returns observed Projection identity/generation, not future delivery.
    #[must_use]
    pub const fn projection(&self) -> ProjectionStamp {
        self.projection
    }

    /// Returns callback work, temporary-memory and result-byte reservations.
    #[must_use]
    pub const fn charge(&self) -> SemanticPolicyCallCharge {
        self.charge
    }

    /// Invokes the exact provider and privately binds its semantic output.
    ///
    /// This is not installation. After this returns, the owner observes
    /// current state/fences and calls
    /// [`RepresentationSnapshot::check_admission`].
    /// Callback failure leaves the borrowed state untouched. The outside
    /// Runtime owns panic containment; this portable call cannot catch unwind.
    ///
    /// # Errors
    ///
    /// Returns expected provider non-result, or descriptor/schema/shape
    /// violation while retaining any provider-owned output for safe release.
    pub fn produce_with<Provider>(
        &self,
        provider: &Provider,
    ) -> Result<RepresentationSnapshot, RepresentationSnapshotRejection>
    where
        Provider: RepresentationSnapshotPort<State = State> + ?Sized,
    {
        check_provider(self.descriptor, provider.descriptor()).map_err(
            |cause| RepresentationSnapshotRejection { cause, payload: None },
        )?;
        let payload = provider.snapshot(self).map_err(|failure| {
            RepresentationSnapshotRejection {
                cause: RepresentationSnapshotError::Policy(failure),
                payload: None,
            }
        })?;
        let checked = check_provider(self.descriptor, provider.descriptor())
            .and_then(|()| {
                validate_payload(
                    self.descriptor,
                    &payload,
                    self.charge.result_bytes().value(),
                )
            });
        if let Err(cause) = checked {
            return Err(RepresentationSnapshotRejection {
                cause,
                payload: Some(payload),
            });
        }
        Ok(RepresentationSnapshot::new(
            self.descriptor,
            &self.observed.stamp,
            &self.projection,
            payload,
        ))
    }
}

impl<State> fmt::Debug for RepresentationSnapshotRequest<'_, State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationSnapshotRequest")
            .field("observed", &self.observed)
            .field("projection", &self.projection)
            .field("charge", &self.charge)
            .finish_non_exhaustive()
    }
}

fn check_provider(
    selected: &RepresentationSnapshotDescriptor,
    actual: &RepresentationSnapshotDescriptor,
) -> Result<(), RepresentationSnapshotError> {
    if selected != actual {
        return Err(RepresentationSnapshotError::ProviderMismatch);
    }
    Ok(())
}

const fn call_bound(
    cause: SemanticBoundViolation,
) -> RepresentationSnapshotError {
    match cause {
        SemanticBoundViolation::Bound { field, cause } => {
            RepresentationSnapshotError::CallBound { field, cause }
        }
        SemanticBoundViolation::ChargeOwnerMismatch { .. } => {
            RepresentationSnapshotError::CallOwnerMismatch
        }
    }
}

const fn check_fences(
    cancellation: CancellationState,
    deadline: DeadlineState,
) -> Result<(), RepresentationSnapshotError> {
    if matches!(cancellation, CancellationState::Requested(_)) {
        return Err(RepresentationSnapshotError::Fenced(
            FenceCause::Cancellation,
        ));
    }
    if matches!(deadline, DeadlineState::Reached) {
        return Err(RepresentationSnapshotError::Fenced(FenceCause::Deadline));
    }
    Ok(())
}
