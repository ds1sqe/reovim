//! Bounded Diagnostic Output contract values without routing mechanics.

use core::fmt;

pub use ::reovim_foundation::bound::{
    BoundDescriptor, BoundError, CheckedByteCount, CheckedCount,
    CheckedDuration,
};
pub use ::reovim_foundation::carrier::CarrierSafetyDescriptor;

use crate::carrier::{
    BorrowModel, CopyModel, DropModel, InvocationModel, LifetimeModel,
    OwnershipModel, SharingModel, ThreadTransfer, TypeMismatch, UnsafeBoundary,
    UnwindModel,
};
use reovim_foundation::bound::{BoundValue, ZeroPolicy};

use crate::contract::ContractId;
use crate::event::{CanonicalEventRecordRef, EventSequence};
use crate::lifecycle::LifecycleIdentity;
use crate::operation::{ContractSelection, OperationDescriptor};

/// Nonzero outer completion bounds for activation and shutdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticOutputCompletionBounds {
    activation: CheckedDuration,
    shutdown: CheckedDuration,
}

impl DiagnosticOutputCompletionBounds {
    /// Constructs nonzero hard completion maxima in nanoseconds.
    ///
    /// # Errors
    ///
    /// Returns [`BoundError::ZeroRejected`] for either zero maximum.
    pub const fn new(
        activation_nanoseconds: u64,
        shutdown_nanoseconds: u64,
    ) -> Result<Self, BoundError> {
        let activation = match CheckedDuration::from_nanoseconds(
            activation_nanoseconds,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let shutdown = match CheckedDuration::from_nanoseconds(
            shutdown_nanoseconds,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        Ok(Self { activation, shutdown })
    }

    /// Returns the activation outer completion maximum.
    #[must_use]
    pub const fn activation(self) -> CheckedDuration {
        self.activation
    }

    /// Returns the shutdown outer completion maximum.
    #[must_use]
    pub const fn shutdown(self) -> CheckedDuration {
        self.shutdown
    }
}

/// Hard per-call and aggregate bounds for one Diagnostic Output generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticOutputLimits {
    output: LifecycleIdentity,
    submission_work: BoundDescriptor,
    temporary_bytes: BoundDescriptor,
    record_capacity: BoundDescriptor,
    byte_capacity: BoundDescriptor,
    completion: DiagnosticOutputCompletionBounds,
    shutdown_report_records: CheckedCount,
    shutdown_report_record_bytes: CheckedByteCount,
}

impl DiagnosticOutputLimits {
    /// Constructs output-owned bounds without allocating or doing work.
    ///
    /// Temporary submission bytes are fixed at zero. Shutdown report entry
    /// and source-record byte limits are derived from outstanding capacities.
    ///
    /// # Errors
    ///
    /// Returns a typed zero, unit, or aggregate incoherence.
    pub const fn new(
        output: LifecycleIdentity,
        maximum_submission_work: u64,
        maximum_record_bytes: u64,
        maximum_outstanding_records: u64,
        maximum_outstanding_bytes: u64,
        completion: DiagnosticOutputCompletionBounds,
    ) -> Result<Self, DiagnosticOutputError> {
        let owner = output.instance();
        let submission_work =
            match submission_work_bound(owner, maximum_submission_work) {
                Ok(value) => value,
                Err(error) => return Err(DiagnosticOutputError::Bound(error)),
            };
        let temporary_bytes = match zero_temporary_bytes_bound(owner) {
            Ok(value) => value,
            Err(error) => return Err(DiagnosticOutputError::Bound(error)),
        };
        let record_capacity =
            match record_capacity_bound(owner, maximum_outstanding_records) {
                Ok(value) => value,
                Err(error) => return Err(DiagnosticOutputError::Bound(error)),
            };
        let byte_capacity = match byte_capacity_bound(
            owner,
            maximum_record_bytes,
            maximum_outstanding_bytes,
        ) {
            Ok(value) => value,
            Err(error) => return Err(DiagnosticOutputError::Bound(error)),
        };
        let report_records = match CheckedCount::new(
            maximum_outstanding_records,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(DiagnosticOutputError::Bound(error)),
        };
        let report_bytes = match CheckedByteCount::new(
            maximum_outstanding_bytes,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(DiagnosticOutputError::Bound(error)),
        };
        Ok(Self {
            output,
            submission_work,
            temporary_bytes,
            record_capacity,
            byte_capacity,
            completion,
            shutdown_report_records: report_records,
            shutdown_report_record_bytes: report_bytes,
        })
    }

    /// Returns the complete output generation that owns these bounds.
    #[must_use]
    pub const fn output(&self) -> LifecycleIdentity {
        self.output
    }

    /// Returns the exact maximum local work for one submission.
    #[must_use]
    pub const fn submission_work(&self) -> BoundDescriptor {
        self.submission_work
    }

    /// Returns the exact-zero temporary allocation bound for submission.
    #[must_use]
    pub const fn temporary_bytes(&self) -> BoundDescriptor {
        self.temporary_bytes
    }

    /// Returns the per-record and aggregate outstanding-record bound.
    #[must_use]
    pub const fn record_capacity(&self) -> BoundDescriptor {
        self.record_capacity
    }

    /// Returns the maximum record and aggregate outstanding-byte bound.
    #[must_use]
    pub const fn byte_capacity(&self) -> BoundDescriptor {
        self.byte_capacity
    }

    /// Returns activation and shutdown completion maxima.
    #[must_use]
    pub const fn completion(&self) -> DiagnosticOutputCompletionBounds {
        self.completion
    }

    /// Returns the non-truncating shutdown report entry capacity.
    #[must_use]
    pub const fn shutdown_report_records(&self) -> CheckedCount {
        self.shutdown_report_records
    }

    /// Returns the source-record bytes covered by the shutdown report.
    #[must_use]
    pub const fn shutdown_report_record_bytes(&self) -> CheckedByteCount {
        self.shutdown_report_record_bytes
    }

    const fn lifecycle_matches(&self, output: LifecycleIdentity) -> bool {
        self.output.same_generation(output)
    }
}

/// Preselected treatment for work owned at the first Closing cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticShutdownPolicy {
    /// Attempt bounded completion of work accepted before Closing.
    Drain,
    /// Discard work that has not begun a destination attempt.
    Discard,
}

/// Predicate required before one output generation may publish readiness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticOutputReadinessPredicate {
    /// The current destination and health path admit one bounded record.
    CurrentGenerationAdmission,
}

/// Public Diagnostic Output operation whose classification is requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticOutputOperation {
    /// Inspect the immutable descriptor.
    DescriptorInspection,
    /// Validate and prepare one canonical record for allocation.
    RecordPreparation,
    /// Attempt one bounded local ownership transfer.
    Submission,
    /// Activate the output within its outer completion bound.
    Activation,
    /// Close admission and resolve owned work within the shutdown bound.
    Shutdown,
}

/// Authority-supplied type identity and safety terms for output records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticRecordContract {
    type_id: ContractId,
    safety: CarrierSafetyDescriptor,
}

impl DiagnosticRecordContract {
    /// Groups an independently assigned record type with its safety terms.
    #[must_use]
    pub const fn new(
        type_id: ContractId,
        safety: CarrierSafetyDescriptor,
    ) -> Self {
        Self { type_id, safety }
    }

    /// Returns the independently assigned record carrier identity.
    #[must_use]
    pub const fn type_id(self) -> ContractId {
        self.type_id
    }

    /// Returns the authority-supplied record carrier safety terms.
    #[must_use]
    pub const fn safety(self) -> CarrierSafetyDescriptor {
        self.safety
    }
}

/// Immutable contract, lifecycle, bounds, and policy for one output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticOutputDescriptor {
    contract: ContractSelection,
    lifecycle: LifecycleIdentity,
    owner: LifecycleIdentity,
    source_stream: LifecycleIdentity,
    limits: DiagnosticOutputLimits,
    record_contract: DiagnosticRecordContract,
    shutdown_policy: DiagnosticShutdownPolicy,
}

impl DiagnosticOutputDescriptor {
    /// Constructs one coherent Diagnostic Output declaration.
    ///
    /// # Errors
    ///
    /// Returns a typed limits-lifecycle or carrier declaration failure.
    pub const fn new(
        contract: ContractSelection,
        lifecycle: LifecycleIdentity,
        owner: LifecycleIdentity,
        source_stream: LifecycleIdentity,
        limits: DiagnosticOutputLimits,
        record_contract: DiagnosticRecordContract,
        shutdown_policy: DiagnosticShutdownPolicy,
    ) -> Result<Self, DiagnosticOutputError> {
        if !limits.lifecycle_matches(lifecycle) {
            return Err(DiagnosticOutputError::LimitsLifecycleMismatch);
        }
        if !record_safety_is_exact(contract, record_contract) {
            return Err(DiagnosticOutputError::IncoherentCarrier);
        }
        Ok(Self {
            contract,
            lifecycle,
            owner,
            source_stream,
            limits,
            record_contract,
            shutdown_policy,
        })
    }

    /// Returns the selected output contract and compatible version range.
    #[must_use]
    pub const fn contract(&self) -> ContractSelection {
        self.contract
    }

    /// Returns the exact output lifecycle stamp.
    #[must_use]
    pub const fn lifecycle(&self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Returns the Lifecycle Owner responsible for this output.
    #[must_use]
    pub const fn owner(&self) -> LifecycleIdentity {
        self.owner
    }

    /// Returns the only Event Stream generation accepted as a source.
    #[must_use]
    pub const fn source_stream(&self) -> LifecycleIdentity {
        self.source_stream
    }

    /// Returns all output-owned hard bounds.
    #[must_use]
    pub const fn limits(&self) -> &DiagnosticOutputLimits {
        &self.limits
    }

    /// Returns the enforced safety declaration for an owned record.
    #[must_use]
    pub const fn record_safety(&self) -> CarrierSafetyDescriptor {
        self.record_contract.safety()
    }

    /// Returns the independently assigned output-record carrier identity.
    #[must_use]
    pub const fn record_type(&self) -> ContractId {
        self.record_contract.type_id()
    }

    /// Returns the policy fixed before this generation becomes ready.
    #[must_use]
    pub const fn shutdown_policy(&self) -> DiagnosticShutdownPolicy {
        self.shutdown_policy
    }

    /// Returns the exact predicate required before readiness publication.
    #[must_use]
    pub const fn readiness_predicate(
        &self,
    ) -> DiagnosticOutputReadinessPredicate {
        DiagnosticOutputReadinessPredicate::CurrentGenerationAdmission
    }

    /// Returns the complete six-axis classification for one operation.
    #[must_use]
    pub const fn operation(
        &self,
        operation: DiagnosticOutputOperation,
    ) -> OperationDescriptor {
        match operation {
            DiagnosticOutputOperation::DescriptorInspection
            | DiagnosticOutputOperation::RecordPreparation
            | DiagnosticOutputOperation::Submission => {
                OperationDescriptor::local_inline()
            }
            DiagnosticOutputOperation::Activation
            | DiagnosticOutputOperation::Shutdown => {
                OperationDescriptor::local_deferred()
            }
        }
    }

    /// Reports whether an operation remains valid after degraded readiness.
    #[must_use]
    pub const fn degraded_operation_is_safe(
        &self,
        operation: DiagnosticOutputOperation,
    ) -> bool {
        diagnostic_operation_code(operation) != 0
    }

    /// Validates one requested output contract identity and version.
    ///
    /// # Errors
    ///
    /// Returns typed unsupported-family or incompatible-version failure.
    pub const fn negotiate(
        &self,
        requested_contract: u128,
        requested_version: u64,
    ) -> Result<(), DiagnosticOutputError> {
        if requested_contract != self.contract.identity() {
            return Err(DiagnosticOutputError::UnsupportedContract);
        }
        if !self.contract.supports(requested_version) {
            return Err(DiagnosticOutputError::IncompatibleContract);
        }
        Ok(())
    }

    /// Validates one source and returns its checked allocation size.
    ///
    /// This is a pure pre-allocation check, not lifecycle or submission
    /// authority. [`DiagnosticOutputRecord::bind`] repeats it after copying.
    ///
    /// # Errors
    ///
    /// Returns a typed source-generation or record-byte bound failure.
    pub const fn validate_record(
        &self,
        source: CanonicalEventRecordRef<'_>,
    ) -> Result<CheckedByteCount, DiagnosticRecordError> {
        validate_record(self, source)
    }
}

const fn diagnostic_operation_code(operation: DiagnosticOutputOperation) -> u8 {
    match operation {
        DiagnosticOutputOperation::Activation => 0,
        DiagnosticOutputOperation::DescriptorInspection
        | DiagnosticOutputOperation::RecordPreparation
        | DiagnosticOutputOperation::Submission
        | DiagnosticOutputOperation::Shutdown => 1,
    }
}

/// Failure to construct, negotiate, or validate an output descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticOutputError {
    /// A Foundation bound rejected the declaration.
    Bound(BoundError),
    /// Limits were prepared for another output lifecycle generation.
    LimitsLifecycleMismatch,
    /// The required move-only record carrier declaration is incoherent.
    IncoherentCarrier,
    /// The requested output contract family is unsupported.
    UnsupportedContract,
    /// The requested output contract version is incompatible.
    IncompatibleContract,
}

impl fmt::Display for DiagnosticOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Bound(_) => "Diagnostic Output bound is invalid",
            Self::LimitsLifecycleMismatch => {
                "Diagnostic Output limits lifecycle differs"
            }
            Self::IncoherentCarrier => {
                "Diagnostic Output record carrier is incoherent"
            }
            Self::UnsupportedContract => {
                "Diagnostic Output contract is unsupported"
            }
            Self::IncompatibleContract => {
                "Diagnostic Output contract is incompatible"
            }
        })
    }
}

impl std::error::Error for DiagnosticOutputError {}

/// Failure to prepare or bind one exact canonical output record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticRecordError {
    /// Canonical bytes exceed the output's per-record bound.
    Bound(BoundError),
    /// The canonical record belongs to another Event Stream generation.
    SourceStreamMismatch,
    /// Owned carrier length cannot fit the portable size representation.
    LengthOverflow,
    /// Owned carrier length differs from the prepared canonical length.
    LengthMismatch,
    /// Owned carrier contents differ from the prepared canonical bytes.
    CanonicalMismatch,
}

impl fmt::Display for DiagnosticRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Bound(_) => "Diagnostic record exceeds its byte bound",
            Self::SourceStreamMismatch => {
                "Diagnostic record source Stream differs"
            }
            Self::LengthOverflow => {
                "Diagnostic record length exceeds its size carrier"
            }
            Self::LengthMismatch => "Diagnostic record length differs",
            Self::CanonicalMismatch => "Diagnostic record bytes differ",
        })
    }
}

impl std::error::Error for DiagnosticRecordError {}

/// Capacity-exact immutable bytes owned by one output submission.
pub struct DiagnosticOutputRecord {
    output: LifecycleIdentity,
    stream: LifecycleIdentity,
    sequence: EventSequence,
    size: CheckedByteCount,
    bytes: ::std::boxed::Box<[u8]>,
}

impl DiagnosticOutputRecord {
    /// Binds a byte-identical owned copy to one target descriptor.
    ///
    /// Runtime first calls [`DiagnosticOutputDescriptor::validate_record`]
    /// before allocation, then re-presents the same source here after copying.
    /// This call retains no source borrow after returning.
    ///
    /// # Errors
    ///
    /// Returns the exact supplied carrier with a typed mismatch cause.
    pub fn bind(
        descriptor: &DiagnosticOutputDescriptor,
        source: CanonicalEventRecordRef<'_>,
        bytes: ::std::boxed::Box<[u8]>,
    ) -> Result<Self, DiagnosticRecordRejection> {
        let size = match descriptor.validate_record(source) {
            Ok(value) => value,
            Err(cause) => {
                return Err(DiagnosticRecordRejection { cause, bytes });
            }
        };
        let Ok(actual) = u64::try_from(bytes.len()) else {
            return Err(DiagnosticRecordRejection {
                cause: DiagnosticRecordError::LengthOverflow,
                bytes,
            });
        };
        if actual != size.value() {
            return Err(DiagnosticRecordRejection {
                cause: DiagnosticRecordError::LengthMismatch,
                bytes,
            });
        }
        if bytes.as_ref() != source.bytes() {
            return Err(DiagnosticRecordRejection {
                cause: DiagnosticRecordError::CanonicalMismatch,
                bytes,
            });
        }
        Ok(Self {
            output: descriptor.lifecycle,
            stream: source.stream(),
            sequence: source.sequence(),
            size,
            bytes,
        })
    }

    /// Returns the exact target output lifecycle stamp.
    #[must_use]
    pub const fn output(&self) -> LifecycleIdentity {
        self.output
    }

    /// Returns the complete source Event Stream lifecycle stamp.
    #[must_use]
    pub const fn stream(&self) -> LifecycleIdentity {
        self.stream
    }

    /// Returns the source Event sequence.
    #[must_use]
    pub const fn sequence(&self) -> EventSequence {
        self.sequence
    }

    /// Returns the exact checked canonical byte length.
    #[must_use]
    pub const fn size(&self) -> CheckedByteCount {
        self.size
    }

    /// Borrows the immutable canonical bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for DiagnosticOutputRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiagnosticOutputRecord")
            .field("output", &self.output)
            .field("stream", &self.stream)
            .field("sequence", &self.sequence)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

/// Typed failed bind that returns the exact caller-owned carrier.
pub struct DiagnosticRecordRejection {
    cause: DiagnosticRecordError,
    bytes: ::std::boxed::Box<[u8]>,
}

impl DiagnosticRecordRejection {
    /// Returns the reason finalization rejected the carrier.
    #[must_use]
    pub const fn cause(&self) -> DiagnosticRecordError {
        self.cause
    }

    /// Borrows the exact returned carrier.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns ownership of the exact rejected carrier.
    #[must_use]
    pub fn into_bytes(self) -> ::std::boxed::Box<[u8]> {
        self.bytes
    }
}

impl fmt::Debug for DiagnosticRecordRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiagnosticRecordRejection")
            .field("cause", &self.cause)
            .field("size", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

/// Typed reason one live output did not take record ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticDropCause {
    /// Record identity or metadata is invalid or duplicated.
    InvalidRecord,
    /// The record exceeds a declared hard bound.
    BoundExceeded,
    /// The output generation has not published readiness.
    NotReady,
    /// Non-waiting synchronization authority was unavailable.
    SynchronizationPressure,
    /// Aggregate outstanding-record capacity was exhausted.
    RecordCapacity,
    /// Aggregate outstanding-byte capacity was exhausted.
    ByteCapacity,
    /// Checked aggregate charge arithmetic overflowed.
    ChargeOverflow,
    /// The destination is unavailable while lifecycle remains open.
    DestinationUnavailable,
    /// Output-health accounting is unavailable while lifecycle remains open.
    HealthUnavailable,
    /// Another bounded live-output failure prevented transfer.
    InternalFailure,
}

/// Typed lifecycle reason one output generation rejects new ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticClosedCause {
    /// The output has begun Closing.
    Closing,
    /// The output is Closed.
    Closed,
    /// A newer generation replaced this output.
    Replaced,
    /// A previous bounded shutdown remains incomplete and latched.
    IncompleteShutdown,
}

/// Coarse ownership result of one submission invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticSubmissionStatus {
    /// The output took record ownership.
    Accepted,
    /// A live output did not take ownership.
    Dropped,
    /// Lifecycle fencing prevented ownership transfer.
    Closed,
}

/// Complete ownership result of one bounded submission.
#[must_use]
pub enum DiagnosticSubmissionOutcome {
    /// The output took record ownership within its declared bounds.
    Accepted,
    /// A live output returned record ownership with an exact cause.
    Dropped {
        /// Reason the live output did not take ownership.
        cause: DiagnosticDropCause,
        /// Complete record returned to the caller.
        record: DiagnosticOutputRecord,
    },
    /// A fenced output returned record ownership with an exact cause.
    Closed {
        /// Lifecycle reason admission is fenced.
        cause: DiagnosticClosedCause,
        /// Complete record returned to the caller.
        record: DiagnosticOutputRecord,
    },
}

impl DiagnosticSubmissionOutcome {
    /// Constructs an accepted ownership-transfer result.
    pub const fn accepted() -> Self {
        Self::Accepted
    }

    /// Constructs a typed live-output drop with returned ownership.
    pub const fn dropped(
        cause: DiagnosticDropCause,
        record: DiagnosticOutputRecord,
    ) -> Self {
        Self::Dropped { cause, record }
    }

    /// Constructs a typed lifecycle close with returned ownership.
    pub const fn closed(
        cause: DiagnosticClosedCause,
        record: DiagnosticOutputRecord,
    ) -> Self {
        Self::Closed { cause, record }
    }

    /// Returns the coarse ownership status.
    #[must_use]
    pub const fn status(&self) -> DiagnosticSubmissionStatus {
        match self {
            Self::Accepted => DiagnosticSubmissionStatus::Accepted,
            Self::Dropped { .. } => DiagnosticSubmissionStatus::Dropped,
            Self::Closed { .. } => DiagnosticSubmissionStatus::Closed,
        }
    }

    /// Borrows the returned record for Dropped or Closed.
    #[must_use]
    pub const fn returned_record(&self) -> Option<&DiagnosticOutputRecord> {
        match self {
            Self::Accepted => None,
            Self::Dropped { record, .. } | Self::Closed { record, .. } => {
                Some(record)
            }
        }
    }

    /// Returns record ownership for Dropped or Closed.
    #[must_use]
    pub fn into_returned_record(self) -> Option<DiagnosticOutputRecord> {
        match self {
            Self::Accepted => None,
            Self::Dropped { record, .. } | Self::Closed { record, .. } => {
                Some(record)
            }
        }
    }
}

impl fmt::Debug for DiagnosticSubmissionOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Accepted => formatter.write_str("Accepted"),
            Self::Dropped { cause, record } => formatter
                .debug_struct("Dropped")
                .field("cause", cause)
                .field("record", record)
                .finish(),
            Self::Closed { cause, record } => formatter
                .debug_struct("Closed")
                .field("cause", cause)
                .field("record", record)
                .finish(),
        }
    }
}

/// Semantic port for one bounded local Diagnostic Output ownership transfer.
pub trait DiagnosticOutput {
    /// Returns the immutable output declaration.
    fn descriptor(&self) -> &DiagnosticOutputDescriptor;

    /// Attempts one bounded ownership transfer without allocation or I/O.
    ///
    /// Implementations atomically revalidate target/source generations,
    /// duplicate record identity, capacity, readiness, and lifecycle at the
    /// transfer cut. Every nonaccepted result returns the complete record.
    fn submit(
        &self,
        record: DiagnosticOutputRecord,
    ) -> DiagnosticSubmissionOutcome;
}

const fn validate_record(
    descriptor: &DiagnosticOutputDescriptor,
    source: CanonicalEventRecordRef<'_>,
) -> Result<CheckedByteCount, DiagnosticRecordError> {
    if !descriptor.source_stream.same_generation(source.stream()) {
        return Err(DiagnosticRecordError::SourceStreamMismatch);
    }
    let size = BoundValue::Bytes(source.size());
    match descriptor.limits.byte_capacity.admit_per_object(size) {
        Ok(()) => Ok(source.size()),
        Err(error) => Err(DiagnosticRecordError::Bound(error)),
    }
}

const fn submission_work_bound(
    owner: u128,
    maximum: u64,
) -> Result<BoundDescriptor, BoundError> {
    let value = match CheckedCount::new(maximum, ZeroPolicy::Reject) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    BoundDescriptor::new(
        BoundValue::Count(value),
        BoundValue::Count(value),
        ZeroPolicy::Reject,
        reovim_foundation::identity::InstanceId::new(owner),
    )
}

const fn zero_temporary_bytes_bound(
    owner: u128,
) -> Result<BoundDescriptor, BoundError> {
    let zero = match CheckedByteCount::new(0, ZeroPolicy::Allow) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    BoundDescriptor::new(
        BoundValue::Bytes(zero),
        BoundValue::Bytes(zero),
        ZeroPolicy::Allow,
        reovim_foundation::identity::InstanceId::new(owner),
    )
}

const fn record_capacity_bound(
    owner: u128,
    maximum: u64,
) -> Result<BoundDescriptor, BoundError> {
    let one = match CheckedCount::new(1, ZeroPolicy::Reject) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    let maximum = match CheckedCount::new(maximum, ZeroPolicy::Reject) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    BoundDescriptor::new(
        BoundValue::Count(one),
        BoundValue::Count(maximum),
        ZeroPolicy::Reject,
        reovim_foundation::identity::InstanceId::new(owner),
    )
}

const fn byte_capacity_bound(
    owner: u128,
    record: u64,
    aggregate: u64,
) -> Result<BoundDescriptor, BoundError> {
    let record = match CheckedByteCount::new(record, ZeroPolicy::Reject) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    let aggregate = match CheckedByteCount::new(aggregate, ZeroPolicy::Reject) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    BoundDescriptor::new(
        BoundValue::Bytes(record),
        BoundValue::Bytes(aggregate),
        ZeroPolicy::Reject,
        reovim_foundation::identity::InstanceId::new(owner),
    )
}

const fn record_safety_is_exact(
    contract: ContractSelection,
    record_contract: DiagnosticRecordContract,
) -> bool {
    let record_type = record_contract.type_id();
    let safety = record_contract.safety();
    let versions = safety.schema_versions();
    let memory = safety.memory();
    let concurrency = safety.concurrency();
    let invocation = safety.invocation();
    safety.type_id().value() == record_type.value()
        && versions.minimum().value() == contract.minimum()
        && versions.maximum().value() == contract.maximum()
        && record_memory_code(memory) == 0
        && record_concurrency_code(concurrency) == 0
        && record_invocation_code(invocation) == 0
        && record_mismatch_code(safety.mismatch()) == 0
        && record_boundary_code(safety.unsafe_boundary()) == 0
}

const fn record_memory_code(memory: crate::carrier::MemorySafety) -> u8 {
    match (
        memory.ownership(),
        memory.borrowing(),
        memory.copying(),
        memory.dropping(),
        memory.lifetime(),
    ) {
        (
            OwnershipModel::Owned,
            BorrowModel::Shared,
            CopyModel::MoveOnly,
            DropModel::ReleasesOwnedState,
            LifetimeModel::Value,
        ) => 0,
        _ => 1,
    }
}

const fn record_concurrency_code(
    concurrency: crate::carrier::ConcurrencySafety,
) -> u8 {
    match (concurrency.thread_transfer(), concurrency.sharing()) {
        (ThreadTransfer::Transferable, SharingModel::Immutable) => 0,
        _ => 1,
    }
}

const fn record_invocation_code(
    invocation: crate::carrier::InvocationSafety,
) -> u8 {
    match (invocation.invocation(), invocation.unwind()) {
        (InvocationModel::DataOnly, UnwindModel::NotInvocable) => 0,
        _ => 1,
    }
}

const fn record_mismatch_code(mismatch: TypeMismatch) -> u8 {
    match mismatch {
        TypeMismatch::Reject => 0,
    }
}

const fn record_boundary_code(boundary: UnsafeBoundary) -> u8 {
    match boundary {
        UnsafeBoundary::None => 0,
        UnsafeBoundary::Reviewed(_) => 1,
    }
}
