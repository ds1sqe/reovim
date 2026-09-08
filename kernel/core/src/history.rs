use alloc::rc::Rc;
use core::fmt;

use reovim_kernel_interface::buffer::plan::BufferPlanStamp;
use reovim_kernel_interface::buffer::{
    BufferHistoryDirection, BufferHistoryEdge, BufferHistoryNodeIdentity,
};
use reovim_kernel_interface::resource::state::ResourcePreparedState;

use crate::StateLease;

/// Core-selected exact tree relation and immutable reconstruction inputs.
///
/// Selection alone changes no state or preferred branch. Its opaque proof
/// remains subject to currentness, membership and preference checks at commit.
pub struct HistorySelection<State> {
    pub(crate) current: StateLease<State>,
    pub(crate) retained: StateLease<State>,
    pub(crate) retained_index: usize,
    pub(crate) edge: BufferHistoryEdge,
    pub(crate) direction: BufferHistoryDirection,
    pub(crate) plan: BufferPlanStamp,
    pub(crate) preferred: Option<BufferHistoryNodeIdentity>,
}

impl<State> HistorySelection<State> {
    /// Borrows the actual current materialization, not its canonical node.
    #[must_use]
    pub const fn current(&self) -> &StateLease<State> {
        &self.current
    }

    /// Borrows canonical retained data under its original accepted stamp.
    #[must_use]
    pub const fn retained(&self) -> &StateLease<State> {
        &self.retained
    }

    /// Returns the exact selected parent/child relation.
    #[must_use]
    pub const fn edge(&self) -> BufferHistoryEdge {
        self.edge
    }

    /// Returns the traversal direction, independent of node identity values.
    #[must_use]
    pub const fn direction(&self) -> BufferHistoryDirection {
        self.direction
    }

    /// Returns the newly requested transaction and causal authorities.
    #[must_use]
    pub const fn plan(&self) -> BufferPlanStamp {
        self.plan
    }
}

impl<State> fmt::Debug for HistorySelection<State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HistorySelection")
            .field("current", &self.current.stamp())
            .field("retained", &self.retained.stamp())
            .field("edge", &self.edge)
            .field("direction", &self.direction)
            .finish_non_exhaustive()
    }
}

/// Native reconstruction with allocation finished before mutable authority.
///
/// The borrowed selection keeps its own leases outside the commit call.
/// Rejected staged work retains the complete candidate for outside release.
pub struct StagedHistory<'a, State> {
    pub(crate) selection: &'a HistorySelection<State>,
    pub(crate) prepared: Rc<ResourcePreparedState<State>>,
}

impl<State> fmt::Debug for StagedHistory<'_, State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StagedHistory")
            .field("selection", &self.selection)
            .finish_non_exhaustive()
    }
}
