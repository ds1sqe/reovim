use reovim_foundation::bound::{
    BoundDescriptor, BoundValue, CheckedByteCount, CheckedCount, ZeroPolicy,
};
use reovim_foundation::identity::InstanceId;

use super::{EVENT_RING_MAX_BYTES, EventError, STARTUP_CANONICAL_MAX_BYTES};

impl crate::event::StartupEventLimits {
    /// Constructs unit-correct limits from Kernel Interface scalar inputs.
    ///
    /// # Errors
    ///
    /// Returns a typed zero, arithmetic, coherence, or bound failure.
    pub const fn new(
        stream_instance: u128,
        component_per_object: u64,
        component_aggregate: u64,
        synchronous_work: u64,
        temporary_bytes: u64,
        snapshot_records: u64,
        snapshot_bytes: u64,
    ) -> Result<Self, EventError> {
        let owner = InstanceId::new(stream_instance);
        let component_counts = match count_bound(
            component_per_object,
            component_aggregate,
            ZeroPolicy::Allow,
            owner,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let synchronous_work = match count_bound(
            synchronous_work,
            synchronous_work,
            ZeroPolicy::Reject,
            owner,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let temporary_bytes = match byte_bound(
            temporary_bytes,
            temporary_bytes,
            ZeroPolicy::Reject,
            owner,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let canonical_bytes = match byte_bound(
            STARTUP_CANONICAL_MAX_BYTES,
            STARTUP_CANONICAL_MAX_BYTES,
            ZeroPolicy::Reject,
            owner,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let ring_bytes = match byte_bound(
            EVENT_RING_MAX_BYTES,
            EVENT_RING_MAX_BYTES,
            ZeroPolicy::Reject,
            owner,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let snapshot_records =
            match CheckedCount::new(snapshot_records, ZeroPolicy::Reject) {
                Ok(value) => value,
                Err(error) => return Err(EventError::Bound(error)),
            };
        let snapshot_bytes =
            match CheckedByteCount::new(snapshot_bytes, ZeroPolicy::Reject) {
                Ok(value) => value,
                Err(error) => return Err(EventError::Bound(error)),
            };
        Ok(Self {
            component_counts,
            synchronous_work,
            temporary_bytes,
            canonical_bytes,
            ring_bytes,
            snapshot_records,
            snapshot_bytes,
        })
    }

    /// Returns the active/degraded/unavailable count bound.
    #[must_use]
    pub const fn component_counts(&self) -> BoundDescriptor {
        self.component_counts
    }

    /// Returns the synchronous admission-work bound.
    #[must_use]
    pub const fn synchronous_work(&self) -> BoundDescriptor {
        self.synchronous_work
    }

    /// Returns the temporary admission-byte bound.
    #[must_use]
    pub const fn temporary_bytes(&self) -> BoundDescriptor {
        self.temporary_bytes
    }

    /// Returns the per-record and aggregate canonical-byte bound.
    #[must_use]
    pub const fn canonical_bytes(&self) -> BoundDescriptor {
        self.canonical_bytes
    }

    /// Returns the exact 1 MiB resident Event Ring byte ceiling.
    #[must_use]
    pub const fn ring_bytes(&self) -> BoundDescriptor {
        self.ring_bytes
    }

    /// Returns the descriptor's per-snapshot record ceiling.
    #[must_use]
    pub const fn snapshot_records(&self) -> CheckedCount {
        self.snapshot_records
    }

    /// Returns the descriptor's per-snapshot canonical-byte ceiling.
    #[must_use]
    pub const fn snapshot_bytes(&self) -> CheckedByteCount {
        self.snapshot_bytes
    }

    pub(super) const fn validate_owner(
        &self,
        stream_instance: u128,
    ) -> Result<(), EventError> {
        let owners = [
            self.component_counts.charge_owner().value(),
            self.synchronous_work.charge_owner().value(),
            self.temporary_bytes.charge_owner().value(),
            self.canonical_bytes.charge_owner().value(),
            self.ring_bytes.charge_owner().value(),
        ];
        if owners[0] == stream_instance
            && owners[1] == stream_instance
            && owners[2] == stream_instance
            && owners[3] == stream_instance
            && owners[4] == stream_instance
        {
            return Ok(());
        }
        Err(EventError::ChargeOwnerMismatch)
    }
}

const fn count_bound(
    per_object: u64,
    aggregate: u64,
    zero: ZeroPolicy,
    owner: InstanceId,
) -> Result<BoundDescriptor, EventError> {
    let per_object = match CheckedCount::new(per_object, zero) {
        Ok(value) => value,
        Err(error) => return Err(EventError::Bound(error)),
    };
    let aggregate = match CheckedCount::new(aggregate, zero) {
        Ok(value) => value,
        Err(error) => return Err(EventError::Bound(error)),
    };
    make_bound(
        BoundValue::Count(per_object),
        BoundValue::Count(aggregate),
        zero,
        owner,
    )
}

const fn byte_bound(
    per_object: u64,
    aggregate: u64,
    zero: ZeroPolicy,
    owner: InstanceId,
) -> Result<BoundDescriptor, EventError> {
    let per_object = match CheckedByteCount::new(per_object, zero) {
        Ok(value) => value,
        Err(error) => return Err(EventError::Bound(error)),
    };
    let aggregate = match CheckedByteCount::new(aggregate, zero) {
        Ok(value) => value,
        Err(error) => return Err(EventError::Bound(error)),
    };
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
) -> Result<BoundDescriptor, EventError> {
    match BoundDescriptor::new(per_object, aggregate, zero, owner) {
        Ok(value) => Ok(value),
        Err(error) => Err(EventError::Bound(error)),
    }
}
