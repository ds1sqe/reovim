use reovim_kernel_interface::event::{
    EventCursor, EventError, EventSequence, RetainedSequenceRange,
    STARTUP_CANONICAL_MAX_BYTES, SaturatingCounter, SnapshotCoverage,
    SnapshotMetadata, SnapshotRequest, SnapshotStatistics,
    StartupEventDescriptor, StartupEventRecord, StartupSnapshot,
};

use super::{READY_LINE, STREAM_INCARNATION, lifecycle_owner, make_descriptor};

pub const fn const_cases() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    request_cases(&descriptor)
        && empty_metadata_case(&descriptor)
        && partial_metadata_case(&descriptor)
        && maximum_metadata_case(&descriptor)
        && nonfitting_first_record_case(&descriptor)
        && invalid_metadata_cases(&descriptor)
}

const fn request_cases(descriptor: &StartupEventDescriptor) -> bool {
    let Ok(request) =
        SnapshotRequest::new(descriptor, lifecycle_owner(), 4, 2_048)
    else {
        return false;
    };
    let large = descriptor.limits().snapshot_bytes();
    let Ok(small) = SnapshotRequest::new(descriptor, lifecycle_owner(), 1, 1)
    else {
        return false;
    };

    request.charge_owner().same_generation(lifecycle_owner())
        && request.maximum_records().value() == 4
        && request.maximum_bytes().value() == 2_048
        && request.admit_record_bytes(large).is_ok()
        && unit_code(small.admit_record_bytes(large)) == 13
        && request_code(SnapshotRequest::new(
            descriptor,
            lifecycle_owner(),
            5,
            2_048,
        )) == 13
        && request_code(SnapshotRequest::new(
            descriptor,
            lifecycle_owner(),
            0,
            1,
        )) == 1
}

const fn empty_metadata_case(descriptor: &StartupEventDescriptor) -> bool {
    let Ok(request) =
        SnapshotRequest::new(descriptor, lifecycle_owner(), 4, 2_048)
    else {
        return false;
    };
    let zero = SaturatingCounter::zero();
    let Ok(statistics) = SnapshotStatistics::new(request, 0, 0, zero, zero)
    else {
        return false;
    };
    let first = EventSequence::first();
    let cursor = EventCursor::new(STREAM_INCARNATION, first);
    let Ok(metadata) = SnapshotMetadata::new(
        request,
        STREAM_INCARNATION,
        RetainedSequenceRange::empty(),
        statistics,
        cursor,
        SnapshotCoverage::Complete,
    ) else {
        return false;
    };

    statistics.copied_records().value() == 0
        && statistics.copied_bytes().value() == 0
        && statistics.evicted_records().value() == 0
        && statistics.evicted_bytes().value() == 0
        && metadata.charge_owner().same_generation(lifecycle_owner())
        && metadata.stream_incarnation() == STREAM_INCARNATION
        && metadata.retained().is_empty()
        && metadata.statistics().copied_records().value() == 0
        && super::sequence::position_code(
            metadata.next_cursor().position(),
            first,
        ) == 1
        && coverage_code(metadata.coverage()) == 0
}

const fn partial_metadata_case(descriptor: &StartupEventDescriptor) -> bool {
    let Ok(request) =
        SnapshotRequest::new(descriptor, lifecycle_owner(), 4, 2_048)
    else {
        return false;
    };
    let zero = SaturatingCounter::zero();
    let Ok(statistics) = SnapshotStatistics::new(request, 1, 1, zero, zero)
    else {
        return false;
    };
    let first = EventSequence::first();
    let Ok(second) = first.next() else { return false };
    let Ok(third) = second.next() else { return false };
    let Ok(retained) = RetainedSequenceRange::non_empty(first, third) else {
        return false;
    };
    let coverage = SnapshotCoverage::Partial { first_uncopied: second };
    let cursor = EventCursor::new(STREAM_INCARNATION, second);
    match SnapshotMetadata::new(
        request,
        STREAM_INCARNATION,
        retained,
        statistics,
        cursor,
        coverage,
    ) {
        Ok(metadata) => coverage_code(metadata.coverage()) == 1,
        Err(_) => false,
    }
}

const fn maximum_metadata_case(descriptor: &StartupEventDescriptor) -> bool {
    let Ok(request) = SnapshotRequest::new(descriptor, lifecycle_owner(), 1, 1)
    else {
        return false;
    };
    let zero = SaturatingCounter::zero();
    let Ok(statistics) = SnapshotStatistics::new(request, 1, 1, zero, zero)
    else {
        return false;
    };
    let Ok(maximum) = EventSequence::new(u64::MAX) else {
        return false;
    };
    let Ok(retained) = RetainedSequenceRange::non_empty(maximum, maximum)
    else {
        return false;
    };
    SnapshotMetadata::new(
        request,
        STREAM_INCARNATION,
        retained,
        statistics,
        EventCursor::exhausted(STREAM_INCARNATION),
        SnapshotCoverage::Complete,
    )
    .is_ok()
}

const fn nonfitting_first_record_case(
    descriptor: &StartupEventDescriptor,
) -> bool {
    let Ok(request) = SnapshotRequest::new(descriptor, lifecycle_owner(), 1, 1)
    else {
        return false;
    };
    let zero = SaturatingCounter::zero();
    let Ok(statistics) = SnapshotStatistics::new(request, 0, 0, zero, zero)
    else {
        return false;
    };
    let first = EventSequence::first();
    let Ok(retained) = RetainedSequenceRange::non_empty(first, first) else {
        return false;
    };
    let cursor = EventCursor::new(STREAM_INCARNATION, first);
    let coverage = SnapshotCoverage::Partial { first_uncopied: first };
    metadata_code(SnapshotMetadata::new(
        request,
        STREAM_INCARNATION,
        retained,
        statistics,
        cursor,
        coverage,
    )) == 13
}

const fn invalid_metadata_cases(descriptor: &StartupEventDescriptor) -> bool {
    let Ok(request) = SnapshotRequest::new(descriptor, lifecycle_owner(), 1, 1)
    else {
        return false;
    };
    let zero = SaturatingCounter::zero();
    let Ok(one) = SaturatingCounter::from_parts(1, false) else {
        return false;
    };
    let inconsistent_copy = SnapshotStatistics::new(request, 1, 0, zero, zero);
    let inconsistent_eviction =
        SnapshotStatistics::new(request, 0, 0, one, zero);
    let Ok(statistics) = SnapshotStatistics::new(request, 0, 0, zero, zero)
    else {
        return false;
    };
    let cursor = EventCursor::new(0xff, EventSequence::first());
    let metadata = SnapshotMetadata::new(
        request,
        STREAM_INCARNATION,
        RetainedSequenceRange::empty(),
        statistics,
        cursor,
        SnapshotCoverage::Complete,
    );

    statistics_code(inconsistent_copy) == 14
        && statistics_code(inconsistent_eviction) == 14
        && metadata_code(metadata) == 14
}

const fn request_code(result: Result<SnapshotRequest, EventError>) -> u8 {
    match result {
        Err(error) => super::outcomes::event_error_code(error),
        Ok(_) => 0,
    }
}

const fn statistics_code(result: Result<SnapshotStatistics, EventError>) -> u8 {
    match result {
        Err(error) => super::outcomes::event_error_code(error),
        Ok(_) => 0,
    }
}

const fn metadata_code(result: Result<SnapshotMetadata, EventError>) -> u8 {
    match result {
        Err(error) => super::outcomes::event_error_code(error),
        Ok(_) => 0,
    }
}

const fn unit_code(result: Result<(), EventError>) -> u8 {
    match result {
        Err(error) => super::outcomes::event_error_code(error),
        Ok(()) => 0,
    }
}

const fn coverage_code(value: SnapshotCoverage) -> u8 {
    match value {
        SnapshotCoverage::Complete => 0,
        SnapshotCoverage::Partial { .. } => 1,
    }
}

pub fn exercise() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = super::canonical::make_ready_record(&descriptor) else {
        return false;
    };
    let Ok(line_size) = u64::try_from(READY_LINE.len()) else {
        return false;
    };
    let Some(metadata) = complete_metadata(&descriptor, line_size) else {
        return false;
    };
    let records = [record];
    let Ok(snapshot) = StartupSnapshot::new(&descriptor, metadata, &records)
    else {
        return false;
    };

    snapshot.metadata().statistics().copied_records().value() == 1
        && snapshot.metadata().statistics().copied_bytes().value() == line_size
        && snapshot.records().len() == 1
        && snapshot.records()[0].sequence().value() == 1
        && snapshot_clone_copy(&snapshot)
        && snapshot_record_mismatch(&descriptor, metadata)
        && canonical_byte_mismatch(&descriptor, &record, line_size)
}

fn canonical_byte_mismatch(
    descriptor: &StartupEventDescriptor,
    record: &StartupEventRecord<'static>,
    line_size: u64,
) -> bool {
    let Some(underreported) = line_size.checked_sub(1) else {
        return false;
    };
    let Some(overreported) = line_size.checked_add(1) else {
        return false;
    };
    let Some(under_metadata) = complete_metadata(descriptor, underreported)
    else {
        return false;
    };
    let Some(over_metadata) = complete_metadata(descriptor, overreported)
    else {
        return false;
    };
    let records = core::slice::from_ref(record);

    invalid_snapshot(StartupSnapshot::new(descriptor, under_metadata, records))
        && invalid_snapshot(StartupSnapshot::new(
            descriptor,
            over_metadata,
            records,
        ))
}

const fn invalid_snapshot(
    result: Result<StartupSnapshot<'_>, EventError>,
) -> bool {
    match result {
        Err(error) => super::outcomes::event_error_code(error) == 14,
        Ok(_) => false,
    }
}

fn complete_metadata(
    descriptor: &StartupEventDescriptor,
    line_size: u64,
) -> Option<SnapshotMetadata> {
    let Ok(request) = SnapshotRequest::new(
        descriptor,
        lifecycle_owner(),
        1,
        STARTUP_CANONICAL_MAX_BYTES,
    ) else {
        return None;
    };
    let zero = SaturatingCounter::zero();
    let Ok(statistics) =
        SnapshotStatistics::new(request, 1, line_size, zero, zero)
    else {
        return None;
    };
    let first = EventSequence::first();
    let Ok(retained) = RetainedSequenceRange::non_empty(first, first) else {
        return None;
    };
    let Ok(next) = first.next() else { return None };
    SnapshotMetadata::new(
        request,
        STREAM_INCARNATION,
        retained,
        statistics,
        EventCursor::new(STREAM_INCARNATION, next),
        SnapshotCoverage::Complete,
    )
    .ok()
}

fn snapshot_clone_copy(snapshot: &StartupSnapshot<'_>) -> bool {
    let cloned = super::super::clone_value(snapshot);
    let (left, right) = super::super::copy_pair(cloned);
    left.metadata() == right.metadata()
        && left.records().len() == right.records().len()
}

fn snapshot_record_mismatch(
    descriptor: &StartupEventDescriptor,
    metadata: SnapshotMetadata,
) -> bool {
    match StartupSnapshot::new(descriptor, metadata, &[]) {
        Err(error) => super::outcomes::event_error_code(error) == 14,
        Ok(_) => false,
    }
}
