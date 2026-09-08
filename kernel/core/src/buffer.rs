use alloc::{rc::Rc, vec::Vec};
use core::fmt;

use reovim_kernel_interface::bound::CheckedByteCount;
use reovim_kernel_interface::buffer::plan::BufferPlanStamp;
use reovim_kernel_interface::buffer::{
    BufferHistoryEdge, BufferHistoryNodeIdentity, BufferStateStamp,
};
use reovim_kernel_interface::resource::state::ResourcePreparedState;

use crate::{BufferConfig, StateLease};

mod edit;
mod history;
mod initialize;

struct Retained<State> {
    stamp: BufferStateStamp,
    parent: Option<BufferHistoryNodeIdentity>,
    preferred_child: Option<BufferHistoryNodeIdentity>,
    prepared: Rc<ResourcePreparedState<State>>,
}

/// One authoritative native working state with bounded retained History.
///
/// This owner has no policy, Resource access, Client or executor reference.
/// Runtime serializes operations and releases leases outside authority.
/// Capacity exhaustion preserves all accepted nodes; no implicit eviction.
pub struct Buffer<State> {
    config: BufferConfig,
    nodes: Vec<Option<Retained<State>>>,
    current: usize,
    current_state: Rc<ResourcePreparedState<State>>,
    used: usize,
    stamp: BufferStateStamp,
    next_node: u128,
    retained_bytes: CheckedByteCount,
    resident_bytes: CheckedByteCount,
}

/// A Core-selected accepted transition, ready for Runtime delivery binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferCommit {
    /// Exact prior authority, not merely the client's claimed version.
    pub prior: BufferStateStamp,
    /// Fresh Buffer version and the newly created or traversed History node.
    pub result: BufferStateStamp,
    /// Exact created or traversed relation; siblings remain retained.
    pub edge: BufferHistoryEdge,
    /// Exact evaluated native plan and causal authority.
    pub plan: BufferPlanStamp,
}

/// Installed result and displaced ownership, both returned outside authority.
///
/// The retired lease may still share a canonical retained node. A restored
/// materialization can instead be unique, so its destructor must never run
/// before the caller releases the Buffer's mutable guard.
pub struct Installation<State> {
    /// Accepted state transition for Runtime outcome/notification binding.
    pub commit: BufferCommit,
    /// Previously current native ownership; release outside authority.
    pub retired: StateLease<State>,
}

impl<State> fmt::Debug for Installation<State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Installation")
            .field("commit", &self.commit)
            .field("retired", &self.retired)
            .finish()
    }
}

impl<State> Buffer<State> {
    /// Returns the current exact authoritative state without native work.
    #[must_use]
    pub const fn stamp(&self) -> BufferStateStamp {
        self.stamp
    }

    /// Retains current immutable data without requiring `State: Clone`.
    /// Runtime bounds lease owners and drops the borrow before policy calls.
    #[must_use]
    pub fn observe(&self) -> StateLease<State> {
        StateLease {
            model: self.config.model,
            stamp: self.stamp,
            prepared: Rc::clone(&self.current_state),
        }
    }

    /// Returns canonical native charges plus an extra current reconstruction.
    /// Runtime separately bounds pending, leased and returned retired work.
    #[must_use]
    pub const fn resident_bytes(&self) -> CheckedByteCount {
        self.resident_bytes
    }
}

impl<State> fmt::Debug for Buffer<State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let node = self.nodes[self.current].as_ref();
        formatter
            .debug_struct("Buffer")
            .field("stamp", &self.stamp)
            .field("retained_stamp", &node.map(|value| value.stamp))
            .field("parent", &node.and_then(|value| value.parent))
            .field("retained_bytes", &self.retained_bytes)
            .field("resident_bytes", &self.resident_bytes)
            .finish_non_exhaustive()
    }
}
