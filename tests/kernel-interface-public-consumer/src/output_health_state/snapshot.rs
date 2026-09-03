use reovim_kernel_interface::event::SaturatingCounter;
use reovim_kernel_interface::output_health::{
    OutputHealthDescriptor, OutputHealthError, OutputHealthHighWater,
    OutputHealthRetainedRange, OutputHealthSequence,
    OutputHealthSequencePosition, OutputHealthSnapshot,
    OutputHealthSnapshotCoverage, OutputHealthSnapshotMetadata,
    OutputHealthSnapshotRequest, OutputHealthSnapshotStatistics,
};

use super::{
    ANOMALY_BYTES, make_anomaly, make_health, next_position_is,
    output_bound_error, output_error_is,
};
use crate::output_health_state::counters::{
    counter, make_counters, make_counters_with,
};

pub(super) fn exercise() -> bool {
    request_and_statistics()
        && pristine_snapshot()
        && complete_snapshot()
        && partial_snapshot()
        && fully_evicted_snapshot()
        && exhausted_snapshot()
        && metadata_rejections()
        && record_rejections()
}

fn request_and_statistics() -> bool {
    let Some((health, output)) = make_health() else {
        return false;
    };
    let Ok(request) = request(&health) else {
        return false;
    };
    let Some(record) = make_anomaly(&health, &output, 1) else {
        return false;
    };
    let Ok(empty) = OutputHealthSnapshotStatistics::new(request, 0, 0) else {
        return false;
    };
    request.stream().same_generation(health.stream())
        && request.charge_owner().same_generation(health.lifecycle_owner())
        && request.anomaly_bytes().value() == ANOMALY_BYTES
        && request.maximum_records().value() == 4
        && request.maximum_bytes().value() == 512
        && request.admit_record(record).is_ok()
        && empty.copied_records().value() == 0
        && empty.copied_bytes().value() == 0
        && output_bound_error(&OutputHealthSnapshotRequest::new(
            &health,
            health.lifecycle_owner(),
            0,
            512,
        ))
        && output_error_is(
            &OutputHealthSnapshotRequest::new(
                &health,
                health.lifecycle_owner(),
                5,
                512,
            ),
            OutputHealthError::SnapshotBoundExceeded,
        )
        && output_error_is(
            &OutputHealthSnapshotRequest::new(
                &health,
                health.lifecycle_owner(),
                1,
                ANOMALY_BYTES - 1,
            ),
            OutputHealthError::SnapshotBoundExceeded,
        )
        && output_error_is(
            &OutputHealthSnapshotStatistics::new(request, 0, ANOMALY_BYTES),
            OutputHealthError::InvalidSnapshotMetadata,
        )
        && output_error_is(
            &OutputHealthSnapshotStatistics::new(request, 5, 512),
            OutputHealthError::SnapshotBoundExceeded,
        )
}

fn pristine_snapshot() -> bool {
    let Some((health, _)) = make_health() else {
        return false;
    };
    let Ok(request) = request(&health) else {
        return false;
    };
    let Ok(statistics) = OutputHealthSnapshotStatistics::new(request, 0, 0)
    else {
        return false;
    };
    let Ok(metadata) = OutputHealthSnapshotMetadata::new(
        request,
        OutputHealthHighWater::empty(),
        OutputHealthRetainedRange::empty(),
        statistics,
        OutputHealthSequencePosition::Next(OutputHealthSequence::first()),
        OutputHealthSnapshotCoverage::Complete,
    ) else {
        return false;
    };
    let Some(counters) = make_counters(0, 0) else {
        return false;
    };
    let Ok(snapshot) = OutputHealthSnapshot::new(metadata, &counters, &[])
    else {
        return false;
    };
    snapshot.metadata() == metadata
        && *snapshot.counters() == counters
        && snapshot.records().is_empty()
        && crate::traits(snapshot)
}

fn complete_snapshot() -> bool {
    let Some((health, output)) = make_health() else {
        return false;
    };
    let (Some(first), Some(second)) =
        (make_anomaly(&health, &output, 3), make_anomaly(&health, &output, 4))
    else {
        return false;
    };
    let records = [first, second];
    let Some(counters) = make_counters(2, 2 * ANOMALY_BYTES) else {
        return false;
    };
    let Some(metadata) = complete_metadata(&health, 4, 3, 4, 2) else {
        return false;
    };
    let Ok(snapshot) = OutputHealthSnapshot::new(metadata, &counters, &records)
    else {
        return false;
    };
    let Ok(next) = OutputHealthSequence::new(5) else {
        return false;
    };
    snapshot.records() == records
        && snapshot.metadata().high_water().latest()
            == OutputHealthSequence::new(4).ok()
        && snapshot.metadata().retained().earliest()
            == OutputHealthSequence::new(3).ok()
        && next_position_is(snapshot.metadata().next(), next)
}

fn partial_snapshot() -> bool {
    let Some((health, output)) = make_health() else {
        return false;
    };
    let Some(first) = make_anomaly(&health, &output, 3) else {
        return false;
    };
    let records = [first];
    let Ok(request) = request(&health) else {
        return false;
    };
    let Ok(statistics) =
        OutputHealthSnapshotStatistics::new(request, 1, ANOMALY_BYTES)
    else {
        return false;
    };
    let Some(range) = range(3, 4) else {
        return false;
    };
    let Ok(first_uncopied) = OutputHealthSequence::new(4) else {
        return false;
    };
    let Ok(metadata) = OutputHealthSnapshotMetadata::new(
        request,
        high_water(4),
        range,
        statistics,
        OutputHealthSequencePosition::Next(first_uncopied),
        OutputHealthSnapshotCoverage::Partial { first_uncopied },
    ) else {
        return false;
    };
    let Some(counters) = make_counters(2, 2 * ANOMALY_BYTES) else {
        return false;
    };
    OutputHealthSnapshot::new(metadata, &counters, &records).is_ok()
}

fn fully_evicted_snapshot() -> bool {
    let Some((health, _)) = make_health() else {
        return false;
    };
    let Ok(request) = request(&health) else {
        return false;
    };
    let Ok(statistics) = OutputHealthSnapshotStatistics::new(request, 0, 0)
    else {
        return false;
    };
    let Ok(next) = OutputHealthSequence::new(5) else {
        return false;
    };
    let Ok(metadata) = OutputHealthSnapshotMetadata::new(
        request,
        high_water(4),
        OutputHealthRetainedRange::empty(),
        statistics,
        OutputHealthSequencePosition::Next(next),
        OutputHealthSnapshotCoverage::Complete,
    ) else {
        return false;
    };
    let Some(counters) = make_counters(4, 4 * ANOMALY_BYTES) else {
        return false;
    };
    OutputHealthSnapshot::new(metadata, &counters, &[]).is_ok()
}

fn exhausted_snapshot() -> bool {
    let Some((health, output)) = make_health() else {
        return false;
    };
    let Some(record) = make_anomaly(&health, &output, u64::MAX) else {
        return false;
    };
    let records = [record];
    let Some(range) = range(u64::MAX, u64::MAX) else {
        return false;
    };
    let Ok(request) = request(&health) else {
        return false;
    };
    let Ok(statistics) =
        OutputHealthSnapshotStatistics::new(request, 1, ANOMALY_BYTES)
    else {
        return false;
    };
    let Ok(metadata) = OutputHealthSnapshotMetadata::new(
        request,
        high_water(u64::MAX),
        range,
        statistics,
        OutputHealthSequencePosition::Exhausted,
        OutputHealthSnapshotCoverage::Complete,
    ) else {
        return false;
    };
    let Some(evicted_records) = counter(u64::MAX - 1) else {
        return false;
    };
    let Ok(evicted_bytes) = SaturatingCounter::from_parts(u64::MAX, true)
    else {
        return false;
    };
    let Some(counters) = make_counters_with(evicted_records, evicted_bytes)
    else {
        return false;
    };
    OutputHealthSnapshot::new(metadata, &counters, &records).is_ok()
}

fn metadata_rejections() -> bool {
    let Some((health, _)) = make_health() else {
        return false;
    };
    let Ok(request) = request(&health) else {
        return false;
    };
    let Ok(empty) = OutputHealthSnapshotStatistics::new(request, 0, 0) else {
        return false;
    };
    let Ok(one) =
        OutputHealthSnapshotStatistics::new(request, 1, ANOMALY_BYTES)
    else {
        return false;
    };
    let Some(range) = range(1, 1) else {
        return false;
    };
    let first = OutputHealthSequence::first();
    let absent_high_water = OutputHealthSnapshotMetadata::new(
        request,
        OutputHealthHighWater::empty(),
        range,
        one,
        OutputHealthSequencePosition::Next(first),
        OutputHealthSnapshotCoverage::Complete,
    );
    let empty_partial = OutputHealthSnapshotMetadata::new(
        request,
        high_water(1),
        OutputHealthRetainedRange::empty(),
        empty,
        OutputHealthSequencePosition::Next(first),
        OutputHealthSnapshotCoverage::Partial { first_uncopied: first },
    );
    let zero_nonempty = OutputHealthSnapshotMetadata::new(
        request,
        high_water(1),
        range,
        empty,
        OutputHealthSequencePosition::Next(first),
        OutputHealthSnapshotCoverage::Complete,
    );
    let stale_latest = OutputHealthSnapshotMetadata::new(
        request,
        high_water(2),
        range,
        one,
        OutputHealthSequencePosition::Next(first),
        OutputHealthSnapshotCoverage::Complete,
    );
    output_error_is(
        &absent_high_water,
        OutputHealthError::InvalidSnapshotMetadata,
    ) && output_error_is(
        &empty_partial,
        OutputHealthError::InvalidSnapshotMetadata,
    ) && output_error_is(
        &zero_nonempty,
        OutputHealthError::SnapshotBoundExceeded,
    ) && output_error_is(
        &stale_latest,
        OutputHealthError::InvalidSnapshotMetadata,
    )
}

fn record_rejections() -> bool {
    let Some((health, output)) = make_health() else {
        return false;
    };
    let (Some(first), Some(second)) =
        (make_anomaly(&health, &output, 3), make_anomaly(&health, &output, 4))
    else {
        return false;
    };
    let Some(metadata) = complete_metadata(&health, 4, 3, 4, 2) else {
        return false;
    };
    let Some(counters) = make_counters(2, 2 * ANOMALY_BYTES) else {
        return false;
    };
    let Some(wrong_eviction) = make_counters(1, ANOMALY_BYTES) else {
        return false;
    };
    let Ok(saturated_record) = SaturatingCounter::from_parts(u64::MAX, true)
    else {
        return false;
    };
    let Some(saturated_eviction) =
        make_counters_with(saturated_record, saturated_record)
    else {
        return false;
    };
    let reversed = [second, first];
    let one_record = [first];
    output_error_is(
        &OutputHealthSnapshot::new(metadata, &counters, &one_record),
        OutputHealthError::InvalidSnapshotRecords,
    ) && output_error_is(
        &OutputHealthSnapshot::new(metadata, &counters, &reversed),
        OutputHealthError::InvalidSnapshotRecords,
    ) && output_error_is(
        &OutputHealthSnapshot::new(metadata, &wrong_eviction, &[first, second]),
        OutputHealthError::InvalidSnapshotMetadata,
    ) && output_error_is(
        &OutputHealthSnapshot::new(
            metadata,
            &saturated_eviction,
            &[first, second],
        ),
        OutputHealthError::InvalidSnapshotMetadata,
    )
}

const fn request(
    health: &OutputHealthDescriptor,
) -> Result<OutputHealthSnapshotRequest, OutputHealthError> {
    OutputHealthSnapshotRequest::new(health, health.lifecycle_owner(), 4, 512)
}

fn complete_metadata(
    health: &OutputHealthDescriptor,
    high: u64,
    earliest: u64,
    latest: u64,
    copied: u64,
) -> Option<OutputHealthSnapshotMetadata> {
    let request = request(health).ok()?;
    let statistics = OutputHealthSnapshotStatistics::new(
        request,
        copied,
        copied.checked_mul(ANOMALY_BYTES)?,
    )
    .ok()?;
    let range = range(earliest, latest)?;
    let high_water = high_water(high);
    let next = high_water.next_position();
    OutputHealthSnapshotMetadata::new(
        request,
        high_water,
        range,
        statistics,
        next,
        OutputHealthSnapshotCoverage::Complete,
    )
    .ok()
}

fn range(earliest: u64, latest: u64) -> Option<OutputHealthRetainedRange> {
    OutputHealthRetainedRange::non_empty(
        OutputHealthSequence::new(earliest).ok()?,
        OutputHealthSequence::new(latest).ok()?,
    )
    .ok()
}

const fn high_water(latest: u64) -> OutputHealthHighWater {
    let Ok(sequence) = OutputHealthSequence::new(latest) else {
        return OutputHealthHighWater::empty();
    };
    OutputHealthHighWater::assigned(sequence)
}
