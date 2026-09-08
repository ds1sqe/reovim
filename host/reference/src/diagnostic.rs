use alloc::collections::VecDeque;
use core::{cell::RefCell, fmt};

use reovim_kernel_interface::diagnostic::{
    DiagnosticOutputDescriptor, DiagnosticOutputError, DiagnosticOutputRecord,
};
use reovim_kernel_interface::event::EventSequence;
use reovim_kernel_interface::lifecycle::LifecycleIdentity;

mod descriptor;
mod operations;
#[cfg(test)]
mod tests;

/// Construction or non-waiting local ownership failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueError {
    /// A selected record contract or named bound is incoherent.
    Descriptor(DiagnosticOutputError),
    /// Metadata capacity cannot be reserved before activation.
    Capacity,
    /// Another operation owns the local queue; no waiting was attempted.
    SynchronizationPressure,
    /// Admission is no longer live; a failure toggle cannot reopen it.
    Closed,
    /// Internal byte accounting cannot justify an ownership transfer.
    Accounting,
}

impl fmt::Display for QueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "diagnostic queue: {self:?}")
    }
}

impl core::error::Error for QueueError {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    Closing,
    Closed,
}

struct State {
    records: VecDeque<DiagnosticOutputRecord>,
    bytes: u64,
    last: Option<EventSequence>,
    phase: Phase,
    available: bool,
}

/// Bounded ownership queue, not an I/O destination or a second event ring.
///
/// Runtime supplies already-canonical complete records. Submission moves only
/// into preallocated slots. The external consumer takes owned records and
/// performs any actual stderr/file operation outside the editing path.
pub struct DiagnosticQueue {
    descriptor: DiagnosticOutputDescriptor,
    capacity: usize,
    maximum_record: u64,
    maximum_bytes: u64,
    state: RefCell<State>,
}

impl DiagnosticQueue {
    /// Reserves queue metadata before accepting any record ownership.
    ///
    /// # Errors
    /// Returns an incoherent bound/contract or metadata allocation failure.
    pub fn new(
        owner: LifecycleIdentity,
        output: LifecycleIdentity,
        stream: LifecycleIdentity,
        capacity: u64,
        maximum_record: u64,
        maximum_bytes: u64,
    ) -> Result<Self, QueueError> {
        let descriptor = descriptor::build(
            owner,
            output,
            stream,
            capacity,
            maximum_record,
            maximum_bytes,
        )?;
        let capacity =
            usize::try_from(capacity).map_err(|_| QueueError::Capacity)?;
        let mut records = VecDeque::new();
        records
            .try_reserve_exact(capacity)
            .map_err(|_| QueueError::Capacity)?;
        Ok(Self {
            descriptor,
            capacity,
            maximum_record,
            maximum_bytes,
            state: RefCell::new(State {
                records,
                bytes: 0,
                last: None,
                phase: Phase::Ready,
                available: true,
            }),
        })
    }
}
