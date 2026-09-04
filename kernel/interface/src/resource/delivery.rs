//! Resource operation admission, cancellation, and terminal delivery.

use core::fmt;

use crate::cancellation::CancellationOutcome;

use super::access::{ResourceObservationOutcome, ResourceOpenOutcome};
use super::operation::{
    ResourceObservationRequest, ResourceOpenRequest, ResourceOperationStamp,
};

/// Whether a Resource request transferred to deferred provider ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceAdmissionStatus {
    /// The provider owns the request until one terminal completion.
    Admitted,
    /// The provider retained nothing and returned the complete request.
    NotAdmitted,
}

/// Cause of definite immediate Resource operation non-admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceAdmissionFailure {
    /// Authority was denied before revealing lookup or prior request state.
    Denied,
    /// The operation or Resource family is unsupported.
    Unsupported,
    /// Selected contract or carrier terms are incompatible.
    Incompatible,
    /// Request structure failed validation before provider work.
    InvalidRequest,
    /// The adapter generation has not published readiness.
    NotReady,
    /// This exact request identity was already admitted.
    DuplicateRequest,
    /// The request names another or retired Resource Access generation.
    StaleAccess,
    /// Declared capacity cannot retain the request and its terminal result.
    Pressure,
    /// Cancellation was already requested before admission.
    Cancelled,
    /// A newer adapter generation replaced the request target.
    Replaced,
    /// Adapter shutdown has begun and admission is closed.
    Closing,
    /// The adapter is closed and rejects admission.
    Closed,
    /// Another bounded adapter failure prevented ownership transfer.
    AdapterFailure,
}

impl fmt::Display for ResourceAdmissionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Denied => "Resource operation authority was denied",
            Self::Unsupported => "Resource operation is unsupported",
            Self::Incompatible => "Resource operation is incompatible",
            Self::InvalidRequest => "Resource operation request is invalid",
            Self::NotReady => "Resource adapter is not ready",
            Self::DuplicateRequest => {
                "Resource operation request was already admitted"
            }
            Self::StaleAccess => "Resource Access generation is stale",
            Self::Pressure => "Resource operation capacity is exhausted",
            Self::Cancelled => {
                "Resource operation was cancelled before admission"
            }
            Self::Replaced => "Resource adapter generation was replaced",
            Self::Closing => "Resource adapter is closing",
            Self::Closed => "Resource adapter is closed",
            Self::AdapterFailure => "Resource adapter failed before admission",
        })
    }
}

impl core::error::Error for ResourceAdmissionFailure {}

/// Open admission result that preserves exact request ownership.
pub struct ResourceOpenAdmission {
    admitted: Option<ResourceOperationStamp>,
    failure: Option<ResourceAdmissionFailure>,
    returned: Option<ResourceOpenRequest>,
}

impl fmt::Debug for ResourceOpenAdmission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceOpenAdmission")
            .field("status", &self.status())
            .field("admitted", &self.admitted)
            .field("failure", &self.failure)
            .field("returned", &self.returned)
            .finish()
    }
}

impl ResourceOpenAdmission {
    /// Reports transfer of this exact open request to the provider.
    #[must_use]
    pub const fn admitted(request: &ResourceOpenRequest) -> Self {
        Self {
            admitted: Some(request.operation()),
            failure: None,
            returned: None,
        }
    }

    /// Reports definite non-admission and returns the exact request.
    #[must_use]
    pub const fn not_admitted(
        failure: ResourceAdmissionFailure,
        request: ResourceOpenRequest,
    ) -> Self {
        Self { admitted: None, failure: Some(failure), returned: Some(request) }
    }

    /// Returns whether the provider accepted ownership.
    #[must_use]
    pub const fn status(&self) -> ResourceAdmissionStatus {
        if self.admitted.is_some() {
            ResourceAdmissionStatus::Admitted
        } else {
            ResourceAdmissionStatus::NotAdmitted
        }
    }

    /// Returns the admitted operation stamp, when ownership transferred.
    #[must_use]
    pub const fn admitted_operation(&self) -> Option<ResourceOperationStamp> {
        self.admitted
    }

    /// Returns the immediate non-admission cause, when applicable.
    #[must_use]
    pub const fn failure(&self) -> Option<ResourceAdmissionFailure> {
        self.failure
    }

    /// Borrows the returned request after definite non-admission.
    #[must_use]
    pub const fn returned_request(&self) -> Option<&ResourceOpenRequest> {
        self.returned.as_ref()
    }

    /// Returns the complete request after definite non-admission.
    #[must_use]
    pub const fn into_returned_request(self) -> Option<ResourceOpenRequest> {
        self.returned
    }
}

/// Observation admission result that preserves move-only request ownership.
pub struct ResourceObservationAdmission {
    admitted: Option<ResourceOperationStamp>,
    failure: Option<ResourceAdmissionFailure>,
    returned: Option<ResourceObservationRequest>,
}

impl fmt::Debug for ResourceObservationAdmission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceObservationAdmission")
            .field("status", &self.status())
            .field("admitted", &self.admitted)
            .field("failure", &self.failure)
            .field("returned", &self.returned)
            .finish()
    }
}

impl ResourceObservationAdmission {
    /// Reports transfer of this exact observation request to the provider.
    #[must_use]
    pub const fn admitted(request: &ResourceObservationRequest) -> Self {
        Self {
            admitted: Some(request.operation()),
            failure: None,
            returned: None,
        }
    }

    /// Reports definite non-admission and returns the exact request.
    #[must_use]
    pub const fn not_admitted(
        failure: ResourceAdmissionFailure,
        request: ResourceObservationRequest,
    ) -> Self {
        Self { admitted: None, failure: Some(failure), returned: Some(request) }
    }

    /// Returns whether the provider accepted ownership.
    #[must_use]
    pub const fn status(&self) -> ResourceAdmissionStatus {
        if self.admitted.is_some() {
            ResourceAdmissionStatus::Admitted
        } else {
            ResourceAdmissionStatus::NotAdmitted
        }
    }

    /// Returns the admitted operation stamp, when ownership transferred.
    #[must_use]
    pub const fn admitted_operation(&self) -> Option<ResourceOperationStamp> {
        self.admitted
    }

    /// Returns the immediate non-admission cause, when applicable.
    #[must_use]
    pub const fn failure(&self) -> Option<ResourceAdmissionFailure> {
        self.failure
    }

    /// Borrows the returned request after definite non-admission.
    #[must_use]
    pub const fn returned_request(
        &self,
    ) -> Option<&ResourceObservationRequest> {
        self.returned.as_ref()
    }

    /// Returns the complete request after definite non-admission.
    #[must_use]
    pub fn into_returned_request(self) -> Option<ResourceObservationRequest> {
        self.returned
    }
}

/// Immediate acknowledgement of one cooperative cancellation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceCancellationResult {
    /// Cancellation was newly requested with separate execution disposition.
    Requested(CancellationOutcome),
    /// The same cancellation was already requested and remains observable.
    AlreadyRequested(CancellationOutcome),
    /// The target already reached and retained a terminal completion.
    AlreadyTerminal,
    /// Authority was denied without revealing whether the target exists.
    Denied,
    /// No admitted operation matches the supplied stamp.
    UnknownRequest,
    /// The request targets another adapter generation.
    StaleAdapter,
    /// Adapter shutdown is resolving already admitted work.
    Closing,
    /// The adapter is closed and owns no cancellable operation.
    Closed,
}

/// Reason an exact deferred completion is not delivered by this invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceCompletionUnavailable {
    /// The admitted operation remains owned and nonterminal.
    Pending,
    /// No admitted operation matches the supplied stamp.
    UnknownRequest,
    /// The supplied stamp names another operation kind.
    WrongOperation,
    /// This terminal completion was already transferred to the caller.
    AlreadyDelivered,
    /// Retention loss created a visible completion-reconciliation gap.
    Gap,
    /// A newer adapter generation fenced this completion lookup.
    Replaced,
    /// Adapter shutdown is still resolving terminal delivery.
    Closing,
    /// The adapter is closed and has no retained matching completion.
    Closed,
}

/// Delivered or unavailable state of one completion retrieval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceCompletionStatus {
    /// The call transfers one exact terminal completion.
    Delivered,
    /// No completion transfers for the stated reason.
    Unavailable(ResourceCompletionUnavailable),
}

/// Exact returned open request and its one terminal outcome.
pub struct ResourceOpenCompletion {
    request: ResourceOpenRequest,
    outcome: ResourceOpenOutcome,
}

impl fmt::Debug for ResourceOpenCompletion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceOpenCompletion")
            .field("request", &self.request)
            .field("outcome", &self.outcome)
            .finish()
    }
}

impl ResourceOpenCompletion {
    /// Binds one returned request to its terminal open outcome.
    #[must_use]
    pub const fn new(
        request: ResourceOpenRequest,
        outcome: ResourceOpenOutcome,
    ) -> Self {
        Self { request, outcome }
    }

    /// Returns the exact operation stamp.
    #[must_use]
    pub const fn operation(&self) -> ResourceOperationStamp {
        self.request.operation()
    }

    /// Returns the completed open request.
    #[must_use]
    pub const fn request(&self) -> ResourceOpenRequest {
        self.request
    }

    /// Borrows the terminal open outcome.
    #[must_use]
    pub const fn outcome(&self) -> &ResourceOpenOutcome {
        &self.outcome
    }

    /// Releases the completion into request and outcome ownership.
    #[must_use]
    pub const fn into_parts(
        self,
    ) -> (ResourceOpenRequest, ResourceOpenOutcome) {
        (self.request, self.outcome)
    }
}

/// Exact returned observation request and its one terminal outcome.
pub struct ResourceObservationCompletion {
    request: ResourceObservationRequest,
    outcome: ResourceObservationOutcome,
}

impl fmt::Debug for ResourceObservationCompletion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceObservationCompletion")
            .field("request", &self.request)
            .field("outcome", &self.outcome)
            .finish()
    }
}

impl ResourceObservationCompletion {
    /// Binds one returned request to its terminal observation outcome.
    #[must_use]
    pub const fn new(
        request: ResourceObservationRequest,
        outcome: ResourceObservationOutcome,
    ) -> Self {
        Self { request, outcome }
    }

    /// Returns the exact operation stamp.
    #[must_use]
    pub const fn operation(&self) -> ResourceOperationStamp {
        self.request.operation()
    }

    /// Borrows the completed observation request.
    #[must_use]
    pub const fn request(&self) -> &ResourceObservationRequest {
        &self.request
    }

    /// Borrows the terminal observation outcome.
    #[must_use]
    pub const fn outcome(&self) -> &ResourceObservationOutcome {
        &self.outcome
    }

    /// Releases the completion into request and outcome ownership.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (ResourceObservationRequest, ResourceObservationOutcome) {
        (self.request, self.outcome)
    }
}

/// Non-waiting retrieval result for one open completion.
pub struct ResourceOpenCompletionDelivery {
    completion: Option<ResourceOpenCompletion>,
    unavailable: Option<ResourceCompletionUnavailable>,
}

impl fmt::Debug for ResourceOpenCompletionDelivery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceOpenCompletionDelivery")
            .field("status", &self.status())
            .field("completion", &self.completion)
            .field("unavailable", &self.unavailable)
            .finish()
    }
}

impl ResourceOpenCompletionDelivery {
    /// Transfers one exact terminal open completion.
    #[must_use]
    pub const fn delivered(completion: ResourceOpenCompletion) -> Self {
        Self { completion: Some(completion), unavailable: None }
    }

    /// Reports why this invocation transferred no open completion.
    #[must_use]
    pub const fn unavailable(cause: ResourceCompletionUnavailable) -> Self {
        Self { completion: None, unavailable: Some(cause) }
    }

    /// Returns delivered or typed unavailable status.
    #[must_use]
    pub const fn status(&self) -> ResourceCompletionStatus {
        match self.unavailable {
            Some(cause) => ResourceCompletionStatus::Unavailable(cause),
            None => ResourceCompletionStatus::Delivered,
        }
    }

    /// Borrows the terminal completion when delivered.
    #[must_use]
    pub const fn completion(&self) -> Option<&ResourceOpenCompletion> {
        self.completion.as_ref()
    }

    /// Returns terminal completion ownership when delivered.
    #[must_use]
    pub const fn into_completion(self) -> Option<ResourceOpenCompletion> {
        self.completion
    }
}

/// Non-waiting retrieval result for one observation completion.
pub struct ResourceObservationCompletionDelivery {
    completion: Option<ResourceObservationCompletion>,
    unavailable: Option<ResourceCompletionUnavailable>,
}

impl fmt::Debug for ResourceObservationCompletionDelivery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceObservationCompletionDelivery")
            .field("status", &self.status())
            .field("completion", &self.completion)
            .field("unavailable", &self.unavailable)
            .finish()
    }
}

impl ResourceObservationCompletionDelivery {
    /// Transfers one exact terminal observation completion.
    #[must_use]
    pub const fn delivered(completion: ResourceObservationCompletion) -> Self {
        Self { completion: Some(completion), unavailable: None }
    }

    /// Reports why this invocation transferred no observation completion.
    #[must_use]
    pub const fn unavailable(cause: ResourceCompletionUnavailable) -> Self {
        Self { completion: None, unavailable: Some(cause) }
    }

    /// Returns delivered or typed unavailable status.
    #[must_use]
    pub const fn status(&self) -> ResourceCompletionStatus {
        match self.unavailable {
            Some(cause) => ResourceCompletionStatus::Unavailable(cause),
            None => ResourceCompletionStatus::Delivered,
        }
    }

    /// Borrows the terminal completion when delivered.
    #[must_use]
    pub const fn completion(&self) -> Option<&ResourceObservationCompletion> {
        self.completion.as_ref()
    }

    /// Returns terminal completion ownership when delivered.
    #[must_use]
    pub fn into_completion(self) -> Option<ResourceObservationCompletion> {
        self.completion
    }
}
