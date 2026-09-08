use core::cell::Cell;
use std::time::Instant;

use reovim_kernel_interface::clock::{
    ClockDescriptor, ClockError, MonotonicClock, MonotonicInstant,
};

/// Hosted monotonic clock; the native epoch never leaves this adapter.
pub struct StdClock {
    descriptor: ClockDescriptor,
    epoch: Instant,
    last: Cell<u64>,
    closed: Cell<bool>,
}

impl StdClock {
    /// Captures the native epoch for the assembly-selected clock contract.
    #[must_use]
    pub fn new(descriptor: ClockDescriptor) -> Self {
        Self {
            descriptor,
            epoch: Instant::now(),
            last: Cell::new(0),
            closed: Cell::new(false),
        }
    }

    /// Idempotently fences observations without invalidating past instants.
    pub fn close(&self) {
        self.closed.set(true);
    }
}

impl MonotonicClock for StdClock {
    fn descriptor(&self) -> ClockDescriptor {
        self.descriptor
    }

    fn observe(&self) -> Result<MonotonicInstant, ClockError> {
        if self.closed.get() {
            return Err(ClockError::Closed);
        }
        let elapsed = u64::try_from(self.epoch.elapsed().as_nanos())
            .map_err(|_| ClockError::SourceConversionOverflow)?;
        if elapsed < self.last.get() {
            return Err(ClockError::ObservedRegression);
        }
        let observed = self.descriptor.instant(elapsed)?;
        self.last.set(elapsed);
        Ok(observed)
    }
}
