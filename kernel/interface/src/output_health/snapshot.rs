use crate::bound::{CheckedByteCount, CheckedCount, ZeroPolicy};
use crate::event::SaturatingCounter;
use crate::lifecycle::LifecycleIdentity;

use super::{
    OutputHealthAnomalyRecord, OutputHealthCounters, OutputHealthDescriptor,
    OutputHealthError, OutputHealthHighWater, OutputHealthRetainedRange,
    OutputHealthSequence, OutputHealthSequencePosition,
    OutputHealthSnapshotCoverage, OutputHealthSnapshotMetadata,
    OutputHealthSnapshotRequest, OutputHealthSnapshotStatistics,
};

impl crate::output_health::OutputHealthSnapshotRequest {
    /// Constructs caller-owned record and fixed-charge byte limits.
    ///
    /// # Errors
    ///
    /// Returns a typed zero, descriptor-exhaustion, or too-small byte bound.
    pub const fn new(
        descriptor: &OutputHealthDescriptor,
        charge_owner: LifecycleIdentity,
        maximum_records: u64,
        maximum_bytes: u64,
    ) -> Result<Self, OutputHealthError> {
        let maximum_records =
            match CheckedCount::new(maximum_records, ZeroPolicy::Reject) {
                Ok(value) => value,
                Err(error) => return Err(OutputHealthError::Bound(error)),
            };
        let maximum_bytes =
            match CheckedByteCount::new(maximum_bytes, ZeroPolicy::Reject) {
                Ok(value) => value,
                Err(error) => return Err(OutputHealthError::Bound(error)),
            };
        let limits = descriptor.limits();
        let anomaly_bytes = match limits.anomaly_bytes().per_object() {
            crate::bound::BoundValue::Bytes(value) => value,
            crate::bound::BoundValue::Count(_)
            | crate::bound::BoundValue::Duration(_) => {
                return Err(OutputHealthError::InvalidSnapshotMetadata);
            }
        };
        if maximum_records.value() > limits.snapshot_records().value()
            || maximum_bytes.value() > limits.snapshot_bytes().value()
        {
            return Err(OutputHealthError::SnapshotBoundExceeded);
        }
        if maximum_bytes.value() < anomaly_bytes.value() {
            return Err(OutputHealthError::SnapshotBoundExceeded);
        }
        Ok(Self {
            stream: descriptor.stream(),
            charge_owner,
            anomaly_bytes,
            maximum_records,
            maximum_bytes,
        })
    }

    /// Returns the exact health source Stream generation.
    #[must_use]
    pub const fn stream(self) -> LifecycleIdentity {
        self.stream
    }

    /// Returns the caller that owns the future output carrier charge.
    #[must_use]
    pub const fn charge_owner(self) -> LifecycleIdentity {
        self.charge_owner
    }

    /// Returns the fixed charge of each complete anomaly record.
    #[must_use]
    pub const fn anomaly_bytes(self) -> CheckedByteCount {
        self.anomaly_bytes
    }

    /// Returns the caller-owned complete-record limit.
    #[must_use]
    pub const fn maximum_records(self) -> CheckedCount {
        self.maximum_records
    }

    /// Returns the caller-owned fixed-charge byte limit.
    #[must_use]
    pub const fn maximum_bytes(self) -> CheckedByteCount {
        self.maximum_bytes
    }

    /// Validates that one complete anomaly can fit this request.
    ///
    /// # Errors
    ///
    /// Returns [`OutputHealthError::SnapshotBoundExceeded`] without permitting
    /// a partial anomaly.
    pub const fn admit_record(
        self,
        record: OutputHealthAnomalyRecord,
    ) -> Result<(), OutputHealthError> {
        if record.resident_bytes().value() > self.maximum_bytes.value() {
            return Err(OutputHealthError::SnapshotBoundExceeded);
        }
        Ok(())
    }
}

impl crate::output_health::OutputHealthSnapshotStatistics {
    /// Constructs exact copied complete-record and fixed-charge byte counts.
    ///
    /// # Errors
    ///
    /// Returns a typed bound or zero-pair coherence failure.
    pub const fn new(
        request: OutputHealthSnapshotRequest,
        copied_records: u64,
        copied_bytes: u64,
    ) -> Result<Self, OutputHealthError> {
        let copied_records =
            match CheckedCount::new(copied_records, ZeroPolicy::Allow) {
                Ok(value) => value,
                Err(error) => return Err(OutputHealthError::Bound(error)),
            };
        let copied_bytes =
            match CheckedByteCount::new(copied_bytes, ZeroPolicy::Allow) {
                Ok(value) => value,
                Err(error) => return Err(OutputHealthError::Bound(error)),
            };
        if copied_records.value() > request.maximum_records().value()
            || copied_bytes.value() > request.maximum_bytes().value()
        {
            return Err(OutputHealthError::SnapshotBoundExceeded);
        }
        if (copied_records.value() == 0) != (copied_bytes.value() == 0) {
            return Err(OutputHealthError::InvalidSnapshotMetadata);
        }
        Ok(Self { copied_records, copied_bytes })
    }

    /// Returns the number of copied complete anomaly records.
    #[must_use]
    pub const fn copied_records(self) -> CheckedCount {
        self.copied_records
    }

    /// Returns the exact copied fixed-charge byte sum.
    #[must_use]
    pub const fn copied_bytes(self) -> CheckedByteCount {
        self.copied_bytes
    }
}

impl crate::output_health::OutputHealthSnapshotMetadata {
    /// Constructs scalar metadata after validating the state-cut relations.
    ///
    /// [`crate::output_health::OutputHealthSnapshot::new`] performs the
    /// complete-record and eviction joins.
    ///
    /// # Errors
    ///
    /// Returns a typed bound, range, position, or coverage failure.
    pub const fn new(
        request: OutputHealthSnapshotRequest,
        high_water: OutputHealthHighWater,
        retained: OutputHealthRetainedRange,
        statistics: OutputHealthSnapshotStatistics,
        next: OutputHealthSequencePosition,
        coverage: OutputHealthSnapshotCoverage,
    ) -> Result<Self, OutputHealthError> {
        if statistics.copied_records().value()
            > request.maximum_records().value()
            || statistics.copied_bytes().value()
                > request.maximum_bytes().value()
        {
            return Err(OutputHealthError::SnapshotBoundExceeded);
        }
        match validate_metadata(
            high_water, retained, statistics, next, coverage,
        ) {
            Ok(()) => Ok(Self {
                stream: request.stream(),
                charge_owner: request.charge_owner(),
                anomaly_bytes: request.anomaly_bytes(),
                high_water,
                retained,
                statistics,
                next,
                coverage,
            }),
            Err(error) => Err(error),
        }
    }

    /// Returns the exact health source Stream generation.
    #[must_use]
    pub const fn stream(self) -> LifecycleIdentity {
        self.stream
    }

    /// Returns the caller that owns the eventual output carrier charge.
    #[must_use]
    pub const fn charge_owner(self) -> LifecycleIdentity {
        self.charge_owner
    }

    /// Returns the fixed charge of each complete anomaly record.
    #[must_use]
    pub const fn anomaly_bytes(self) -> CheckedByteCount {
        self.anomaly_bytes
    }

    /// Returns the latest sequence assigned before the snapshot cut.
    #[must_use]
    pub const fn high_water(self) -> OutputHealthHighWater {
        self.high_water
    }

    /// Returns the complete retained range observed at the snapshot cut.
    #[must_use]
    pub const fn retained(self) -> OutputHealthRetainedRange {
        self.retained
    }

    /// Returns copied complete-record and fixed-charge byte counts.
    #[must_use]
    pub const fn statistics(self) -> OutputHealthSnapshotStatistics {
        self.statistics
    }

    /// Returns the next sequence after copied coverage.
    #[must_use]
    pub const fn next(self) -> OutputHealthSequencePosition {
        self.next
    }

    /// Returns whether all retained anomalies were copied.
    #[must_use]
    pub const fn coverage(self) -> OutputHealthSnapshotCoverage {
        self.coverage
    }
}

impl<'a> crate::output_health::OutputHealthSnapshot<'a> {
    /// Joins scalar metadata, all counters, and complete borrowed anomalies.
    ///
    /// Construction proves structural coherence only, not live observation.
    ///
    /// # Errors
    ///
    /// Returns a typed count, identity, order, charge, or eviction mismatch.
    pub fn new(
        metadata: OutputHealthSnapshotMetadata,
        counters: &'a OutputHealthCounters,
        records: &'a [OutputHealthAnomalyRecord],
    ) -> Result<Self, OutputHealthError> {
        validate_records(metadata, counters, records)?;
        Ok(Self { metadata, counters, records })
    }

    /// Returns bounded range, high-water, position, and coverage facts.
    #[must_use]
    pub const fn metadata(self) -> OutputHealthSnapshotMetadata {
        self.metadata
    }

    /// Returns the complete fixed counter bank.
    #[must_use]
    pub const fn counters(self) -> &'a OutputHealthCounters {
        self.counters
    }

    /// Returns borrowed complete anomaly records in oldest-first order.
    #[must_use]
    pub const fn records(self) -> &'a [OutputHealthAnomalyRecord] {
        self.records
    }
}

const fn validate_metadata(
    high_water: OutputHealthHighWater,
    retained: OutputHealthRetainedRange,
    statistics: OutputHealthSnapshotStatistics,
    next: OutputHealthSequencePosition,
    coverage: OutputHealthSnapshotCoverage,
) -> Result<(), OutputHealthError> {
    match (high_water.latest(), retained.earliest(), retained.latest()) {
        (None, None, None) => validate_empty(statistics, next, coverage),
        (Some(_), None, None) => {
            validate_empty_with_history(high_water, statistics, next, coverage)
        }
        (Some(high), Some(earliest), Some(latest)) => validate_nonempty(
            high, earliest, latest, statistics, next, coverage,
        ),
        _ => Err(OutputHealthError::InvalidSnapshotMetadata),
    }
}

const fn validate_empty(
    statistics: OutputHealthSnapshotStatistics,
    next: OutputHealthSequencePosition,
    coverage: OutputHealthSnapshotCoverage,
) -> Result<(), OutputHealthError> {
    let high_water = OutputHealthHighWater::empty();
    validate_empty_with_history(high_water, statistics, next, coverage)
}

const fn validate_empty_with_history(
    high_water: OutputHealthHighWater,
    statistics: OutputHealthSnapshotStatistics,
    next: OutputHealthSequencePosition,
    coverage: OutputHealthSnapshotCoverage,
) -> Result<(), OutputHealthError> {
    if statistics.copied_records().value() != 0
        || !positions_equal(next, high_water.next_position())
    {
        return Err(OutputHealthError::InvalidSnapshotMetadata);
    }
    match coverage {
        OutputHealthSnapshotCoverage::Complete => Ok(()),
        OutputHealthSnapshotCoverage::Partial { .. } => {
            Err(OutputHealthError::InvalidSnapshotMetadata)
        }
    }
}

const fn validate_nonempty(
    high: OutputHealthSequence,
    earliest: OutputHealthSequence,
    latest: OutputHealthSequence,
    statistics: OutputHealthSnapshotStatistics,
    next: OutputHealthSequencePosition,
    coverage: OutputHealthSnapshotCoverage,
) -> Result<(), OutputHealthError> {
    if high.value() != latest.value() || latest.is_before(earliest) {
        return Err(OutputHealthError::InvalidSnapshotMetadata);
    }
    let copied = statistics.copied_records().value();
    if copied == 0 {
        return Err(OutputHealthError::SnapshotBoundExceeded);
    }
    let retained = latest.value() - earliest.value() + 1;
    match coverage {
        OutputHealthSnapshotCoverage::Complete => {
            if copied == retained
                && positions_equal(
                    next,
                    OutputHealthHighWater::assigned(high).next_position(),
                )
            {
                return Ok(());
            }
        }
        OutputHealthSnapshotCoverage::Partial { first_uncopied } => {
            if partial_metadata_valid(
                earliest,
                latest,
                copied,
                first_uncopied,
                next,
            ) {
                return Ok(());
            }
        }
    }
    Err(OutputHealthError::InvalidSnapshotMetadata)
}

const fn partial_metadata_valid(
    earliest: OutputHealthSequence,
    latest: OutputHealthSequence,
    copied: u64,
    first_uncopied: OutputHealthSequence,
    next: OutputHealthSequencePosition,
) -> bool {
    let within = !first_uncopied.is_before(earliest)
        && !latest.is_before(first_uncopied);
    let expected = first_uncopied.value() - earliest.value();
    within
        && copied == expected
        && positions_equal(
            next,
            OutputHealthSequencePosition::Next(first_uncopied),
        )
}

fn validate_records(
    metadata: OutputHealthSnapshotMetadata,
    counters: &OutputHealthCounters,
    records: &[OutputHealthAnomalyRecord],
) -> Result<(), OutputHealthError> {
    let count = u64::try_from(records.len())
        .map_err(|_| OutputHealthError::InvalidSnapshotRecords)?;
    if count != metadata.statistics().copied_records().value() {
        return Err(OutputHealthError::InvalidSnapshotRecords);
    }
    validate_record_sequence(metadata, records)?;
    validate_record_bytes(metadata, records)?;
    validate_eviction(metadata, counters.eviction())
}

fn validate_record_sequence(
    metadata: OutputHealthSnapshotMetadata,
    records: &[OutputHealthAnomalyRecord],
) -> Result<(), OutputHealthError> {
    let Some(mut expected) = metadata.retained().earliest() else {
        return if records.is_empty() {
            Ok(())
        } else {
            Err(OutputHealthError::InvalidSnapshotRecords)
        };
    };
    for (index, record) in records.iter().enumerate() {
        if !record.proposal().source_stream().same_generation(metadata.stream())
            || record.sequence() != expected
        {
            return Err(OutputHealthError::InvalidSnapshotRecords);
        }
        if index + 1 < records.len() {
            expected = expected
                .next()
                .map_err(|_| OutputHealthError::InvalidSnapshotRecords)?;
        }
    }
    Ok(())
}

fn validate_record_bytes(
    metadata: OutputHealthSnapshotMetadata,
    records: &[OutputHealthAnomalyRecord],
) -> Result<(), OutputHealthError> {
    let mut total = CheckedByteCount::new(0, ZeroPolicy::Allow)
        .map_err(OutputHealthError::Bound)?;
    let expected_charge = metadata.anomaly_bytes();
    for record in records {
        if record.resident_bytes() != expected_charge {
            return Err(OutputHealthError::InvalidSnapshotRecords);
        }
        total = total
            .checked_add(record.resident_bytes())
            .map_err(OutputHealthError::Bound)?;
    }
    if total != metadata.statistics().copied_bytes() {
        return Err(OutputHealthError::InvalidSnapshotRecords);
    }
    Ok(())
}

fn validate_eviction(
    metadata: OutputHealthSnapshotMetadata,
    eviction: super::OutputHealthEvictionCounters,
) -> Result<(), OutputHealthError> {
    let assigned =
        metadata.high_water().latest().map_or(0, OutputHealthSequence::value);
    let retained = retained_count(metadata.retained());
    let expected_records = assigned
        .checked_sub(retained)
        .ok_or(OutputHealthError::InvalidSnapshotMetadata)?;
    let record_counter = eviction.records();
    if record_counter.saturated() || record_counter.value() != expected_records
    {
        return Err(OutputHealthError::InvalidSnapshotMetadata);
    }
    validate_evicted_bytes(
        eviction.resident_bytes(),
        expected_records,
        metadata.anomaly_bytes().value(),
    )
}

const fn validate_evicted_bytes(
    counter: SaturatingCounter,
    records: u64,
    charge: u64,
) -> Result<(), OutputHealthError> {
    match records.checked_mul(charge) {
        Some(expected) => {
            if counter.value() == expected && !counter.saturated() {
                return Ok(());
            }
        }
        None => {
            if counter.value() == u64::MAX && counter.saturated() {
                return Ok(());
            }
        }
    }
    Err(OutputHealthError::InvalidSnapshotMetadata)
}

const fn retained_count(range: OutputHealthRetainedRange) -> u64 {
    match (range.earliest(), range.latest()) {
        (Some(earliest), Some(latest)) => latest.value() - earliest.value() + 1,
        _ => 0,
    }
}

const fn positions_equal(
    left: OutputHealthSequencePosition,
    right: OutputHealthSequencePosition,
) -> bool {
    match (left, right) {
        (
            OutputHealthSequencePosition::Next(left),
            OutputHealthSequencePosition::Next(right),
        ) => left.value() == right.value(),
        (
            OutputHealthSequencePosition::Exhausted,
            OutputHealthSequencePosition::Exhausted,
        ) => true,
        _ => false,
    }
}
