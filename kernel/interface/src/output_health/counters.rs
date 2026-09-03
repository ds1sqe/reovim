use crate::diagnostic::{DiagnosticClosedCause, DiagnosticDropCause};
use crate::effect::EffectCertainty;
use crate::event::SaturatingCounter;

use super::{
    OutputHealthAcceptedCounters, OutputHealthClosedCounters,
    OutputHealthDropCounters, OutputHealthEffectCounters,
    OutputHealthEvictionCounters, OutputHealthImportCounterKind,
    OutputHealthImportCounters, OutputHealthRecordResultCounters,
    OutputHealthRecordResultKind,
};

impl crate::output_health::OutputHealthAcceptedCounters {
    /// Groups accepted `DiagnosticOutput` record and byte counters.
    #[must_use]
    pub const fn new(
        records: SaturatingCounter,
        record_bytes: SaturatingCounter,
    ) -> Self {
        Self { records, record_bytes }
    }

    /// Returns accepted `DiagnosticOutput` records.
    #[must_use]
    pub const fn records(self) -> SaturatingCounter {
        self.records
    }

    /// Returns accepted canonical record bytes.
    #[must_use]
    pub const fn record_bytes(self) -> SaturatingCounter {
        self.record_bytes
    }
}

impl crate::output_health::OutputHealthDropCounters {
    /// Reconstructs counters in `DiagnosticDropCause` declaration order.
    #[must_use]
    pub const fn from_ordered(counters: [SaturatingCounter; 10]) -> Self {
        Self { counters }
    }

    /// Returns the counter for one exact `DiagnosticOutput` drop cause.
    #[must_use]
    pub const fn for_cause(
        self,
        cause: DiagnosticDropCause,
    ) -> SaturatingCounter {
        self.counters[drop_index(cause)]
    }
}

impl crate::output_health::OutputHealthClosedCounters {
    /// Reconstructs counters in `DiagnosticClosedCause` declaration order.
    #[must_use]
    pub const fn from_ordered(counters: [SaturatingCounter; 4]) -> Self {
        Self { counters }
    }

    /// Returns the counter for one exact `DiagnosticOutput` closed cause.
    #[must_use]
    pub const fn for_cause(
        self,
        cause: DiagnosticClosedCause,
    ) -> SaturatingCounter {
        self.counters[closed_index(cause)]
    }
}

impl crate::output_health::OutputHealthEffectCounters {
    /// Reconstructs counters in `EffectCertainty` declaration order.
    #[must_use]
    pub const fn from_ordered(counters: [SaturatingCounter; 3]) -> Self {
        Self { counters }
    }

    /// Returns the counter for one exact Effect Certainty result.
    #[must_use]
    pub const fn for_certainty(
        self,
        certainty: EffectCertainty,
    ) -> SaturatingCounter {
        self.counters[effect_index(certainty)]
    }
}

impl crate::output_health::OutputHealthEvictionCounters {
    /// Groups complete-anomaly and fixed-charge eviction counters.
    #[must_use]
    pub const fn new(
        records: SaturatingCounter,
        resident_bytes: SaturatingCounter,
    ) -> Self {
        Self { records, resident_bytes }
    }

    /// Returns evicted complete anomaly records.
    #[must_use]
    pub const fn records(self) -> SaturatingCounter {
        self.records
    }

    /// Returns evicted fixed-charge resident bytes.
    #[must_use]
    pub const fn resident_bytes(self) -> SaturatingCounter {
        self.resident_bytes
    }
}

impl crate::output_health::OutputHealthRecordResultCounters {
    /// Reconstructs counters in record-result-kind declaration order.
    #[must_use]
    pub const fn from_ordered(counters: [SaturatingCounter; 5]) -> Self {
        Self { counters }
    }

    /// Returns the counter for one nonaccepted record result.
    #[must_use]
    pub const fn for_kind(
        self,
        kind: OutputHealthRecordResultKind,
    ) -> SaturatingCounter {
        self.counters[record_result_index(kind)]
    }
}

impl crate::output_health::OutputHealthImportCounters {
    /// Reconstructs counters in import-counter-kind declaration order.
    #[must_use]
    pub const fn from_ordered(counters: [SaturatingCounter; 5]) -> Self {
        Self { counters }
    }

    /// Returns the reserved counter for one future import result.
    #[must_use]
    pub const fn for_kind(
        self,
        kind: OutputHealthImportCounterKind,
    ) -> SaturatingCounter {
        self.counters[import_index(kind)]
    }
}

impl crate::output_health::OutputHealthCounters {
    /// Groups the exact seven fixed counter families.
    #[must_use]
    pub const fn new(
        accepted: OutputHealthAcceptedCounters,
        dropped: OutputHealthDropCounters,
        closed: OutputHealthClosedCounters,
        effect: OutputHealthEffectCounters,
        eviction: OutputHealthEvictionCounters,
        record_results: OutputHealthRecordResultCounters,
        import: OutputHealthImportCounters,
    ) -> Self {
        Self {
            accepted,
            dropped,
            closed,
            effect,
            eviction,
            record_results,
            import,
        }
    }

    /// Returns accepted record and byte counters.
    #[must_use]
    pub const fn accepted(self) -> OutputHealthAcceptedCounters {
        self.accepted
    }

    /// Returns all `DiagnosticOutput` drop counters.
    #[must_use]
    pub const fn dropped(self) -> OutputHealthDropCounters {
        self.dropped
    }

    /// Returns all `DiagnosticOutput` closed counters.
    #[must_use]
    pub const fn closed(self) -> OutputHealthClosedCounters {
        self.closed
    }

    /// Returns all Effect Certainty counters.
    #[must_use]
    pub const fn effect(self) -> OutputHealthEffectCounters {
        self.effect
    }

    /// Returns complete-anomaly eviction counters.
    #[must_use]
    pub const fn eviction(self) -> OutputHealthEvictionCounters {
        self.eviction
    }

    /// Returns all nonaccepted health record result counters.
    #[must_use]
    pub const fn record_results(self) -> OutputHealthRecordResultCounters {
        self.record_results
    }

    /// Returns counters reserved for the future import operation.
    #[must_use]
    pub const fn import(self) -> OutputHealthImportCounters {
        self.import
    }
}

const fn drop_index(cause: DiagnosticDropCause) -> usize {
    match cause {
        DiagnosticDropCause::InvalidRecord => 0,
        DiagnosticDropCause::BoundExceeded => 1,
        DiagnosticDropCause::NotReady => 2,
        DiagnosticDropCause::SynchronizationPressure => 3,
        DiagnosticDropCause::RecordCapacity => 4,
        DiagnosticDropCause::ByteCapacity => 5,
        DiagnosticDropCause::ChargeOverflow => 6,
        DiagnosticDropCause::DestinationUnavailable => 7,
        DiagnosticDropCause::HealthUnavailable => 8,
        DiagnosticDropCause::InternalFailure => 9,
    }
}

const fn closed_index(cause: DiagnosticClosedCause) -> usize {
    match cause {
        DiagnosticClosedCause::Closing => 0,
        DiagnosticClosedCause::Closed => 1,
        DiagnosticClosedCause::Replaced => 2,
        DiagnosticClosedCause::IncompleteShutdown => 3,
    }
}

const fn effect_index(certainty: EffectCertainty) -> usize {
    match certainty {
        EffectCertainty::Applied => 0,
        EffectCertainty::DefinitelyNotApplied => 1,
        EffectCertainty::OutcomeUnknown => 2,
    }
}

const fn record_result_index(kind: OutputHealthRecordResultKind) -> usize {
    match kind {
        OutputHealthRecordResultKind::Pressure => 0,
        OutputHealthRecordResultKind::Invalid => 1,
        OutputHealthRecordResultKind::Stale => 2,
        OutputHealthRecordResultKind::Closed => 3,
        OutputHealthRecordResultKind::SequenceExhaustion => 4,
    }
}

const fn import_index(kind: OutputHealthImportCounterKind) -> usize {
    match kind {
        OutputHealthImportCounterKind::Accepted => 0,
        OutputHealthImportCounterKind::Rejected => 1,
        OutputHealthImportCounterKind::Dropped => 2,
        OutputHealthImportCounterKind::Closed => 3,
        OutputHealthImportCounterKind::AttemptLimit => 4,
    }
}
