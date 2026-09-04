//! Public semantic Resource Access operation port.

use super::access::{
    ResourceAccessDescriptor, ResourceApplicabilityOutcome,
    ResourceCloseOutcome,
};
use super::delivery::{
    ResourceCancellationResult, ResourceObservationAdmission,
    ResourceObservationCompletionDelivery, ResourceOpenAdmission,
    ResourceOpenCompletionDelivery,
};
use super::operation::{
    ResourceCancellationRequest, ResourceCloseRequest,
    ResourceObservationRequest, ResourceOpenRequest, ResourceOperationStamp,
};

/// Public contract implemented by one selected Resource Adapter generation.
pub trait ResourceAccessPort {
    /// Returns the immutable descriptor governing this exact port.
    fn descriptor(&self) -> &ResourceAccessDescriptor;

    /// Checks authority and applicability without revealing Resource lookup.
    fn applicability(
        &self,
        request: &ResourceOpenRequest,
    ) -> ResourceApplicabilityOutcome;

    /// Attempts bounded non-waiting ownership transfer of one open request.
    fn submit_open(
        &mut self,
        request: ResourceOpenRequest,
    ) -> ResourceOpenAdmission;

    /// Attempts bounded non-waiting ownership transfer of one observation.
    fn submit_observation(
        &mut self,
        request: ResourceObservationRequest,
    ) -> ResourceObservationAdmission;

    /// Requests cooperative cancellation without claiming terminal execution.
    fn request_cancellation(
        &mut self,
        request: ResourceCancellationRequest,
    ) -> ResourceCancellationResult;

    /// Takes one terminal open completion without waiting.
    fn take_open_completion(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceOpenCompletionDelivery;

    /// Takes one terminal observation completion without waiting.
    fn take_observation_completion(
        &mut self,
        operation: ResourceOperationStamp,
    ) -> ResourceObservationCompletionDelivery;

    /// Closes admission and terminally resolves or exposes owned work.
    fn close(&mut self, request: ResourceCloseRequest) -> ResourceCloseOutcome;
}
