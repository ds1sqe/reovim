use reovim_foundation::bound::{
    BoundDescriptor, BoundValue, CheckedCount, ZeroPolicy,
};

use super::{EventError, StartupComponentCounts};

impl crate::event::StartupComponentCounts {
    /// Constructs checked component counts before applying descriptor bounds.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::Bound`] when aggregate count arithmetic
    /// overflows.
    pub const fn new(
        active: u64,
        degraded: u64,
        unavailable: u64,
    ) -> Result<Self, EventError> {
        let active = match CheckedCount::new(active, ZeroPolicy::Allow) {
            Ok(value) => value,
            Err(error) => return Err(EventError::Bound(error)),
        };
        let degraded = match CheckedCount::new(degraded, ZeroPolicy::Allow) {
            Ok(value) => value,
            Err(error) => return Err(EventError::Bound(error)),
        };
        let unavailable =
            match CheckedCount::new(unavailable, ZeroPolicy::Allow) {
                Ok(value) => value,
                Err(error) => return Err(EventError::Bound(error)),
            };
        let partial = match active.checked_add(degraded) {
            Ok(value) => value,
            Err(error) => return Err(EventError::Bound(error)),
        };
        let total = match partial.checked_add(unavailable) {
            Ok(value) => value,
            Err(error) => return Err(EventError::Bound(error)),
        };
        Ok(Self { active, degraded, unavailable, total })
    }

    /// Returns the active component count.
    #[must_use]
    pub const fn active(self) -> CheckedCount {
        self.active
    }

    /// Returns the degraded component count.
    #[must_use]
    pub const fn degraded(self) -> CheckedCount {
        self.degraded
    }

    /// Returns the unavailable component count.
    #[must_use]
    pub const fn unavailable(self) -> CheckedCount {
        self.unavailable
    }

    /// Returns the checked aggregate component count.
    #[must_use]
    pub const fn total(self) -> CheckedCount {
        self.total
    }
}

pub(super) const fn admit_counts(
    bound: BoundDescriptor,
    counts: StartupComponentCounts,
) -> Result<(), reovim_foundation::bound::BoundError> {
    let values = [counts.active(), counts.degraded(), counts.unavailable()];
    let mut index = 0;
    while index < values.len() {
        match bound.admit_per_object(BoundValue::Count(values[index])) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        index += 1;
    }
    let zero = match CheckedCount::new(0, ZeroPolicy::Allow) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    match bound.admit_aggregate(
        BoundValue::Count(zero),
        BoundValue::Count(counts.total()),
    ) {
        Ok(_) => Ok(()),
        Err(error) => Err(error),
    }
}
