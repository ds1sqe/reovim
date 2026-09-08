use alloc::rc::Rc;
use core::fmt;

use reovim_kernel_interface::buffer::BufferStateStamp;
use reovim_kernel_interface::resource::state::{
    ResourcePreparedState, ResourceStateModelBinding, ResourceWorkingState,
};

/// Immutable retained observation; cloning copies a handle, never native data.
///
/// Runtime bounds outstanding leases and releases its Buffer borrow before
/// invoking policies. Native state must honor the Interface immutability
/// contract; an interior-mutable provider is not made safe by this handle.
pub struct StateLease<State> {
    pub(crate) model: ResourceStateModelBinding,
    pub(crate) stamp: BufferStateStamp,
    pub(crate) prepared: Rc<ResourcePreparedState<State>>,
}

impl<State> StateLease<State> {
    /// Returns the exact authoritative observation retained by this lease.
    #[must_use]
    pub const fn stamp(&self) -> BufferStateStamp {
        self.stamp
    }

    /// Borrows actual data under its model and accepted Buffer stamps.
    #[must_use]
    pub fn view(&self) -> ResourceWorkingState<'_, State> {
        ResourceWorkingState {
            model: self.model,
            stamp: self.stamp,
            state: self.prepared.state(),
        }
    }
}

impl<State> Clone for StateLease<State> {
    fn clone(&self) -> Self {
        Self {
            model: self.model,
            stamp: self.stamp,
            prepared: Rc::clone(&self.prepared),
        }
    }
}

impl<State> fmt::Debug for StateLease<State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StateLease")
            .field("stamp", &self.stamp)
            .finish_non_exhaustive()
    }
}
