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
mod initialize;

struct Retained<State> {
    stamp: BufferStateStamp,
    parent: Option<BufferHistoryNodeIdentity>,
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
    used: usize,
    stamp: BufferStateStamp,
    next_node: u128,
    retained_bytes: CheckedByteCount,
}

/// A Core-selected accepted transition, ready for Runtime delivery binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferCommit {
    /// Exact prior authority, not merely the client's claimed version.
    pub prior: BufferStateStamp,
    /// Fresh accepted Buffer version and Core-allocated History node.
    pub result: BufferStateStamp,
    /// New child relation; prior retained nodes remain available.
    pub edge: BufferHistoryEdge,
    /// Exact evaluated native plan and causal authority.
    pub plan: BufferPlanStamp,
}

impl<State> Buffer<State> {
    /// Returns the current exact authoritative state without native work.
    #[must_use]
    pub const fn stamp(&self) -> BufferStateStamp {
        self.stamp
    }

    /// Retains current immutable data without requiring `State: Clone`.
    /// Runtime bounds lease owners and drops the borrow before policy calls.
    ///
    /// # Panics
    /// Panics only if the private retained-current invariant is violated.
    #[must_use]
    pub fn observe(&self) -> StateLease<State> {
        let node = self.nodes[self.current]
            .as_ref()
            .expect("the current accepted node remains retained");
        StateLease {
            model: self.config.model,
            stamp: self.stamp,
            prepared: Rc::clone(&node.prepared),
        }
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
            .finish_non_exhaustive()
    }
}
