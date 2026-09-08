use core::cell::Cell;

use reovim_kernel_interface::clock::{
    ClockDescriptor, ClockError, MonotonicClock, MonotonicInstant,
};

#[cfg(feature = "std")]
mod standard;
#[cfg(feature = "std")]
pub use standard::StdClock;

/// Explicitly advanced monotonic time under one injected owner/generation.
pub struct DeterministicClock {
    descriptor: ClockDescriptor,
    elapsed: Cell<u64>,
    closed: Cell<bool>,
}

impl DeterministicClock {
    /// Starts an isolated clock at elapsed zero under the supplied contract.
    #[must_use]
    pub const fn new(descriptor: ClockDescriptor) -> Self {
        Self { descriptor, elapsed: Cell::new(0), closed: Cell::new(false) }
    }

    /// Advances this clock without wrapping or changing state on failure.
    ///
    /// # Errors
    /// Returns closed, arithmetic overflow or descriptor exhaustion.
    pub fn advance(&self, nanoseconds: u64) -> Result<(), ClockError> {
        if self.closed.get() {
            return Err(ClockError::Closed);
        }
        let next = self
            .elapsed
            .get()
            .checked_add(nanoseconds)
            .ok_or(ClockError::DeterministicAdvancementOverflow)?;
        self.descriptor.instant(next)?;
        self.elapsed.set(next);
        Ok(())
    }

    /// Idempotently fences future observations and advancement.
    pub fn close(&self) {
        self.closed.set(true);
    }
}

impl MonotonicClock for DeterministicClock {
    fn descriptor(&self) -> ClockDescriptor {
        self.descriptor
    }

    fn observe(&self) -> Result<MonotonicInstant, ClockError> {
        if self.closed.get() {
            return Err(ClockError::Closed);
        }
        self.descriptor.instant(self.elapsed.get())
    }
}
