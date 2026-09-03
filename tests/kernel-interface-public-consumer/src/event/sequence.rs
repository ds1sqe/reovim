use reovim_kernel_interface::event::{
    CursorPosition, EventCursor, EventError, EventSequence,
    RetainedSequenceRange, SaturatingCounter,
};

use super::STREAM_INCARNATION;

pub const fn const_cases() -> bool {
    sequence_cases() && counter_cases() && cursor_cases() && range_cases()
}

const fn sequence_cases() -> bool {
    let first = EventSequence::first();
    let Ok(second) = first.next() else { return false };
    let Ok(maximum) = EventSequence::new(u64::MAX) else {
        return false;
    };

    first.value() == 1
        && first.is_before(second)
        && second.value() == 2
        && sequence_code(EventSequence::new(0)) == 9
        && sequence_code(maximum.next()) == 10
}

const fn sequence_code(result: Result<EventSequence, EventError>) -> u8 {
    match result {
        Err(error) => super::outcomes::event_error_code(error),
        Ok(_) => 0,
    }
}

const fn counter_cases() -> bool {
    let zero = SaturatingCounter::zero();
    let Ok(maximum) = SaturatingCounter::from_parts(u64::MAX, false) else {
        return false;
    };
    let saturated = maximum.increment();

    zero.value() == 0
        && !zero.saturated()
        && zero.increment().value() == 1
        && saturated.value() == u64::MAX
        && saturated.saturated()
        && saturated.increment().saturated()
        && counter_code(SaturatingCounter::from_parts(1, true)) == 11
}

const fn counter_code(result: Result<SaturatingCounter, EventError>) -> u8 {
    match result {
        Err(error) => super::outcomes::event_error_code(error),
        Ok(_) => 0,
    }
}

const fn cursor_cases() -> bool {
    let first = EventSequence::first();
    let next = EventCursor::new(STREAM_INCARNATION, first);
    let exhausted = EventCursor::exhausted(STREAM_INCARNATION);

    next.stream_incarnation() == STREAM_INCARNATION
        && position_code(next.position(), first) == 1
        && exhausted.stream_incarnation() == STREAM_INCARNATION
        && position_code(exhausted.position(), first) == 2
}

pub const fn position_code(
    position: CursorPosition,
    expected: EventSequence,
) -> u8 {
    match position {
        CursorPosition::Next(actual) => {
            if actual.value() == expected.value() {
                return 1;
            }
            0
        }
        CursorPosition::Exhausted => 2,
    }
}

const fn range_cases() -> bool {
    let first = EventSequence::first();
    let Ok(second) = first.next() else { return false };
    let empty = RetainedSequenceRange::empty();
    let Ok(nonempty) = RetainedSequenceRange::non_empty(first, second) else {
        return false;
    };

    empty.is_empty()
        && empty.earliest().is_none()
        && empty.latest().is_none()
        && !nonempty.is_empty()
        && option_value(nonempty.earliest()) == first.value()
        && option_value(nonempty.latest()) == second.value()
        && range_code(RetainedSequenceRange::non_empty(second, first)) == 12
}

const fn option_value(value: Option<EventSequence>) -> u64 {
    match value {
        Some(sequence) => sequence.value(),
        None => 0,
    }
}

const fn range_code(result: Result<RetainedSequenceRange, EventError>) -> u8 {
    match result {
        Err(error) => super::outcomes::event_error_code(error),
        Ok(_) => 0,
    }
}
