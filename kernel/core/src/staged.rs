use alloc::rc::Rc;
use core::fmt;

use reovim_kernel_interface::resource::state::ResourcePreparedState;

/// Owned candidate with its lease allocation completed before installation.
///
/// Only Core can construct it. Staging does not reserve a version, create a
/// History node or grant installation: the final mutable call rechecks all
/// current state, capacity and cancellation fences.
pub struct StagedEdit<State> {
    pub(crate) prepared: Rc<ResourcePreparedState<State>>,
}

impl<State> fmt::Debug for StagedEdit<State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StagedEdit")
            .field("expected", &self.prepared.stamp())
            .finish_non_exhaustive()
    }
}
