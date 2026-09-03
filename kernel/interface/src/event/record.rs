use super::canonical::{
    StartupHealthCode, StartupOutcomeCode, StartupRollbackCode,
    parse_startup_line,
};
use super::{
    BootstrapHealth, CanonicalEventBytes, EventAdmissionScope, EventError,
    EventSequence, EventTimestamp, ReadinessOutcome, RetainedSequenceRange,
    StartupEventDescriptor, StartupEventRecord, StartupProposal,
};

impl<'a> crate::event::StartupEventRecord<'a> {
    /// Constructs one fully validated startup record value.
    ///
    /// Construction does not grant admission, consume a sequence, or commit
    /// the record. Only a [`super::StartupEventAdmission`] implementation owns
    /// that atomic authority.
    ///
    /// # Errors
    ///
    /// Returns a typed scope, proposal, clock, or canonical-field mismatch.
    pub fn new(
        descriptor: &StartupEventDescriptor,
        scope: EventAdmissionScope,
        sequence: EventSequence,
        timestamp: EventTimestamp,
        proposal: StartupProposal,
        canonical: CanonicalEventBytes<'a>,
    ) -> Result<Self, EventError> {
        validate_record(
            descriptor, scope, sequence, timestamp, proposal, canonical,
        )?;
        Ok(Self {
            scope,
            family: reovim_foundation::contract::ContractId::new(
                descriptor.identities().family(),
            ),
            schema_version: descriptor.family_schema_version(),
            sequence,
            timestamp,
            proposal,
            canonical,
        })
    }

    /// Returns the editor-stamped dispatch scope.
    #[must_use]
    pub const fn scope(self) -> EventAdmissionScope {
        self.scope
    }

    /// Returns the stable startup family identity scalar.
    #[must_use]
    pub const fn family(self) -> u128 {
        self.family.value()
    }

    /// Returns the fixed startup family schema version.
    #[must_use]
    pub const fn schema_version(self) -> u64 {
        self.schema_version
    }

    /// Returns the candidate sequence assigned at the eventual commit cut.
    #[must_use]
    pub const fn sequence(self) -> EventSequence {
        self.sequence
    }

    /// Returns the descriptor-bound monotonic Event timestamp.
    #[must_use]
    pub const fn timestamp(self) -> EventTimestamp {
        self.timestamp
    }

    /// Returns the validated startup fields and derived classification.
    #[must_use]
    pub const fn proposal(self) -> StartupProposal {
        self.proposal
    }

    /// Returns the exact canonical startup bytes.
    #[must_use]
    pub const fn canonical(self) -> CanonicalEventBytes<'a> {
        self.canonical
    }
}

impl<'a> crate::event::StartupSnapshot<'a> {
    /// Joins caller-owned complete record copies to snapshot metadata.
    ///
    /// # Errors
    ///
    /// Returns a typed count, charge, Stream, or sequence mismatch.
    pub fn new(
        descriptor: &StartupEventDescriptor,
        metadata: super::SnapshotMetadata,
        records: &'a [StartupEventRecord<'a>],
    ) -> Result<Self, EventError> {
        validate_snapshot_records(descriptor, metadata, records)?;
        Ok(Self { metadata, records })
    }

    /// Returns the bounded snapshot metadata and cursor.
    #[must_use]
    pub const fn metadata(self) -> super::SnapshotMetadata {
        self.metadata
    }

    /// Returns caller-owned complete startup record copies.
    #[must_use]
    pub const fn records(self) -> &'a [StartupEventRecord<'a>] {
        self.records
    }
}

fn validate_record(
    descriptor: &StartupEventDescriptor,
    scope: EventAdmissionScope,
    sequence: EventSequence,
    timestamp: EventTimestamp,
    proposal: StartupProposal,
    canonical: CanonicalEventBytes<'_>,
) -> Result<(), EventError> {
    if !scope.matches_descriptor(descriptor) {
        return Err(EventError::AdmissionScopeMismatch);
    }
    proposal.validate_for_record(descriptor)?;
    descriptor
        .clock()
        .validate_stamp(timestamp.instant())
        .map_err(EventError::Clock)?;
    let line = parse_startup_line(descriptor, canonical)?;
    if line_matches(line, scope, sequence, timestamp, proposal) {
        return Ok(());
    }
    Err(EventError::InvalidStartupRecord)
}

const fn line_matches(
    line: super::canonical::StartupLine,
    scope: EventAdmissionScope,
    sequence: EventSequence,
    timestamp: EventTimestamp,
    proposal: StartupProposal,
) -> bool {
    let counts = proposal.counts();
    line.stream_incarnation == scope.stream().incarnation()
        && line.sequence.value() == sequence.value()
        && line.timestamp_nanoseconds == timestamp.elapsed_nanoseconds()
        && line.producer == scope.producer()
        && line.correlation == scope.correlation()
        && outcome_matches(line.outcome, proposal.readiness())
        && rollback_matches(line.rollback, proposal.readiness())
        && line.active == counts.active().value()
        && line.degraded == counts.degraded().value()
        && line.unavailable == counts.unavailable().value()
        && health_matches(line.health, proposal.bootstrap_health())
}

const fn outcome_matches(
    actual: StartupOutcomeCode,
    expected: ReadinessOutcome,
) -> bool {
    startup_outcome_code(actual) == readiness_code(expected)
}

const fn health_matches(
    actual: StartupHealthCode,
    expected: BootstrapHealth,
) -> bool {
    startup_health_code(actual) == bootstrap_health_code(expected)
}

const fn startup_outcome_code(value: StartupOutcomeCode) -> u8 {
    match value {
        StartupOutcomeCode::Ready => 0,
        StartupOutcomeCode::DiagnosticsDegraded => 1,
        StartupOutcomeCode::ActivationFailed => 2,
    }
}

const fn readiness_code(value: ReadinessOutcome) -> u8 {
    match value {
        ReadinessOutcome::Ready => 0,
        ReadinessOutcome::DiagnosticsDegraded => 1,
        ReadinessOutcome::ActivationFailed(_) => 2,
    }
}

const fn rollback_matches(
    actual: StartupRollbackCode,
    readiness: ReadinessOutcome,
) -> bool {
    startup_rollback_code(actual) == readiness_rollback_code(readiness)
}

const fn startup_rollback_code(value: StartupRollbackCode) -> u8 {
    match value {
        StartupRollbackCode::None => 0,
        StartupRollbackCode::Reverted => 1,
        StartupRollbackCode::IncompleteShutdown => 2,
    }
}

const fn readiness_rollback_code(value: ReadinessOutcome) -> u8 {
    match value {
        ReadinessOutcome::Ready | ReadinessOutcome::DiagnosticsDegraded => 0,
        ReadinessOutcome::ActivationFailed(
            crate::lifecycle::RollbackOutcome::Reverted,
        ) => 1,
        ReadinessOutcome::ActivationFailed(
            crate::lifecycle::RollbackOutcome::IncompleteShutdown,
        ) => 2,
    }
}

const fn startup_health_code(value: StartupHealthCode) -> u8 {
    match value {
        StartupHealthCode::Clear => 0,
        StartupHealthCode::Imported => 1,
        StartupHealthCode::Pending => 2,
    }
}

const fn bootstrap_health_code(value: BootstrapHealth) -> u8 {
    match value {
        BootstrapHealth::Clear => 0,
        BootstrapHealth::Imported => 1,
        BootstrapHealth::Pending => 2,
    }
}

fn validate_snapshot_records(
    descriptor: &StartupEventDescriptor,
    metadata: super::SnapshotMetadata,
    records: &[StartupEventRecord<'_>],
) -> Result<(), EventError> {
    let Ok(record_count) = u64::try_from(records.len()) else {
        return Err(EventError::SnapshotBoundExceeded);
    };
    if record_count != metadata.statistics().copied_records().value() {
        return Err(EventError::InvalidSnapshotMetadata);
    }
    if metadata.stream_incarnation()
        != descriptor.identities().stream().incarnation()
    {
        return Err(EventError::InvalidSnapshotMetadata);
    }
    validate_snapshot_record_descriptors(descriptor, records)?;
    validate_record_sequence(metadata, records)?;
    validate_canonical_byte_count(metadata, records)
}

fn validate_snapshot_record_descriptors(
    descriptor: &StartupEventDescriptor,
    records: &[StartupEventRecord<'_>],
) -> Result<(), EventError> {
    for record in records {
        validate_record(
            descriptor,
            record.scope(),
            record.sequence(),
            record.timestamp(),
            record.proposal(),
            record.canonical(),
        )?;
        if record.family() != descriptor.identities().family()
            || record.schema_version() != descriptor.family_schema_version()
        {
            return Err(EventError::InvalidSnapshotMetadata);
        }
    }
    Ok(())
}

fn validate_record_sequence(
    metadata: super::SnapshotMetadata,
    records: &[StartupEventRecord<'_>],
) -> Result<(), EventError> {
    let range = metadata.retained();
    let mut expected = range.earliest();
    for record in records {
        let Some(sequence) = expected else {
            return Err(EventError::InvalidSnapshotMetadata);
        };
        if record.scope().stream().incarnation()
            != metadata.stream_incarnation()
            || record.sequence().value() != sequence.value()
        {
            return Err(EventError::InvalidSnapshotMetadata);
        }
        expected = sequence.next().ok();
    }
    validate_sequence_end(range, records, metadata.coverage())
}

fn validate_sequence_end(
    range: RetainedSequenceRange,
    records: &[StartupEventRecord<'_>],
    coverage: super::SnapshotCoverage,
) -> Result<(), EventError> {
    if range.is_empty() && records.is_empty() {
        return Ok(());
    }
    let Some(last) = records.last() else {
        return Ok(());
    };
    match coverage {
        super::SnapshotCoverage::Complete => {
            if Some(last.sequence()) == range.latest() {
                return Ok(());
            }
        }
        super::SnapshotCoverage::Partial { first_uncopied } => {
            if last.sequence().next().ok() == Some(first_uncopied) {
                return Ok(());
            }
        }
    }
    Err(EventError::InvalidSnapshotMetadata)
}

fn validate_canonical_byte_count(
    metadata: super::SnapshotMetadata,
    records: &[StartupEventRecord<'_>],
) -> Result<(), EventError> {
    let mut canonical_bytes = 0_u64;
    for record in records {
        let Some(total) =
            canonical_bytes.checked_add(record.canonical().size().value())
        else {
            return Err(EventError::SnapshotBoundExceeded);
        };
        canonical_bytes = total;
    }
    if canonical_bytes == metadata.statistics().copied_bytes().value() {
        return Ok(());
    }
    Err(EventError::InvalidSnapshotMetadata)
}
