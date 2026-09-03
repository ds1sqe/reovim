use reovim_foundation::bound::{CheckedByteCount, CheckedCount, ZeroPolicy};

use crate::lifecycle::LifecycleIdentity;

use super::snapshot_validation::validate_snapshot_join;
use super::{
    EventCursor, EventError, RetainedSequenceRange, SaturatingCounter,
    SnapshotCoverage, SnapshotRequest, SnapshotStatistics,
    StartupEventDescriptor,
};

impl crate::event::SnapshotRequest {
    /// Constructs exact caller-owned record and canonical-byte limits.
    ///
    /// `maximum_bytes` bounds canonical record content. Concrete Runtime
    /// carrier capacity has a separate mechanism-owned charge.
    ///
    /// # Errors
    ///
    /// Returns a typed zero or descriptor-exhaustion failure.
    pub const fn new(
        descriptor: &StartupEventDescriptor,
        charge_owner: LifecycleIdentity,
        maximum_records: u64,
        maximum_bytes: u64,
    ) -> Result<Self, EventError> {
        let maximum_records =
            match CheckedCount::new(maximum_records, ZeroPolicy::Reject) {
                Ok(value) => value,
                Err(error) => return Err(EventError::Bound(error)),
            };
        let maximum_bytes =
            match CheckedByteCount::new(maximum_bytes, ZeroPolicy::Reject) {
                Ok(value) => value,
                Err(error) => return Err(EventError::Bound(error)),
            };
        let limits = descriptor.limits();
        if maximum_records.value() > limits.snapshot_records().value()
            || maximum_bytes.value() > limits.snapshot_bytes().value()
        {
            return Err(EventError::SnapshotBoundExceeded);
        }
        Ok(Self { charge_owner, maximum_records, maximum_bytes })
    }

    /// Returns the caller that owns the preallocated output carrier.
    #[must_use]
    pub const fn charge_owner(self) -> LifecycleIdentity {
        self.charge_owner
    }

    /// Returns the caller-owned complete-record capacity.
    #[must_use]
    pub const fn maximum_records(self) -> CheckedCount {
        self.maximum_records
    }

    /// Returns the caller-owned canonical-byte limit.
    #[must_use]
    pub const fn maximum_bytes(self) -> CheckedByteCount {
        self.maximum_bytes
    }

    /// Validates that one complete record's canonical bytes can fit.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::SnapshotBoundExceeded`] without permitting a
    /// partial record.
    pub const fn admit_record_bytes(
        self,
        required_bytes: CheckedByteCount,
    ) -> Result<(), EventError> {
        if required_bytes.value() > self.maximum_bytes.value() {
            return Err(EventError::SnapshotBoundExceeded);
        }
        Ok(())
    }
}

impl crate::event::SnapshotStatistics {
    /// Constructs candidate copied counts and retained eviction totals.
    ///
    /// This validates scalar shape and request limits.
    /// [`super::StartupSnapshot`] validates the exact canonical-byte sum when
    /// it joins complete records.
    ///
    /// # Errors
    ///
    /// Returns a typed bound failure when copied values exceed capacity.
    pub const fn new(
        request: SnapshotRequest,
        copied_records: u64,
        copied_bytes: u64,
        evicted_records: SaturatingCounter,
        evicted_bytes: SaturatingCounter,
    ) -> Result<Self, EventError> {
        let copied_records =
            match CheckedCount::new(copied_records, ZeroPolicy::Allow) {
                Ok(value) => value,
                Err(error) => return Err(EventError::Bound(error)),
            };
        let copied_bytes =
            match CheckedByteCount::new(copied_bytes, ZeroPolicy::Allow) {
                Ok(value) => value,
                Err(error) => return Err(EventError::Bound(error)),
            };
        if copied_records.value() > request.maximum_records().value()
            || copied_bytes.value() > request.maximum_bytes().value()
        {
            return Err(EventError::SnapshotBoundExceeded);
        }
        if (copied_records.value() == 0) != (copied_bytes.value() == 0) {
            return Err(EventError::InvalidSnapshotMetadata);
        }
        if (evicted_records.value() == 0) != (evicted_bytes.value() == 0) {
            return Err(EventError::InvalidSnapshotMetadata);
        }
        Ok(Self {
            copied_records,
            copied_bytes,
            evicted_records,
            evicted_bytes,
        })
    }

    /// Returns the copied complete-record count.
    #[must_use]
    pub const fn copied_records(self) -> CheckedCount {
        self.copied_records
    }

    /// Returns the declared exact copied canonical-byte count.
    #[must_use]
    pub const fn copied_bytes(self) -> CheckedByteCount {
        self.copied_bytes
    }

    /// Returns the total record-eviction counter.
    #[must_use]
    pub const fn evicted_records(self) -> SaturatingCounter {
        self.evicted_records
    }

    /// Returns the total byte-eviction counter.
    #[must_use]
    pub const fn evicted_bytes(self) -> SaturatingCounter {
        self.evicted_bytes
    }
}

impl crate::event::SnapshotMetadata {
    /// Constructs metadata after validating scalar, range, and cursor joins.
    ///
    /// [`super::StartupSnapshot`] performs the content/count join.
    ///
    /// # Errors
    ///
    /// Returns a typed bound, sequence, or inconsistent-coverage failure.
    pub const fn new(
        request: SnapshotRequest,
        stream_incarnation: u128,
        retained: RetainedSequenceRange,
        statistics: SnapshotStatistics,
        next_cursor: EventCursor,
        coverage: SnapshotCoverage,
    ) -> Result<Self, EventError> {
        if statistics.copied_records().value()
            > request.maximum_records().value()
            || statistics.copied_bytes().value()
                > request.maximum_bytes().value()
        {
            return Err(EventError::SnapshotBoundExceeded);
        }
        match validate_snapshot_join(
            stream_incarnation,
            retained,
            statistics.copied_records(),
            next_cursor,
            coverage,
        ) {
            Ok(()) => Ok(Self {
                charge_owner: request.charge_owner(),
                stream_incarnation,
                retained,
                statistics,
                next_cursor,
                coverage,
            }),
            Err(error) => Err(error),
        }
    }

    /// Returns the caller that owns the copied output carrier.
    #[must_use]
    pub const fn charge_owner(self) -> LifecycleIdentity {
        self.charge_owner
    }

    /// Returns the Stream incarnation represented by this snapshot.
    #[must_use]
    pub const fn stream_incarnation(self) -> u128 {
        self.stream_incarnation
    }

    /// Returns the retained range observed at the snapshot boundary.
    #[must_use]
    pub const fn retained(self) -> RetainedSequenceRange {
        self.retained
    }

    /// Returns the copied record count and exact canonical-byte count.
    #[must_use]
    pub const fn statistics(self) -> SnapshotStatistics {
        self.statistics
    }

    /// Returns the stateless cursor after this snapshot.
    #[must_use]
    pub const fn next_cursor(self) -> EventCursor {
        self.next_cursor
    }

    /// Returns whether capacity covered the complete retained range.
    #[must_use]
    pub const fn coverage(self) -> SnapshotCoverage {
        self.coverage
    }
}
