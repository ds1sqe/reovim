use super::EventError;

impl crate::event::EventSequence {
    /// Returns the first sequence assignable in one Stream incarnation.
    #[must_use]
    pub const fn first() -> Self {
        Self(1)
    }

    /// Constructs an already assigned sequence.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::UnassignedSequence`] when `value` is zero.
    pub const fn new(value: u64) -> Result<Self, EventError> {
        if value == 0 {
            return Err(EventError::UnassignedSequence);
        }
        Ok(Self(value))
    }

    /// Returns the assigned sequence scalar.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Reports whether this sequence precedes `other`.
    #[must_use]
    pub const fn is_before(self, other: Self) -> bool {
        self.0 < other.0
    }

    /// Computes the next sequence without wrapping.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::SequenceExhausted`] at [`u64::MAX`].
    pub const fn next(self) -> Result<Self, EventError> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(EventError::SequenceExhausted),
        }
    }
}

impl crate::event::SaturatingCounter {
    /// Returns a zero, unsaturated counter.
    #[must_use]
    pub const fn zero() -> Self {
        Self { value: 0, saturated: false }
    }

    /// Reconstructs one retained counter and validates its saturation bit.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::InvalidCounterSaturation`] when `saturated` is
    /// true but `value` is below [`u64::MAX`].
    pub const fn from_parts(
        value: u64,
        saturated: bool,
    ) -> Result<Self, EventError> {
        if saturated && value != u64::MAX {
            return Err(EventError::InvalidCounterSaturation);
        }
        Ok(Self { value, saturated })
    }

    /// Returns the retained counter scalar.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.value
    }

    /// Reports whether at least one increment was lost to saturation.
    #[must_use]
    pub const fn saturated(self) -> bool {
        self.saturated
    }

    /// Returns the counter after one visible saturating increment.
    #[must_use]
    pub const fn increment(self) -> Self {
        self.saturating_add(1)
    }

    /// Adds one scalar while making every lost increment visible.
    ///
    /// Exact arrival at [`u64::MAX`] remains unsaturated. A later nonzero
    /// addition sets saturation. Adding zero preserves the complete value.
    #[must_use]
    pub const fn saturating_add(self, additional: u64) -> Self {
        if self.saturated || additional == 0 {
            return self;
        }
        match self.value.checked_add(additional) {
            Some(value) => Self { value, saturated: false },
            None => Self { value: u64::MAX, saturated: true },
        }
    }
}
