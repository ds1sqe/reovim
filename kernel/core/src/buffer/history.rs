use alloc::rc::Rc;

use reovim_kernel_interface::buffer::plan::BufferPlanStamp;
use reovim_kernel_interface::buffer::{
    BufferHistoryDirection, BufferHistoryEdge, BufferHistoryNodeIdentity,
    BufferStateStamp,
};
use reovim_kernel_interface::resource::state::ResourceStateError;

use super::{Buffer, Retained};
use crate::{BufferError, HistorySelection, StateLease};

mod install;
mod stage;

impl<State> Buffer<State> {
    /// Selects an eligible parent or child without altering tree preference.
    /// `None` means no eligible edge. Opaque child IDs are never ordinals.
    /// Scans are bounded by the configured retained-node capacity.
    ///
    /// # Errors
    /// Returns a stale target, wrong plan owner or child supplied for Undo.
    pub fn select_history(
        &self,
        target: &BufferStateStamp,
        plan: BufferPlanStamp,
        direction: BufferHistoryDirection,
        child: Option<BufferHistoryNodeIdentity>,
    ) -> Result<Option<HistorySelection<State>>, BufferError> {
        if *target != self.stamp {
            return Err(BufferError::State(ResourceStateError::StateMismatch));
        }
        if plan.owner() != self.config.plan_owner {
            return Err(BufferError::State(
                ResourceStateError::PlanDescriptorMismatch,
            ));
        }
        if direction == BufferHistoryDirection::Undo && child.is_some() {
            return Err(BufferError::State(
                ResourceStateError::PreparationMismatch,
            ));
        }
        let current = self.node(self.current)?;
        let selected = match direction {
            BufferHistoryDirection::Undo => current.parent,
            BufferHistoryDirection::Redo => child.or(current.preferred_child),
        };
        let Some(index) = selected.and_then(|id| self.find_node(id)) else {
            return Ok(None);
        };
        let retained = self.node(index)?;
        if direction == BufferHistoryDirection::Redo
            && retained.parent != Some(self.stamp.history().node())
        {
            return Ok(None);
        }
        let edge = selected_edge(
            direction,
            self.stamp.history().node(),
            retained.stamp.history().node(),
        )?;
        let preferred = match (direction, child) {
            (BufferHistoryDirection::Redo, None) => current.preferred_child,
            _ => None,
        };
        Ok(Some(HistorySelection {
            current: self.observe(),
            retained: StateLease {
                model: self.config.model,
                stamp: retained.stamp,
                prepared: Rc::clone(&retained.prepared),
            },
            retained_index: index,
            edge,
            direction,
            plan,
            preferred,
        }))
    }

    fn find_node(&self, identity: BufferHistoryNodeIdentity) -> Option<usize> {
        self.nodes[..self.used].iter().position(|node| {
            node.as_ref()
                .is_some_and(|node| node.stamp.history().node() == identity)
        })
    }

    fn node(&self, index: usize) -> Result<&Retained<State>, BufferError> {
        self.nodes
            .get(index)
            .and_then(Option::as_ref)
            .ok_or(BufferError::State(ResourceStateError::PreparationMismatch))
    }

    fn validate_selection(
        &self,
        selection: &HistorySelection<State>,
    ) -> Result<(), BufferError> {
        if selection.current.stamp() != self.stamp
            || !Rc::ptr_eq(&selection.current.prepared, &self.current_state)
        {
            return Err(BufferError::State(ResourceStateError::StateMismatch));
        }
        let retained = self.node(selection.retained_index)?;
        if retained.stamp != selection.retained.stamp()
            || !Rc::ptr_eq(&retained.prepared, &selection.retained.prepared)
        {
            return Err(BufferError::State(
                ResourceStateError::PreparationMismatch,
            ));
        }
        let current = self.node(self.current)?;
        let linked = match selection.direction {
            BufferHistoryDirection::Undo => {
                current.parent == Some(selection.edge.parent())
            }
            BufferHistoryDirection::Redo => {
                retained.parent == Some(selection.edge.parent())
            }
        };
        if !linked
            || selection.edge.from(selection.direction)
                != self.stamp.history().node()
            || selection.edge.to(selection.direction)
                != retained.stamp.history().node()
            || selection
                .preferred
                .is_some_and(|child| current.preferred_child != Some(child))
        {
            return Err(BufferError::State(
                ResourceStateError::PreparationMismatch,
            ));
        }
        Ok(())
    }
}

fn selected_edge(
    direction: BufferHistoryDirection,
    current: BufferHistoryNodeIdentity,
    retained: BufferHistoryNodeIdentity,
) -> Result<BufferHistoryEdge, BufferError> {
    let edge = match direction {
        BufferHistoryDirection::Undo => {
            BufferHistoryEdge::new(retained, current)
        }
        BufferHistoryDirection::Redo => {
            BufferHistoryEdge::new(current, retained)
        }
    };
    edge.map_err(|_| {
        BufferError::State(ResourceStateError::PreparationMismatch)
    })
}
