use super::super::counts::admit_counts;
use super::super::{
    EventError, EventSequence, StartupComponentCounts, StartupEventDescriptor,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::event) enum StartupOutcomeCode {
    Ready,
    DiagnosticsDegraded,
    ActivationFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::event) enum StartupHealthCode {
    Clear,
    Imported,
    Pending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::event) enum StartupRollbackCode {
    None,
    Reverted,
    IncompleteShutdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::event) struct StartupLine {
    pub(in crate::event) stream_incarnation: u128,
    pub(in crate::event) sequence: EventSequence,
    pub(in crate::event) timestamp_nanoseconds: u64,
    pub(in crate::event) producer: u128,
    pub(in crate::event) correlation: u128,
    pub(in crate::event) outcome: StartupOutcomeCode,
    pub(in crate::event) rollback: StartupRollbackCode,
    pub(in crate::event) active: u64,
    pub(in crate::event) degraded: u64,
    pub(in crate::event) unavailable: u64,
    pub(in crate::event) health: StartupHealthCode,
}

pub(super) fn parse_startup_v1(
    descriptor: &StartupEventDescriptor,
    line: &[u8],
) -> Result<StartupLine, EventError> {
    let mut fields = line.split(|byte| *byte == b' ');

    expect(next_field(&mut fields)?, b"reovim-event/v1")?;
    let stream = keyed(next_field(&mut fields)?, b"stream=")?;
    let sequence = keyed(next_field(&mut fields)?, b"sequence=")?;
    let timestamp = keyed(next_field(&mut fields)?, b"timestamp_ns=")?;
    let producer = keyed(next_field(&mut fields)?, b"producer=")?;
    expect(next_field(&mut fields)?, b"family=kernel.startup")?;
    expect(next_field(&mut fields)?, b"schema=1")?;
    let severity = keyed(next_field(&mut fields)?, b"severity=")?;
    let correlation = keyed(next_field(&mut fields)?, b"correlation=")?;
    let actionability = keyed(next_field(&mut fields)?, b"actionability=")?;
    let remediation = keyed(next_field(&mut fields)?, b"remediation=")?;
    let outcome = keyed(next_field(&mut fields)?, b"outcome=")?;
    let rollback = keyed(next_field(&mut fields)?, b"rollback=")?;
    let active = keyed(next_field(&mut fields)?, b"active=")?;
    let degraded = keyed(next_field(&mut fields)?, b"degraded=")?;
    let unavailable = keyed(next_field(&mut fields)?, b"unavailable=")?;
    let health = keyed(next_field(&mut fields)?, b"bootstrap_health=")?;
    if fields.next().is_some() {
        return invalid();
    }

    let stream_incarnation = validate_stream(descriptor, stream)?;
    let producer = parse_hex128(producer)?;
    let correlation = parse_hex128(correlation)?;
    let sequence = validate_sequence(sequence)?;
    let timestamp_nanoseconds = validate_timestamp(descriptor, timestamp)?;
    let (active, degraded, unavailable) =
        validate_counts(descriptor, active, degraded, unavailable)?;
    let outcome =
        validate_classification(severity, actionability, remediation, outcome)?;
    validate_count_outcome(outcome, degraded, unavailable)?;
    let rollback = validate_rollback(outcome, rollback)?;
    let health = validate_health(health)?;
    Ok(StartupLine {
        stream_incarnation,
        sequence,
        timestamp_nanoseconds,
        producer,
        correlation,
        outcome,
        rollback,
        active,
        degraded,
        unavailable,
        health,
    })
}

const fn validate_count_outcome(
    outcome: StartupOutcomeCode,
    degraded: u64,
    unavailable: u64,
) -> Result<(), EventError> {
    let coherent = match outcome {
        StartupOutcomeCode::Ready => degraded == 0 && unavailable == 0,
        StartupOutcomeCode::DiagnosticsDegraded => {
            degraded > 0 || unavailable > 0
        }
        StartupOutcomeCode::ActivationFailed => unavailable > 0,
    };
    if coherent {
        return Ok(());
    }
    Err(EventError::InvalidStartupRecord)
}

fn validate_stream(
    descriptor: &StartupEventDescriptor,
    field: &[u8],
) -> Result<u128, EventError> {
    let stream = parse_hex128(field)?;
    let expected = descriptor.identities().stream().incarnation();
    if stream != expected {
        return invalid();
    }
    Ok(stream)
}

fn validate_sequence(field: &[u8]) -> Result<EventSequence, EventError> {
    let value = parse_u64(field)?;
    EventSequence::new(value)
}

fn validate_timestamp(
    descriptor: &StartupEventDescriptor,
    field: &[u8],
) -> Result<u64, EventError> {
    let value = parse_u64(field)?;
    descriptor.clock().instant(value).map_err(EventError::Clock)?;
    Ok(value)
}

fn validate_counts(
    descriptor: &StartupEventDescriptor,
    active: &[u8],
    degraded: &[u8],
    unavailable: &[u8],
) -> Result<(u64, u64, u64), EventError> {
    let active = parse_u64(active)?;
    let degraded = parse_u64(degraded)?;
    let unavailable = parse_u64(unavailable)?;
    let counts = StartupComponentCounts::new(active, degraded, unavailable)?;
    admit_counts(descriptor.limits().component_counts(), counts)
        .map_err(EventError::Bound)?;
    Ok((active, degraded, unavailable))
}

fn validate_classification(
    severity: &[u8],
    actionability: &[u8],
    remediation: &[u8],
    outcome: &[u8],
) -> Result<StartupOutcomeCode, EventError> {
    if outcome == b"ready"
        && severity == b"info"
        && actionability == b"none"
        && remediation == b"none"
    {
        return Ok(StartupOutcomeCode::Ready);
    }
    if outcome == b"diagnostics-degraded"
        && severity == b"warning"
        && actionability == b"operator"
        && remediation == b"kernel.bootstrap/1/diagnostic-path-degraded"
    {
        return Ok(StartupOutcomeCode::DiagnosticsDegraded);
    }
    if outcome == b"activation-failed"
        && severity == b"error"
        && actionability == b"operator"
        && remediation == b"kernel.bootstrap/1/activation-failed"
    {
        return Ok(StartupOutcomeCode::ActivationFailed);
    }
    invalid()
}

fn validate_health(field: &[u8]) -> Result<StartupHealthCode, EventError> {
    if field == b"clear" {
        return Ok(StartupHealthCode::Clear);
    }
    if field == b"imported" {
        return Ok(StartupHealthCode::Imported);
    }
    if field == b"pending" {
        return Ok(StartupHealthCode::Pending);
    }
    invalid()
}

fn validate_rollback(
    outcome: StartupOutcomeCode,
    field: &[u8],
) -> Result<StartupRollbackCode, EventError> {
    if outcome != StartupOutcomeCode::ActivationFailed && field == b"none" {
        return Ok(StartupRollbackCode::None);
    }
    if outcome == StartupOutcomeCode::ActivationFailed && field == b"reverted" {
        return Ok(StartupRollbackCode::Reverted);
    }
    if outcome == StartupOutcomeCode::ActivationFailed
        && field == b"incomplete-shutdown"
    {
        return Ok(StartupRollbackCode::IncompleteShutdown);
    }
    invalid()
}

fn parse_hex128(field: &[u8]) -> Result<u128, EventError> {
    if field.len() != 32 {
        return invalid();
    }
    let mut value = 0_u128;
    for byte in field {
        let digit = hex_digit(*byte)?;
        value = (value << 4) | u128::from(digit);
    }
    Ok(value)
}

const fn hex_digit(byte: u8) -> Result<u8, EventError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => invalid(),
    }
}

fn parse_u64(field: &[u8]) -> Result<u64, EventError> {
    if field.is_empty() || field.len() > 1 && field[0] == b'0' {
        return invalid();
    }
    let mut value = 0_u64;
    for byte in field {
        if !byte.is_ascii_digit() {
            return invalid();
        }
        let Some(scaled) = value.checked_mul(10) else {
            return invalid();
        };
        let Some(next) = scaled.checked_add(u64::from(*byte - b'0')) else {
            return invalid();
        };
        value = next;
    }
    Ok(value)
}

fn next_field<'a, I>(fields: &mut I) -> Result<&'a [u8], EventError>
where
    I: Iterator<Item = &'a [u8]>,
{
    fields.next().ok_or(EventError::InvalidCanonicalFraming)
}

fn keyed<'a>(field: &'a [u8], prefix: &[u8]) -> Result<&'a [u8], EventError> {
    field.strip_prefix(prefix).ok_or(EventError::InvalidCanonicalFraming)
}

fn expect(field: &[u8], expected: &[u8]) -> Result<(), EventError> {
    if field != expected {
        return invalid();
    }
    Ok(())
}

const fn invalid<T>() -> Result<T, EventError> {
    Err(EventError::InvalidCanonicalFraming)
}
