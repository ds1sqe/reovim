use alloc::{collections::VecDeque, vec::Vec};
use core::{
    cell::{Cell, RefCell},
    mem::size_of,
};

use reovim_kernel_interface::bound::CheckedByteCount;
use reovim_kernel_interface::event::EventSequence;
use reovim_kernel_interface::event::editing::{EditingRecord, EditingStage};
use reovim_kernel_interface::event::trace::{
    SemanticCapture, SemanticCaptureKey, SemanticCaptureStamp, TraceError,
    TraceLimits, TraceLocation, TraceNode, TraceQueryBudget, TraceReadPort,
    TraceReport, TraceStart,
};
use reovim_kernel_interface::interaction::update;
use reovim_kernel_interface::lifecycle::LifecycleIdentity;

mod query;

/// Collector health is independent of the diagnostic ring and editing result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TraceHealth {
    /// Graph nodes evicted by its own retention limit.
    pub evicted_nodes: u64,
    /// Semantic payloads evicted independently of graph nodes.
    pub evicted_snapshots: u64,
    /// Observations that could not be retained.
    pub lost: u64,
}

struct State {
    nodes: VecDeque<TraceNode>,
    snapshots: VecDeque<SemanticCapture>,
    evicted_snapshots: VecDeque<SemanticCaptureKey>,
    evicted_through: Option<EventSequence>,
    payload_bytes: usize,
}

/// A separately granted side store. It contains no native state lease.
pub(super) struct TraceStore {
    stream: LifecycleIdentity,
    grant: LifecycleIdentity,
    limits: TraceLimits,
    metadata_bytes: usize,
    state: RefCell<State>,
    health: Cell<TraceHealth>,
}

impl TraceStore {
    pub fn new(
        stream: LifecycleIdentity,
        grant: LifecycleIdentity,
        limits: TraceLimits,
    ) -> Result<Self, TraceError> {
        let nodes = capacity(limits.nodes.value())?;
        let captures = capacity(limits.snapshots.value())?;
        if nodes == 0 || captures == 0 || limits.payload_bytes.value() == 0 {
            return Err(TraceError::Pressure);
        }
        check_metadata(nodes, captures, limits.metadata_bytes)?;
        let nodes = reserve(nodes)?;
        let snapshots = reserve(captures)?;
        let evicted_snapshots = reserve(captures)?;
        let metadata_bytes = check_metadata(
            nodes.capacity(),
            snapshots.capacity().max(evicted_snapshots.capacity()),
            limits.metadata_bytes,
        )?;
        Ok(Self {
            stream,
            grant,
            limits,
            metadata_bytes,
            health: Cell::new(TraceHealth::default()),
            state: RefCell::new(State {
                nodes,
                snapshots,
                evicted_snapshots,
                evicted_through: None,
                payload_bytes: 0,
            }),
        })
    }

    pub fn record(&self, record: &EditingRecord) -> Result<(), TraceError> {
        let mut state =
            self.state.try_borrow_mut().map_err(|_| TraceError::Pressure)?;
        let node = TraceNode {
            location: TraceLocation {
                stream: self.stream,
                sequence: record.sequence(),
            },
            stage: record.stage(),
            clock: record.clock(),
            nanoseconds: record.nanoseconds(),
            context: *record.context(),
        };
        if state.nodes.len() as u64 == self.limits.nodes.value() {
            state.evicted_through =
                state.nodes.pop_front().map(|node| node.location.sequence);
            let mut health = self.health.get();
            health.evicted_nodes = health.evicted_nodes.saturating_add(1);
            self.health.set(health);
        }
        state.nodes.push_back(node);
        Ok(())
    }

    pub fn snapshot(
        &self,
        stamp: &SemanticCaptureStamp,
        payload: &[u8],
    ) -> Result<(), TraceError> {
        let budget = capacity(self.limits.payload_bytes.value())?;
        if payload.len() > budget {
            return Err(TraceError::Pressure);
        }
        let mut state =
            self.state.try_borrow_mut().map_err(|_| TraceError::Pressure)?;
        if state.snapshots.iter().any(|value| value.stamp == *stamp) {
            return Err(TraceError::Invalid);
        }
        // Release old payloads before allocating their bounded replacement.
        // Capture loss is visible even if the new allocation then fails.
        reserve_snapshot(
            &mut state,
            &self.limits,
            payload.len(),
            &self.health,
        )?;
        let payload = copy_payload(payload)?;
        state.payload_bytes += payload.len();
        state.snapshots.push_back(SemanticCapture { stamp: *stamp, payload });
        Ok(())
    }

    pub fn update(
        &self,
        identity: &update::RepresentationUpdateIdentityStamp,
        stage: EditingStage,
    ) -> Option<TraceNode> {
        self.state
            .try_borrow()
            .ok()?
            .nodes
            .iter()
            .rev()
            .find(|node| {
                node.stage == stage && node.context.update == Some(*identity)
            })
            .copied()
    }

    pub const fn health(&self) -> TraceHealth {
        self.health.get()
    }

    pub fn loss(&self) {
        let mut health = self.health.get();
        health.lost = health.lost.saturating_add(1);
        self.health.set(health);
    }

    pub fn resident_bytes(&self) -> Result<usize, TraceError> {
        self.state
            .try_borrow()
            .map(|state| self.metadata_bytes + state.payload_bytes)
            .map_err(|_| TraceError::Pressure)
    }

    fn authorize(&self, grant: LifecycleIdentity) -> Result<(), TraceError> {
        if grant != self.grant {
            return Err(TraceError::Denied);
        }
        Ok(())
    }
}

impl TraceReadPort for TraceStore {
    fn backtrace(
        &self,
        grant: LifecycleIdentity,
        start: TraceStart,
        budget: TraceQueryBudget,
    ) -> Result<TraceReport, TraceError> {
        self.authorize(grant)?;
        if let TraceStart::Event(location) = start
            && location.stream != self.stream
        {
            return Err(TraceError::Replaced);
        }
        let state =
            self.state.try_borrow().map_err(|_| TraceError::Pressure)?;
        query::walk(&state, &start, &budget)
    }

    fn captured(
        &self,
        grant: LifecycleIdentity,
        key: &SemanticCaptureKey,
        bytes: CheckedByteCount,
    ) -> Result<SemanticCapture, TraceError> {
        self.authorize(grant)?;
        let state =
            self.state.try_borrow().map_err(|_| TraceError::Pressure)?;
        let capture = state.snapshots.iter().find(|value| {
            value.stamp.update == key.update && value.stamp.state == key.state
        });
        let missing = if state.evicted_snapshots.contains(key) {
            TraceError::Evicted
        } else {
            TraceError::NotCaptured
        };
        let capture = capture.ok_or(missing)?;
        let total = size_of::<SemanticCapture>()
            .checked_add(capture.payload.len())
            .ok_or(TraceError::Pressure)?;
        if total > capacity(bytes.value())? {
            return Err(TraceError::Pressure);
        }
        Ok(SemanticCapture {
            stamp: capture.stamp,
            payload: copy_payload(&capture.payload)?,
        })
    }
}

fn reserve_snapshot(
    state: &mut State,
    limits: &TraceLimits,
    bytes: usize,
    counter: &Cell<TraceHealth>,
) -> Result<(), TraceError> {
    let budget = capacity(limits.payload_bytes.value())?;
    while state.snapshots.len() as u64 >= limits.snapshots.value()
        || state.payload_bytes > budget - bytes
    {
        let prior = state.snapshots.pop_front().ok_or(TraceError::Pressure)?;
        state.payload_bytes -= prior.payload.len();
        if state.evicted_snapshots.len() as u64 == limits.snapshots.value() {
            state.evicted_snapshots.pop_front();
        }
        state.evicted_snapshots.push_back(SemanticCaptureKey {
            update: prior.stamp.update,
            state: prior.stamp.state,
        });
        let mut health = counter.get();
        health.evicted_snapshots = health.evicted_snapshots.saturating_add(1);
        counter.set(health);
    }
    Ok(())
}

fn reserve<T>(capacity: usize) -> Result<VecDeque<T>, TraceError> {
    let mut values = VecDeque::new();
    values.try_reserve_exact(capacity).map_err(|_| TraceError::Pressure)?;
    Ok(values)
}

fn copy_payload(bytes: &[u8]) -> Result<alloc::boxed::Box<[u8]>, TraceError> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len()).map_err(|_| TraceError::Pressure)?;
    copy.extend_from_slice(bytes);
    Ok(copy.into_boxed_slice())
}

fn capacity(value: u64) -> Result<usize, TraceError> {
    usize::try_from(value).map_err(|_| TraceError::Pressure)
}

fn check_metadata(
    nodes: usize,
    snapshots: usize,
    limit: CheckedByteCount,
) -> Result<usize, TraceError> {
    let captures =
        size_of::<SemanticCapture>() + size_of::<SemanticCaptureKey>();
    let bytes = nodes
        .checked_mul(size_of::<TraceNode>())
        .and_then(|nodes| {
            snapshots
                .checked_mul(captures)
                .and_then(|captures| nodes.checked_add(captures))
        })
        .and_then(|bytes| bytes.checked_add(size_of::<TraceStore>()))
        .ok_or(TraceError::Pressure)?;
    if bytes > capacity(limit.value())? {
        return Err(TraceError::Pressure);
    }
    Ok(bytes)
}
