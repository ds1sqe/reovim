use reovim_kernel_interface::diagnostic::{
    DiagnosticClosedCause, DiagnosticDropCause,
};
use reovim_kernel_interface::effect::EffectCertainty;
use reovim_kernel_interface::event::SaturatingCounter;
use reovim_kernel_interface::output_health::{
    OutputHealthAcceptedCounters, OutputHealthClosedCounters,
    OutputHealthCounters, OutputHealthDropCounters, OutputHealthEffectCounters,
    OutputHealthEvictionCounters, OutputHealthImportCounterKind,
    OutputHealthImportCounters, OutputHealthRecordResultCounters,
    OutputHealthRecordResultKind,
};

const DROP_CAUSES: [DiagnosticDropCause; 10] = [
    DiagnosticDropCause::InvalidRecord,
    DiagnosticDropCause::BoundExceeded,
    DiagnosticDropCause::NotReady,
    DiagnosticDropCause::SynchronizationPressure,
    DiagnosticDropCause::RecordCapacity,
    DiagnosticDropCause::ByteCapacity,
    DiagnosticDropCause::ChargeOverflow,
    DiagnosticDropCause::DestinationUnavailable,
    DiagnosticDropCause::HealthUnavailable,
    DiagnosticDropCause::InternalFailure,
];

const CLOSED_CAUSES: [DiagnosticClosedCause; 4] = [
    DiagnosticClosedCause::Closing,
    DiagnosticClosedCause::Closed,
    DiagnosticClosedCause::Replaced,
    DiagnosticClosedCause::IncompleteShutdown,
];

const EFFECTS: [EffectCertainty; 3] = [
    EffectCertainty::Applied,
    EffectCertainty::DefinitelyNotApplied,
    EffectCertainty::OutcomeUnknown,
];

const RECORD_RESULTS: [OutputHealthRecordResultKind; 5] = [
    OutputHealthRecordResultKind::Pressure,
    OutputHealthRecordResultKind::Invalid,
    OutputHealthRecordResultKind::Stale,
    OutputHealthRecordResultKind::Closed,
    OutputHealthRecordResultKind::SequenceExhaustion,
];

const IMPORT_RESULTS: [OutputHealthImportCounterKind; 5] = [
    OutputHealthImportCounterKind::Accepted,
    OutputHealthImportCounterKind::Rejected,
    OutputHealthImportCounterKind::Dropped,
    OutputHealthImportCounterKind::Closed,
    OutputHealthImportCounterKind::AttemptLimit,
];

pub(super) fn exercise() -> bool {
    let Some(counters) = make_counters(0, 0) else {
        return false;
    };
    scalar_addition() && grouped_access() && crate::traits(counters)
}

pub(super) fn make_counters(
    evicted_records: u64,
    evicted_bytes: u64,
) -> Option<OutputHealthCounters> {
    make_counters_with(counter(evicted_records)?, counter(evicted_bytes)?)
}

pub(super) fn make_counters_with(
    evicted_records: SaturatingCounter,
    evicted_bytes: SaturatingCounter,
) -> Option<OutputHealthCounters> {
    let accepted =
        OutputHealthAcceptedCounters::new(counter(11)?, counter(12)?);
    let dropped = OutputHealthDropCounters::from_ordered([
        counter(21)?,
        counter(22)?,
        counter(23)?,
        counter(24)?,
        counter(25)?,
        counter(26)?,
        counter(27)?,
        counter(28)?,
        counter(29)?,
        counter(30)?,
    ]);
    let closed = OutputHealthClosedCounters::from_ordered([
        counter(31)?,
        counter(32)?,
        counter(33)?,
        counter(34)?,
    ]);
    let effect = OutputHealthEffectCounters::from_ordered([
        counter(41)?,
        counter(42)?,
        counter(43)?,
    ]);
    let eviction =
        OutputHealthEvictionCounters::new(evicted_records, evicted_bytes);
    let record_results = OutputHealthRecordResultCounters::from_ordered([
        counter(51)?,
        counter(52)?,
        counter(53)?,
        counter(54)?,
        counter(55)?,
    ]);
    let import = OutputHealthImportCounters::from_ordered([
        counter(61)?,
        counter(62)?,
        counter(63)?,
        counter(64)?,
        counter(65)?,
    ]);
    Some(OutputHealthCounters::new(
        accepted,
        dropped,
        closed,
        effect,
        eviction,
        record_results,
        import,
    ))
}

pub(super) fn counter(value: u64) -> Option<SaturatingCounter> {
    SaturatingCounter::from_parts(value, false).ok()
}

fn saturated_counter() -> Option<SaturatingCounter> {
    SaturatingCounter::from_parts(u64::MAX, true).ok()
}

fn scalar_addition() -> bool {
    let Some(zero) = counter(0) else {
        return false;
    };
    let Some(maximum) = counter(u64::MAX) else {
        return false;
    };
    let Some(already_saturated) = saturated_counter() else {
        return false;
    };
    let exact_maximum = zero.saturating_add(u64::MAX);
    let overflow = maximum.saturating_add(1);
    zero.saturating_add(0) == zero
        && zero.increment().value() == 1
        && exact_maximum.value() == u64::MAX
        && !exact_maximum.saturated()
        && overflow.value() == u64::MAX
        && overflow.saturated()
        && already_saturated.saturating_add(1) == already_saturated
}

fn grouped_access() -> bool {
    let Some(counters) = make_counters(7, 896) else {
        return false;
    };
    counters.accepted().records().value() == 11
        && counters.accepted().record_bytes().value() == 12
        && typed_drop_values(counters.dropped())
        && typed_closed_values(counters.closed())
        && typed_effect_values(counters.effect())
        && counters.eviction().records().value() == 7
        && counters.eviction().resident_bytes().value() == 896
        && typed_record_result_values(counters.record_results())
        && typed_import_values(counters.import())
}

fn typed_drop_values(counters: OutputHealthDropCounters) -> bool {
    DROP_CAUSES.into_iter().enumerate().all(|(index, cause)| {
        counters.for_cause(cause).value() == index as u64 + 21
    })
}

fn typed_closed_values(counters: OutputHealthClosedCounters) -> bool {
    CLOSED_CAUSES.into_iter().enumerate().all(|(index, cause)| {
        counters.for_cause(cause).value() == index as u64 + 31
    })
}

fn typed_effect_values(counters: OutputHealthEffectCounters) -> bool {
    EFFECTS.into_iter().enumerate().all(|(index, certainty)| {
        counters.for_certainty(certainty).value() == index as u64 + 41
    })
}

fn typed_record_result_values(
    counters: OutputHealthRecordResultCounters,
) -> bool {
    RECORD_RESULTS.into_iter().enumerate().all(|(index, kind)| {
        counters.for_kind(kind).value() == index as u64 + 51
    })
}

fn typed_import_values(counters: OutputHealthImportCounters) -> bool {
    IMPORT_RESULTS.into_iter().enumerate().all(|(index, kind)| {
        counters.for_kind(kind).value() == index as u64 + 61
    })
}
