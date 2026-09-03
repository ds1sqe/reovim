use reovim_foundation::bound::CheckedCount;

use super::cursor::{
    complete_cursor_valid, empty_cursor_valid, next_cursor_matches,
};
use super::{
    EventCursor, EventError, EventSequence, RetainedSequenceRange,
    SnapshotCoverage,
};

pub(super) const fn validate_snapshot_join(
    stream_incarnation: u128,
    retained: RetainedSequenceRange,
    copied_records: CheckedCount,
    next_cursor: EventCursor,
    coverage: SnapshotCoverage,
) -> Result<(), EventError> {
    if next_cursor.stream_incarnation() != stream_incarnation {
        return Err(EventError::InvalidSnapshotMetadata);
    }
    match (retained.earliest(), retained.latest()) {
        (None, None) => validate_empty(copied_records, next_cursor, coverage),
        (Some(earliest), Some(latest)) => validate_nonempty(
            earliest,
            latest,
            copied_records,
            next_cursor,
            coverage,
        ),
        _ => Err(EventError::InvalidSnapshotMetadata),
    }
}

const fn validate_empty(
    copied: CheckedCount,
    cursor: EventCursor,
    coverage: SnapshotCoverage,
) -> Result<(), EventError> {
    if copied.value() != 0 || !empty_cursor_valid(cursor) {
        return Err(EventError::InvalidSnapshotMetadata);
    }
    match coverage {
        SnapshotCoverage::Complete => Ok(()),
        SnapshotCoverage::Partial { .. } => {
            Err(EventError::InvalidSnapshotMetadata)
        }
    }
}

const fn validate_nonempty(
    earliest: EventSequence,
    latest: EventSequence,
    copied: CheckedCount,
    cursor: EventCursor,
    coverage: SnapshotCoverage,
) -> Result<(), EventError> {
    if latest.is_before(earliest) {
        return Err(EventError::InvalidRetainedRange);
    }
    if copied.value() == 0 {
        return Err(EventError::SnapshotBoundExceeded);
    }
    match coverage {
        SnapshotCoverage::Complete => {
            let expected = latest.value() - earliest.value() + 1;
            if copied.value() == expected
                && complete_cursor_valid(latest, cursor)
            {
                return Ok(());
            }
        }
        SnapshotCoverage::Partial { first_uncopied } => {
            let within = !first_uncopied.is_before(earliest)
                && !latest.is_before(first_uncopied);
            let expected = first_uncopied.value() - earliest.value();
            if within
                && copied.value() == expected
                && next_cursor_matches(cursor, first_uncopied)
            {
                return Ok(());
            }
        }
    }
    Err(EventError::InvalidSnapshotMetadata)
}
