use reovim_kernel_interface::clock::{
    ClockDescriptor, ClockError, InstantOrdering, MonotonicClock,
    MonotonicInstant,
};
use reovim_kernel_interface::event::{
    BootstrapHealth, CanonicalEventBytes, EventAdmissionScope, EventError,
    EventSequence, EventTimestamp, StartupComponentCounts,
    StartupEventDescriptor, StartupEventLimits, StartupEventRecord,
};
use reovim_kernel_interface::lifecycle::{ReadinessOutcome, RollbackOutcome};

use super::{
    CORRELATION, DEGRADED_LINE, FAILED_LINE, PRODUCER, READY_LINE,
    STARTUP_FAMILY, lifecycle_owner, make_descriptor,
};

const MAXIMUM_LINE: &[u8] = b"reovim-event/v1 \
stream=00000000000000000000000000000022 \
sequence=18446744073709551615 \
timestamp_ns=18446744073709551615 \
producer=00000000000000000000000000000051 \
family=kernel.startup \
schema=1 \
severity=error \
correlation=00000000000000000000000000000052 \
actionability=operator \
remediation=kernel.bootstrap/1/activation-failed \
outcome=activation-failed \
rollback=incomplete-shutdown \
active=10000000000000000000 \
degraded=4223372036854775807 \
unavailable=4223372036854775808 \
bootstrap_health=imported\n";

struct FixedClock {
    descriptor: ClockDescriptor,
    observation: Result<MonotonicInstant, ClockError>,
}

impl FixedClock {
    const fn observed(
        descriptor: ClockDescriptor,
        instant: MonotonicInstant,
    ) -> Self {
        Self { descriptor, observation: Ok(instant) }
    }

    const fn failed(descriptor: ClockDescriptor, error: ClockError) -> Self {
        Self { descriptor, observation: Err(error) }
    }
}

impl MonotonicClock for FixedClock {
    fn descriptor(&self) -> ClockDescriptor {
        self.descriptor
    }

    fn observe(&self) -> Result<MonotonicInstant, ClockError> {
        self.observation
    }
}

pub fn exercise() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    constants_and_canonical_cases(&descriptor)
        && timestamp_cases(&descriptor)
        && record_cases(&descriptor)
}

fn constants_and_canonical_cases(descriptor: &StartupEventDescriptor) -> bool {
    super::STARTUP_CANONICAL_MAX_BYTES == 495
        && super::EVENT_RING_MAX_BYTES == 1_048_576
        && READY_LINE.len() <= 495
        && DEGRADED_LINE.len() <= 495
        && FAILED_LINE.len() <= 495
        && maximum_line_case(descriptor)
        && valid_line(descriptor, READY_LINE)
        && valid_line(descriptor, DEGRADED_LINE)
        && valid_line(descriptor, FAILED_LINE)
        && incoherent_ready_line_fails(descriptor)
        && invalid_line_cases(descriptor)
}

fn incoherent_ready_line_fails(descriptor: &StartupEventDescriptor) -> bool {
    let mut line = READY_LINE.to_vec();
    let key = b"degraded=";
    let Some(offset) = line.windows(key.len()).position(|part| part == key)
    else {
        return false;
    };
    line[offset + key.len()] = b'1';
    canonical_error(
        CanonicalEventBytes::new(descriptor, &line),
        EventError::InvalidStartupRecord,
    )
}

fn maximum_line_case(descriptor: &StartupEventDescriptor) -> bool {
    let Some(maximum_descriptor) = maximum_descriptor(descriptor) else {
        return false;
    };
    let Ok(size) = u64::try_from(MAXIMUM_LINE.len()) else {
        return false;
    };
    size == super::STARTUP_CANONICAL_MAX_BYTES
        && valid_line(&maximum_descriptor, MAXIMUM_LINE)
}

fn maximum_descriptor(
    descriptor: &StartupEventDescriptor,
) -> Option<StartupEventDescriptor> {
    let base_clock = descriptor.clock();
    let clock = ClockDescriptor::new(
        base_clock.contract(),
        base_clock.lifecycle(),
        base_clock.owner(),
        base_clock.resolution().nanoseconds(),
        u64::MAX,
        base_clock.instant_safety().type_id().value(),
    )
    .ok()?;
    let limits = StartupEventLimits::new(
        super::STREAM_INSTANCE,
        u64::MAX,
        u64::MAX,
        32,
        2_048,
        4,
        2_048,
    )
    .ok()?;
    StartupEventDescriptor::new(
        descriptor.contract(),
        descriptor.identities(),
        clock,
        limits,
        descriptor.canonical_reference_safety(),
    )
    .ok()
}

fn valid_line(
    descriptor: &StartupEventDescriptor,
    line: &'static [u8],
) -> bool {
    let Ok(canonical) = CanonicalEventBytes::new(descriptor, line) else {
        return false;
    };
    let Ok(expected_size) = u64::try_from(line.len()) else {
        return false;
    };
    canonical.bytes() == line
        && canonical.size().value() == expected_size
        && super::super::traits(canonical)
}

fn invalid_line_cases(descriptor: &StartupEventDescriptor) -> bool {
    let oversized = [b'a'; 496];
    let non_ascii = [0xff, b'\n'];

    canonical_error(
        CanonicalEventBytes::new(descriptor, b""),
        EventError::EmptyCanonicalBytes,
    ) && canonical_error(
        CanonicalEventBytes::new(descriptor, &non_ascii),
        EventError::NonAsciiCanonicalBytes,
    ) && canonical_error(
        CanonicalEventBytes::new(descriptor, b"invalid"),
        EventError::InvalidCanonicalFraming,
    ) && canonical_error(
        CanonicalEventBytes::new(descriptor, b"invalid\n"),
        EventError::InvalidCanonicalFraming,
    ) && canonical_error(
        CanonicalEventBytes::new(descriptor, b"invalid\r\n"),
        EventError::InvalidCanonicalFraming,
    ) && canonical_code(CanonicalEventBytes::new(descriptor, &oversized)) == 1
}

fn canonical_error(
    result: Result<CanonicalEventBytes<'_>, EventError>,
    expected: EventError,
) -> bool {
    match result {
        Err(actual) => actual == expected,
        Ok(_) => false,
    }
}

const fn canonical_code(
    result: Result<CanonicalEventBytes<'_>, EventError>,
) -> u8 {
    match result {
        Err(error) => super::outcomes::event_error_code(error),
        Ok(_) => 0,
    }
}

fn timestamp_cases(descriptor: &StartupEventDescriptor) -> bool {
    let clock_descriptor = descriptor.clock();
    let Ok(instant) = clock_descriptor.instant(5) else {
        return false;
    };
    let clock = FixedClock::observed(clock_descriptor, instant);
    let Ok(timestamp) = descriptor.observe_timestamp(&clock) else {
        return false;
    };

    timestamp.elapsed_nanoseconds() == 5
        && instant_equal(timestamp.instant(), instant)
        && timestamp_clone_copy(timestamp)
        && descriptor_mismatch_case(descriptor)
        && observation_error_case(descriptor)
}

fn timestamp_clone_copy(timestamp: EventTimestamp) -> bool {
    let cloned = super::super::clone_value(&timestamp);
    let (left, right) = super::super::copy_pair(cloned);
    left.elapsed_nanoseconds() == right.elapsed_nanoseconds()
        && instant_equal(left.instant(), right.instant())
}

const fn instant_equal(
    left: MonotonicInstant,
    right: MonotonicInstant,
) -> bool {
    match left.compare(right) {
        Ok(InstantOrdering::Equal) => true,
        Ok(InstantOrdering::Before | InstantOrdering::After) | Err(_) => false,
    }
}

fn descriptor_mismatch_case(descriptor: &StartupEventDescriptor) -> bool {
    let expected = descriptor.clock();
    let altered = ClockDescriptor::new(
        expected.contract(),
        expected.lifecycle(),
        expected.owner(),
        2,
        expected.maximum_elapsed().nanoseconds(),
        expected.instant_safety().type_id().value(),
    );
    let Ok(altered) = altered else { return false };
    let Ok(instant) = altered.instant(6) else { return false };
    let clock = FixedClock::observed(altered, instant);
    match descriptor.observe_timestamp(&clock) {
        Err(EventError::Clock(ClockError::DescriptorMismatch)) => true,
        Err(_) | Ok(_) => false,
    }
}

fn observation_error_case(descriptor: &StartupEventDescriptor) -> bool {
    let clock = FixedClock::failed(descriptor.clock(), ClockError::NotReady);
    match descriptor.observe_timestamp(&clock) {
        Err(EventError::Clock(ClockError::NotReady)) => true,
        Err(_) | Ok(_) => false,
    }
}

pub fn make_ready_record(
    descriptor: &StartupEventDescriptor,
) -> Option<StartupEventRecord<'static>> {
    make_record(
        descriptor,
        EventSequence::first(),
        5,
        ReadinessOutcome::Ready,
        BootstrapHealth::Clear,
        (3, 0, 0),
        READY_LINE,
    )
}

pub fn make_degraded_record(
    descriptor: &StartupEventDescriptor,
) -> Option<StartupEventRecord<'static>> {
    let sequence = EventSequence::new(2).ok()?;
    make_record(
        descriptor,
        sequence,
        6,
        ReadinessOutcome::DiagnosticsDegraded,
        BootstrapHealth::Imported,
        (1, 2, 0),
        DEGRADED_LINE,
    )
}

pub fn make_failed_record(
    descriptor: &StartupEventDescriptor,
) -> Option<StartupEventRecord<'static>> {
    let sequence = EventSequence::new(3).ok()?;
    make_record(
        descriptor,
        sequence,
        7,
        ReadinessOutcome::ActivationFailed(RollbackOutcome::Reverted),
        BootstrapHealth::Pending,
        (0, 1, 2),
        FAILED_LINE,
    )
}

fn make_record(
    descriptor: &StartupEventDescriptor,
    sequence: EventSequence,
    elapsed: u64,
    readiness: ReadinessOutcome,
    health: BootstrapHealth,
    counts: (u64, u64, u64),
    line: &'static [u8],
) -> Option<StartupEventRecord<'static>> {
    let Ok(instant) = descriptor.clock().instant(elapsed) else {
        return None;
    };
    let clock = FixedClock::observed(descriptor.clock(), instant);
    let Ok(timestamp) = descriptor.observe_timestamp(&clock) else {
        return None;
    };
    let Ok(scope) = EventAdmissionScope::new(
        descriptor,
        lifecycle_owner(),
        PRODUCER,
        CORRELATION,
    ) else {
        return None;
    };
    let Ok(counts) = StartupComponentCounts::new(counts.0, counts.1, counts.2)
    else {
        return None;
    };
    let Ok(proposal) =
        super::descriptor::make_proposal(descriptor, readiness, health, counts)
    else {
        return None;
    };
    let Ok(canonical) = CanonicalEventBytes::new(descriptor, line) else {
        return None;
    };
    StartupEventRecord::new(
        descriptor, scope, sequence, timestamp, proposal, canonical,
    )
    .ok()
}

fn record_cases(descriptor: &StartupEventDescriptor) -> bool {
    let Some(record) = make_ready_record(descriptor) else {
        return false;
    };
    record.scope().producer() == PRODUCER
        && record.family() == STARTUP_FAMILY
        && record.schema_version() == 1
        && record.sequence().value() == 1
        && record.timestamp().elapsed_nanoseconds() == 5
        && super::descriptor::readiness_code(record.proposal().readiness()) == 0
        && record.canonical().bytes() == READY_LINE
        && canonical_ref_cases(&record)
        && record_clone_copy(&record)
        && mismatched_record_case(descriptor, &record)
}

fn canonical_ref_cases(record: &StartupEventRecord<'_>) -> bool {
    let reference = record.canonical_ref();
    reference.stream() == record.scope().stream()
        && reference.family() == record.family()
        && reference.schema_version() == record.schema_version()
        && reference.sequence() == record.sequence()
        && reference.bytes() == record.canonical().bytes()
        && reference.size() == record.canonical().size()
        && super::super::traits(reference)
}

fn record_clone_copy(record: &StartupEventRecord<'_>) -> bool {
    let cloned = super::super::clone_value(record);
    let (left, right) = super::super::copy_pair(cloned);
    left.sequence() == right.sequence()
        && left.canonical().bytes() == right.canonical().bytes()
}

fn mismatched_record_case(
    descriptor: &StartupEventDescriptor,
    record: &StartupEventRecord<'_>,
) -> bool {
    let Ok(canonical) = CanonicalEventBytes::new(descriptor, DEGRADED_LINE)
    else {
        return false;
    };
    let result = StartupEventRecord::new(
        descriptor,
        record.scope(),
        record.sequence(),
        record.timestamp(),
        record.proposal(),
        canonical,
    );
    match &result {
        Err(error) => super::outcomes::event_error_code(*error) == 15,
        Ok(_) => false,
    }
}
