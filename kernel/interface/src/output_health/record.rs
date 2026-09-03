use core::fmt;

use crate::bound::{
    BoundDescriptor, BoundError, BoundUnit, BoundValue, CheckedByteCount,
    CheckedCount, ZeroPolicy,
};
use crate::diagnostic::DiagnosticOutputDescriptor;
use crate::identity::InstanceId;
use crate::operation::OperationDescriptor;

use super::{
    OutputHealthDescriptor, OutputHealthError, OutputHealthLimits,
    OutputHealthProposal, OutputHealthProposalScope, OutputHealthSequence,
    OutputHealthSequencePosition, OutputHealthSnapshotLimits,
};

impl crate::output_health::OutputHealthSnapshotLimits {
    /// Constructs nonzero per-snapshot record and charged-byte maxima.
    ///
    /// Retained-capacity coherence is checked by [`OutputHealthLimits::new`].
    ///
    /// # Errors
    ///
    /// Returns a typed zero failure for either maximum.
    pub const fn new(records: u64, bytes: u64) -> Result<Self, BoundError> {
        let records = match CheckedCount::new(records, ZeroPolicy::Reject) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let bytes = match CheckedByteCount::new(bytes, ZeroPolicy::Reject) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        Ok(Self { records, bytes })
    }

    /// Returns the complete-record maximum for one snapshot.
    #[must_use]
    pub const fn records(self) -> CheckedCount {
        self.records
    }

    /// Returns the fixed-charge byte maximum for one snapshot.
    #[must_use]
    pub const fn bytes(self) -> CheckedByteCount {
        self.bytes
    }
}

impl crate::output_health::OutputHealthLimits {
    /// Constructs exact work, storage, snapshot, and import bounds.
    ///
    /// Fixed bytes and the anomaly-slot product determine the complete
    /// partition resident ceiling. Snapshot maxima must fit retained capacity
    /// and permit at least one complete anomaly.
    ///
    /// # Errors
    ///
    /// Returns a typed zero, arithmetic, bound, or snapshot-coherence failure.
    pub fn new(
        stream: crate::lifecycle::LifecycleIdentity,
        maximum_record_work: u64,
        fixed_resident_bytes: u64,
        anomaly_record_bytes: u64,
        maximum_retained_anomalies: u64,
        snapshot: OutputHealthSnapshotLimits,
        maximum_import_attempts: u64,
    ) -> Result<Self, OutputHealthError> {
        let owner = InstanceId::new(stream.instance());
        let record_work = count_bound(
            maximum_record_work,
            maximum_record_work,
            ZeroPolicy::Reject,
            owner,
        )?;
        let temporary_bytes = byte_bound(0, 0, ZeroPolicy::Allow, owner)?;
        let fixed_resident_bytes = byte_bound(
            fixed_resident_bytes,
            fixed_resident_bytes,
            ZeroPolicy::Reject,
            owner,
        )?;
        let anomaly_records = count_bound(
            1,
            maximum_retained_anomalies,
            ZeroPolicy::Reject,
            owner,
        )?;
        let aggregate_anomaly_bytes =
            checked_product(anomaly_record_bytes, maximum_retained_anomalies)?;
        let anomaly_bytes = byte_bound(
            anomaly_record_bytes,
            aggregate_anomaly_bytes,
            ZeroPolicy::Reject,
            owner,
        )?;
        let aggregate_resident_bytes = checked_sum(
            fixed_resident_bytes.aggregate().value(),
            aggregate_anomaly_bytes,
        )?;
        let resident_bytes = byte_bound(
            aggregate_resident_bytes,
            aggregate_resident_bytes,
            ZeroPolicy::Reject,
            owner,
        )?;
        let import_attempts =
            checked_count(maximum_import_attempts, ZeroPolicy::Reject)?;
        if snapshot_limits_incoherent(snapshot, anomaly_records, anomaly_bytes)
        {
            return Err(OutputHealthError::IncoherentSnapshotLimits);
        }
        Ok(Self {
            stream,
            record_work,
            temporary_bytes,
            fixed_resident_bytes,
            anomaly_records,
            anomaly_bytes,
            resident_bytes,
            snapshot,
            import_attempts,
        })
    }

    /// Returns the Event Stream generation that owns these bounds.
    #[must_use]
    pub const fn stream(self) -> crate::lifecycle::LifecycleIdentity {
        self.stream
    }

    /// Returns the exact maximum local work for one record operation.
    #[must_use]
    pub const fn record_work(self) -> BoundDescriptor {
        self.record_work
    }

    /// Returns the exact-zero temporary allocation bound for recording.
    #[must_use]
    pub const fn temporary_bytes(self) -> BoundDescriptor {
        self.temporary_bytes
    }

    /// Returns the fixed resident charge outside anomaly slots.
    #[must_use]
    pub const fn fixed_resident_bytes(self) -> BoundDescriptor {
        self.fixed_resident_bytes
    }

    /// Returns the per-slot and aggregate retained-anomaly count bound.
    #[must_use]
    pub const fn anomaly_records(self) -> BoundDescriptor {
        self.anomaly_records
    }

    /// Returns the fixed-slot and aggregate anomaly resident-byte bound.
    #[must_use]
    pub const fn anomaly_bytes(self) -> BoundDescriptor {
        self.anomaly_bytes
    }

    /// Returns the complete partition resident-byte ceiling.
    #[must_use]
    pub const fn resident_bytes(self) -> BoundDescriptor {
        self.resident_bytes
    }

    /// Returns the descriptor's per-snapshot complete-record ceiling.
    #[must_use]
    pub const fn snapshot_records(self) -> CheckedCount {
        self.snapshot.records()
    }

    /// Returns the descriptor's per-snapshot charged-byte ceiling.
    #[must_use]
    pub const fn snapshot_bytes(self) -> CheckedByteCount {
        self.snapshot.bytes()
    }

    /// Returns both nonzero per-snapshot maxima.
    #[must_use]
    pub const fn snapshot(self) -> OutputHealthSnapshotLimits {
        self.snapshot
    }

    /// Returns the future aggregate health-import attempt ceiling.
    #[must_use]
    pub const fn import_attempts(self) -> CheckedCount {
        self.import_attempts
    }

    const fn stream_matches(
        self,
        stream: crate::lifecycle::LifecycleIdentity,
    ) -> bool {
        self.stream.same_generation(stream)
    }
}

impl crate::output_health::OutputHealthDescriptor {
    /// Constructs one coherent Event Stream-owned `OutputHealth` declaration.
    ///
    /// # Errors
    ///
    /// Returns [`OutputHealthError::LimitsStreamMismatch`] when limits name
    /// another Event Stream generation.
    pub const fn new(
        contract: crate::operation::ContractSelection,
        stream: crate::lifecycle::LifecycleIdentity,
        lifecycle_owner: crate::lifecycle::LifecycleIdentity,
        limits: OutputHealthLimits,
    ) -> Result<Self, OutputHealthError> {
        if !limits.stream_matches(stream) {
            return Err(OutputHealthError::LimitsStreamMismatch);
        }
        Ok(Self { contract, stream, lifecycle_owner, limits })
    }

    /// Returns the selected `OutputHealth` contract.
    #[must_use]
    pub const fn contract(self) -> crate::operation::ContractSelection {
        self.contract
    }

    /// Returns the exact source Event Stream generation.
    #[must_use]
    pub const fn stream(self) -> crate::lifecycle::LifecycleIdentity {
        self.stream
    }

    /// Returns the Event Stream Lifecycle Owner.
    #[must_use]
    pub const fn lifecycle_owner(self) -> crate::lifecycle::LifecycleIdentity {
        self.lifecycle_owner
    }

    /// Returns every `OutputHealth` hard bound.
    #[must_use]
    pub const fn limits(self) -> OutputHealthLimits {
        self.limits
    }

    /// Returns the exact record-operation classification.
    #[must_use]
    pub const fn record_operation(self) -> OperationDescriptor {
        OperationDescriptor::local_inline()
    }

    /// Returns the exact owner-snapshot operation classification.
    #[must_use]
    pub const fn snapshot_operation(self) -> OperationDescriptor {
        OperationDescriptor::local_inline()
    }

    /// Validates one requested `OutputHealth` contract identity and version.
    ///
    /// # Errors
    ///
    /// Returns typed unsupported-family or incompatible-version failure.
    pub const fn negotiate(
        self,
        requested_contract: u128,
        requested_version: u64,
    ) -> Result<(), OutputHealthError> {
        if requested_contract != self.contract.identity() {
            return Err(OutputHealthError::UnsupportedContract);
        }
        if !self.contract.supports(requested_version) {
            return Err(OutputHealthError::IncompatibleContract);
        }
        Ok(())
    }
}

impl crate::output_health::OutputHealthSequence {
    /// Returns the first sequence assignable to an accepted anomaly.
    #[must_use]
    pub const fn first() -> Self {
        Self(1)
    }

    /// Constructs one already assigned health sequence.
    ///
    /// # Errors
    ///
    /// Returns [`OutputHealthError::UnassignedSequence`] for zero.
    pub const fn new(value: u64) -> Result<Self, OutputHealthError> {
        if value == 0 {
            return Err(OutputHealthError::UnassignedSequence);
        }
        Ok(Self(value))
    }

    /// Returns the assigned sequence scalar.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Reports whether this sequence precedes `other`.
    #[must_use]
    pub const fn is_before(self, other: Self) -> bool {
        self.0 < other.0
    }

    /// Computes the next health sequence without wrapping.
    ///
    /// # Errors
    ///
    /// Returns [`OutputHealthError::SequenceExhausted`] at [`u64::MAX`].
    pub const fn next(self) -> Result<Self, OutputHealthError> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(OutputHealthError::SequenceExhausted),
        }
    }
}

impl crate::output_health::OutputHealthHighWater {
    /// Returns a high-water mark before any anomaly was assigned.
    #[must_use]
    pub const fn empty() -> Self {
        Self { latest: None }
    }

    /// Returns a high-water mark containing the latest assigned anomaly.
    #[must_use]
    pub const fn assigned(latest: OutputHealthSequence) -> Self {
        Self { latest: Some(latest) }
    }

    /// Returns the latest assigned sequence, or `None` before assignment.
    #[must_use]
    pub const fn latest(self) -> Option<OutputHealthSequence> {
        self.latest
    }

    /// Returns the next assignable position after this state cut.
    #[must_use]
    pub const fn next_position(self) -> OutputHealthSequencePosition {
        let Some(latest) = self.latest else {
            return OutputHealthSequencePosition::Next(
                OutputHealthSequence::first(),
            );
        };
        match latest.next() {
            Ok(next) => OutputHealthSequencePosition::Next(next),
            Err(OutputHealthError::SequenceExhausted) => {
                OutputHealthSequencePosition::Exhausted
            }
            Err(_) => OutputHealthSequencePosition::Exhausted,
        }
    }
}

impl crate::output_health::OutputHealthRetainedRange {
    /// Returns an explicitly empty retained range.
    #[must_use]
    pub const fn empty() -> Self {
        Self { earliest: None, latest: None }
    }

    /// Constructs one inclusive nonempty retained range.
    ///
    /// # Errors
    ///
    /// Returns [`OutputHealthError::InvalidRetainedRange`] for reverse order.
    pub const fn non_empty(
        earliest: OutputHealthSequence,
        latest: OutputHealthSequence,
    ) -> Result<Self, OutputHealthError> {
        if latest.is_before(earliest) {
            return Err(OutputHealthError::InvalidRetainedRange);
        }
        Ok(Self { earliest: Some(earliest), latest: Some(latest) })
    }

    /// Returns the earliest retained sequence, or `None` when empty.
    #[must_use]
    pub const fn earliest(self) -> Option<OutputHealthSequence> {
        self.earliest
    }

    /// Returns the latest retained sequence, or `None` when empty.
    #[must_use]
    pub const fn latest(self) -> Option<OutputHealthSequence> {
        self.latest
    }

    /// Reports whether no health anomaly is retained.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.earliest.is_none()
    }
}

impl crate::output_health::OutputHealthLeaseScope {
    /// Derives one per-output health scope from coherent descriptors.
    ///
    /// # Errors
    ///
    /// Returns [`OutputHealthError::SourceStreamMismatch`] when the output
    /// consumes another Event Stream generation.
    pub const fn new(
        health: &OutputHealthDescriptor,
        output: &DiagnosticOutputDescriptor,
    ) -> Result<Self, OutputHealthError> {
        if !health.stream.same_generation(output.source_stream()) {
            return Err(OutputHealthError::SourceStreamMismatch);
        }
        Ok(Self {
            stream: health.stream,
            lifecycle_owner: health.lifecycle_owner,
            output: output.lifecycle(),
        })
    }

    /// Returns the exact health source Stream generation.
    #[must_use]
    pub const fn stream(self) -> crate::lifecycle::LifecycleIdentity {
        self.stream
    }

    /// Returns the health state Lifecycle Owner.
    #[must_use]
    pub const fn lifecycle_owner(self) -> crate::lifecycle::LifecycleIdentity {
        self.lifecycle_owner
    }

    /// Returns the only `DiagnosticOutput` generation admitted by this scope.
    #[must_use]
    pub const fn output(self) -> crate::lifecycle::LifecycleIdentity {
        self.output
    }

    /// Classifies proposal identity without observing live state.
    #[must_use]
    pub const fn classify(
        self,
        proposal: OutputHealthProposal,
    ) -> OutputHealthProposalScope {
        let output = proposal.output();
        let stream = proposal.source_stream();
        if !self.output.same_incarnation(output)
            || !self.stream.same_incarnation(stream)
        {
            return OutputHealthProposalScope::Invalid;
        }
        if !self.output.same_generation(output)
            || !self.stream.same_generation(stream)
        {
            return OutputHealthProposalScope::Stale;
        }
        OutputHealthProposalScope::Current
    }
}

impl crate::output_health::OutputHealthAnomalyRecord {
    /// Constructs a coherent complete anomaly candidate.
    ///
    /// # Errors
    ///
    /// Returns a source mismatch before rejecting a counter-only proposal.
    pub const fn new(
        descriptor: &OutputHealthDescriptor,
        sequence: OutputHealthSequence,
        proposal: OutputHealthProposal,
    ) -> Result<Self, OutputHealthError> {
        if !descriptor.stream.same_generation(proposal.source_stream()) {
            return Err(OutputHealthError::SourceStreamMismatch);
        }
        if !proposal.is_anomaly_candidate() {
            return Err(OutputHealthError::CounterOnlyProposal);
        }
        let resident_bytes = match descriptor.limits.anomaly_bytes.per_object()
        {
            BoundValue::Bytes(value) => value,
            BoundValue::Count(_) | BoundValue::Duration(_) => {
                return Err(OutputHealthError::InvalidSnapshotMetadata);
            }
        };
        Ok(Self { sequence, proposal, resident_bytes })
    }

    /// Returns the assigned health sequence.
    #[must_use]
    pub const fn sequence(self) -> OutputHealthSequence {
        self.sequence
    }

    /// Returns the complete copied-scalar proposal.
    #[must_use]
    pub const fn proposal(self) -> OutputHealthProposal {
        self.proposal
    }

    /// Returns the fixed contract charge for this anomaly slot.
    #[must_use]
    pub const fn resident_bytes(self) -> CheckedByteCount {
        self.resident_bytes
    }
}

impl fmt::Display for OutputHealthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Bound(_) => "OutputHealth bound is invalid",
            Self::LimitsStreamMismatch => "OutputHealth limits Stream differs",
            Self::IncoherentSnapshotLimits => {
                "OutputHealth snapshot limits are incoherent"
            }
            Self::UnsupportedContract => "OutputHealth contract is unsupported",
            Self::IncompatibleContract => {
                "OutputHealth contract is incompatible"
            }
            Self::UnassignedSequence => {
                "OutputHealth sequence must be assigned"
            }
            Self::SequenceExhausted => "OutputHealth sequence is exhausted",
            Self::SourceStreamMismatch => "OutputHealth source Stream differs",
            Self::CounterOnlyProposal => {
                "counter-only proposal is not a health anomaly"
            }
            Self::InvalidRetainedRange => {
                "OutputHealth retained range is invalid"
            }
            Self::SnapshotBoundExceeded => {
                "OutputHealth snapshot bound is exceeded"
            }
            Self::InvalidSnapshotMetadata => {
                "OutputHealth snapshot metadata is incoherent"
            }
            Self::InvalidSnapshotRecords => {
                "OutputHealth snapshot records are incoherent"
            }
        })
    }
}

impl std::error::Error for OutputHealthError {}

const fn snapshot_limits_incoherent(
    snapshot: OutputHealthSnapshotLimits,
    anomaly_records: BoundDescriptor,
    anomaly_bytes: BoundDescriptor,
) -> bool {
    snapshot.records().value() > anomaly_records.aggregate().value()
        || snapshot.bytes().value() > anomaly_bytes.aggregate().value()
        || snapshot.bytes().value() < anomaly_bytes.per_object().value()
}

const fn checked_product(
    left: u64,
    right: u64,
) -> Result<u64, OutputHealthError> {
    let Some(value) = left.checked_mul(right) else {
        return Err(OutputHealthError::Bound(BoundError::ArithmeticOverflow {
            unit: BoundUnit::Bytes,
            left,
            right,
        }));
    };
    Ok(value)
}

const fn checked_sum(left: u64, right: u64) -> Result<u64, OutputHealthError> {
    let Some(value) = left.checked_add(right) else {
        return Err(OutputHealthError::Bound(BoundError::ArithmeticOverflow {
            unit: BoundUnit::Bytes,
            left,
            right,
        }));
    };
    Ok(value)
}

const fn checked_count(
    value: u64,
    zero: ZeroPolicy,
) -> Result<CheckedCount, OutputHealthError> {
    match CheckedCount::new(value, zero) {
        Ok(value) => Ok(value),
        Err(error) => Err(OutputHealthError::Bound(error)),
    }
}

const fn checked_bytes(
    value: u64,
    zero: ZeroPolicy,
) -> Result<CheckedByteCount, OutputHealthError> {
    match CheckedByteCount::new(value, zero) {
        Ok(value) => Ok(value),
        Err(error) => Err(OutputHealthError::Bound(error)),
    }
}

fn count_bound(
    per_object: u64,
    aggregate: u64,
    zero: ZeroPolicy,
    owner: InstanceId,
) -> Result<BoundDescriptor, OutputHealthError> {
    let per_object = checked_count(per_object, zero)?;
    let aggregate = checked_count(aggregate, zero)?;
    make_bound(
        BoundValue::Count(per_object),
        BoundValue::Count(aggregate),
        zero,
        owner,
    )
}

fn byte_bound(
    per_object: u64,
    aggregate: u64,
    zero: ZeroPolicy,
    owner: InstanceId,
) -> Result<BoundDescriptor, OutputHealthError> {
    let per_object = checked_bytes(per_object, zero)?;
    let aggregate = checked_bytes(aggregate, zero)?;
    make_bound(
        BoundValue::Bytes(per_object),
        BoundValue::Bytes(aggregate),
        zero,
        owner,
    )
}

const fn make_bound(
    per_object: BoundValue,
    aggregate: BoundValue,
    zero: ZeroPolicy,
    owner: InstanceId,
) -> Result<BoundDescriptor, OutputHealthError> {
    match BoundDescriptor::new(per_object, aggregate, zero, owner) {
        Ok(value) => Ok(value),
        Err(error) => Err(OutputHealthError::Bound(error)),
    }
}
