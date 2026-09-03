use super::{CursorPosition, EventCursor, EventError, EventSequence};

impl crate::event::EventCursor {
    /// Constructs a cursor for the next sequence in one Stream incarnation.
    #[must_use]
    pub const fn new(
        stream_incarnation: u128,
        next_sequence: EventSequence,
    ) -> Self {
        Self {
            stream_incarnation,
            position: CursorPosition::Next(next_sequence),
        }
    }

    /// Constructs the terminal cursor after sequence [`u64::MAX`].
    #[must_use]
    pub const fn exhausted(stream_incarnation: u128) -> Self {
        Self { stream_incarnation, position: CursorPosition::Exhausted }
    }

    /// Returns the Stream incarnation scalar.
    #[must_use]
    pub const fn stream_incarnation(self) -> u128 {
        self.stream_incarnation
    }

    /// Returns the explicit next-sequence or exhausted position.
    #[must_use]
    pub const fn position(self) -> CursorPosition {
        self.position
    }
}

impl crate::event::RetainedSequenceRange {
    /// Returns an explicitly empty retained range.
    #[must_use]
    pub const fn empty() -> Self {
        Self { earliest: None, latest: None }
    }

    /// Constructs one inclusive nonempty retained range.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::InvalidRetainedRange`] when `earliest` follows
    /// `latest`.
    pub const fn non_empty(
        earliest: EventSequence,
        latest: EventSequence,
    ) -> Result<Self, EventError> {
        if latest.is_before(earliest) {
            return Err(EventError::InvalidRetainedRange);
        }
        Ok(Self { earliest: Some(earliest), latest: Some(latest) })
    }

    /// Returns the earliest retained sequence, or `None` when empty.
    #[must_use]
    pub const fn earliest(self) -> Option<EventSequence> {
        self.earliest
    }

    /// Returns the latest retained sequence, or `None` when empty.
    #[must_use]
    pub const fn latest(self) -> Option<EventSequence> {
        self.latest
    }

    /// Reports whether no admitted record is retained.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.earliest.is_none()
    }
}

pub(super) const fn complete_cursor_valid(
    latest: EventSequence,
    cursor: EventCursor,
) -> bool {
    match latest.next() {
        Ok(next) => next_cursor_matches(cursor, next),
        Err(EventError::SequenceExhausted) => {
            cursor_is_exhausted(cursor.position())
        }
        Err(_) => false,
    }
}

pub(super) const fn empty_cursor_valid(cursor: EventCursor) -> bool {
    match cursor.position() {
        CursorPosition::Next(sequence) => {
            sequence.value() == EventSequence::first().value()
        }
        CursorPosition::Exhausted => false,
    }
}

pub(super) const fn next_cursor_matches(
    cursor: EventCursor,
    expected: EventSequence,
) -> bool {
    match cursor.position() {
        CursorPosition::Next(actual) => actual.value() == expected.value(),
        CursorPosition::Exhausted => false,
    }
}

const fn cursor_is_exhausted(position: CursorPosition) -> bool {
    match position {
        CursorPosition::Next(_) => false,
        CursorPosition::Exhausted => true,
    }
}
