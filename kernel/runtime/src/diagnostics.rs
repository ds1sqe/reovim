//! Runtime-owned bounded diagnostics, never editing acceptance authority.

use alloc::{collections::VecDeque, rc::Rc, vec::Vec};
use core::{
    cell::{Cell, RefCell},
    fmt,
    mem::size_of,
};

use reovim_kernel_interface::clock::{
    ClockDescriptor, ClockError, MonotonicClock,
};
use reovim_kernel_interface::diagnostic::{
    DiagnosticOutput, DiagnosticOutputRecord, DiagnosticSubmissionStatus,
};
use reovim_kernel_interface::event::editing::{
    EditingContext, EditingRecord, EditingRecordError, EditingStage,
};
use reovim_kernel_interface::event::{EVENT_RING_MAX_BYTES, EventSequence};
use reovim_kernel_interface::lifecycle::LifecycleIdentity;

/// Diagnostic failure never changes a terminal editing result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelemetryError {
    /// Requested resident ring allocation exceeds the hard 1 MiB ceiling.
    Capacity,
    /// Construction could not reserve the bounded ring allocation.
    Allocation,
    /// Clock or output grant is owned by another lifecycle/stream.
    Grant,
    /// Host clock observation failed.
    Clock(ClockError),
    /// A nested writer could not acquire non-waiting ownership.
    Pressure,
    /// Stream sequence cannot advance without wrapping.
    Sequence,
    /// Safe structured fields could not be encoded coherently.
    Record(EditingRecordError),
    /// The stream has closed admission.
    Closed,
}

impl fmt::Display for TelemetryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Runtime diagnostic: {self:?}")
    }
}
impl core::error::Error for TelemetryError {}

/// Observable diagnostic admission, separate from editing acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emission {
    /// Complete record admitted under the stream-assigned sequence.
    Accepted(EventSequence),
    /// Detailed instrumentation is explicitly disabled.
    Disabled,
    /// Bounded observation was unavailable; no sequence was consumed.
    Unavailable(TelemetryError),
}

impl Emission {
    /// Returns evidence only when a complete diagnostic actually exists.
    #[must_use]
    pub const fn sequence(self) -> Option<EventSequence> {
        match self {
            Self::Accepted(sequence) => Some(sequence),
            _ => None,
        }
    }
}

/// Fixed health remains readable without recursively logging failures.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TelemetryHealth {
    /// Failed pre-admission observations.
    pub rejected: u64,
    /// Old complete records evicted by the admitted ring capacity.
    pub evicted: u64,
    /// Copies that could not reach their separately bounded output.
    pub output_lost: u64,
}

struct Ring {
    records: VecDeque<EditingRecord>,
    next: Option<EventSequence>,
}

/// One stream shared by root and Session owners through explicit injection.
///
/// Records contain inline canonical storage and scalar evidence only. Native
/// snapshots/leases are not retained here. Sink fanout occurs only in `flush`.
pub struct Telemetry {
    owner: LifecycleIdentity,
    stream: LifecycleIdentity,
    clock: Rc<dyn MonotonicClock>,
    clock_selection: ClockDescriptor,
    sink: Option<Rc<dyn DiagnosticOutput>>,
    ring: RefCell<Ring>,
    enabled: Cell<bool>,
    closed: Cell<bool>,
    health: Cell<TelemetryHealth>,
    output_frontier: Cell<u64>,
    flushing: Cell<bool>,
    capacity: usize,
    resident_bytes: usize,
}

impl Telemetry {
    /// Reserves the ring before readiness and validates exact Host grants.
    ///
    /// # Errors
    /// Returns typed grant, capacity or allocation failure without activation.
    pub fn new(
        owner: LifecycleIdentity,
        stream: LifecycleIdentity,
        clock: Rc<dyn MonotonicClock>,
        sink: Option<Rc<dyn DiagnosticOutput>>,
        capacity: usize,
    ) -> Result<Self, TelemetryError> {
        if clock.descriptor().owner() != owner {
            return Err(TelemetryError::Grant);
        }
        if sink.as_ref().is_some_and(|output| {
            output.descriptor().owner() != owner
                || output.descriptor().source_stream() != stream
        }) {
            return Err(TelemetryError::Grant);
        }
        storage_charge(capacity)?;
        let mut records = VecDeque::new();
        records
            .try_reserve_exact(capacity)
            .map_err(|_| TelemetryError::Allocation)?;
        let resident_bytes = storage_charge(records.capacity())?;
        Ok(Self {
            owner,
            stream,
            clock_selection: clock.descriptor(),
            clock,
            sink,
            ring: RefCell::new(Ring {
                records,
                next: Some(EventSequence::first()),
            }),
            enabled: Cell::new(true),
            closed: Cell::new(false),
            health: Cell::new(TelemetryHealth::default()),
            output_frontier: Cell::new(0),
            flushing: Cell::new(false),
            capacity,
            resident_bytes,
        })
    }

    /// Emits after releasing editing authority; never calls an output sink.
    #[must_use]
    pub fn emit(
        &self,
        stage: EditingStage,
        context: &EditingContext,
    ) -> Emission {
        if !self.enabled.get() && !self.closed.get() {
            return Emission::Disabled;
        }
        match self.admit(stage, context) {
            Ok(sequence) => Emission::Accepted(sequence),
            Err(cause) => {
                let mut health = self.health.get();
                health.rejected = health.rejected.saturating_add(1);
                self.health.set(health);
                Emission::Unavailable(cause)
            }
        }
    }

    fn admit(
        &self,
        stage: EditingStage,
        context: &EditingContext,
    ) -> Result<EventSequence, TelemetryError> {
        if self.closed.get() {
            return Err(TelemetryError::Closed);
        }
        let descriptor = self.clock.descriptor();
        if descriptor.owner() != self.owner {
            return Err(TelemetryError::Grant);
        }
        if descriptor != self.clock_selection {
            return Err(TelemetryError::Clock(ClockError::DescriptorMismatch));
        }
        let instant = self.clock.observe().map_err(TelemetryError::Clock)?;
        if self.clock.descriptor() != descriptor {
            return Err(TelemetryError::Clock(ClockError::DescriptorMismatch));
        }
        let mut ring =
            self.ring.try_borrow_mut().map_err(|_| TelemetryError::Pressure)?;
        let sequence = ring.next.ok_or(TelemetryError::Sequence)?;
        let record = EditingRecord::new(
            self.stream,
            self.owner,
            sequence,
            descriptor,
            instant,
            stage,
            *context,
        )
        .map_err(TelemetryError::Record)?;
        if ring.records.len() == self.capacity {
            ring.records.pop_front();
            let mut health = self.health.get();
            health.evicted = health.evicted.saturating_add(1);
            self.health.set(health);
        }
        ring.records.push_back(record);
        ring.next = sequence.next().ok();
        Ok(sequence)
    }

    /// Copies retained canonical bytes to the selected output after editing.
    /// Lost/evicted output is visible; it cannot block or roll back acceptance.
    pub fn flush(&self) {
        let Some(sink) = &self.sink else { return };
        if self.flushing.replace(true) {
            return;
        }
        // A destination may call back into diagnostics, but cannot recursively
        // claim the same output frontier. Unwinding also releases this guard.
        let _guard = FlushGuard(&self.flushing);
        flush_records(self, sink.as_ref());
    }

    fn copy_to_output(
        &self,
        sink: &dyn DiagnosticOutput,
        record: &EditingRecord,
    ) {
        let source = record.canonical();
        if sink.descriptor().validate_record(source).is_err() {
            self.output_loss();
            return;
        }
        let mut bytes = Vec::new();
        if bytes.try_reserve_exact(source.bytes().len()).is_err() {
            self.output_loss();
            return;
        }
        bytes.extend_from_slice(source.bytes());
        let bound = DiagnosticOutputRecord::bind(
            sink.descriptor(),
            source,
            bytes.into_boxed_slice(),
        );
        let Ok(bound) = bound else {
            self.output_loss();
            return;
        };
        if sink.submit(bound).status() != DiagnosticSubmissionStatus::Accepted {
            self.output_loss();
        }
    }

    fn output_loss(&self) {
        let mut health = self.health.get();
        health.output_lost = health.output_lost.saturating_add(1);
        self.health.set(health);
    }

    /// Reads the next retained diagnostic, without consuming another reader.
    /// Compare the returned sequence with `after + 1` to detect eviction gaps.
    #[must_use]
    pub fn next_record(&self, after: u64) -> Option<EditingRecord> {
        self.ring
            .borrow()
            .records
            .iter()
            .find(|record| record.sequence().value() > after)
            .cloned()
    }

    /// Returns fixed failure/loss observations without output recursion.
    #[must_use]
    pub const fn health(&self) -> TelemetryHealth {
        self.health.get()
    }

    /// Returns charged ring allocation including inline records and metadata.
    #[must_use]
    pub const fn resident_bytes(&self) -> usize {
        self.resident_bytes
    }

    /// Enables/disables detailed observations for paired overhead experiments.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }

    /// Fences new diagnostics; retained records remain readable/drainable.
    pub fn close(&self) {
        self.closed.set(true);
    }
}

struct FlushGuard<'a>(&'a Cell<bool>);

impl Drop for FlushGuard<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

fn flush_records(telemetry: &Telemetry, sink: &dyn DiagnosticOutput) {
    // A captured outer frontier prevents a reentrant sink prolonging a drain.
    let frontier = telemetry
        .ring
        .borrow()
        .records
        .back()
        .map_or(0, |record| record.sequence().value());
    for _ in 0..telemetry.capacity {
        let previous = telemetry.output_frontier.get();
        let Some(record) = telemetry.next_record(previous) else { return };
        if record.sequence().value() > frontier {
            return;
        }
        let lost = record
            .sequence()
            .value()
            .saturating_sub(previous)
            .saturating_sub(1);
        let mut health = telemetry.health.get();
        health.output_lost = health.output_lost.saturating_add(lost);
        telemetry.health.set(health);
        telemetry.copy_to_output(sink, &record);
        telemetry.output_frontier.set(record.sequence().value());
    }
}

fn storage_charge(capacity: usize) -> Result<usize, TelemetryError> {
    if capacity == 0 {
        return Err(TelemetryError::Capacity);
    }
    let bytes = capacity
        .checked_mul(size_of::<EditingRecord>())
        .and_then(|bytes| bytes.checked_add(size_of::<Telemetry>()))
        .ok_or(TelemetryError::Capacity)?;
    if bytes as u128 > u128::from(EVENT_RING_MAX_BYTES) {
        return Err(TelemetryError::Capacity);
    }
    Ok(bytes)
}
