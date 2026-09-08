//! Explicit parent/child relations in Core-owned Buffer History.

use core::fmt;

use super::{BufferHistoryDirection, BufferHistoryNodeIdentity};

/// An edge cannot make one node its own parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferHistoryEdgeError {
    /// Parent and child identify the same retained node.
    SelfEdge,
}

impl fmt::Display for BufferHistoryEdgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("History edge has the same parent and child")
    }
}

impl core::error::Error for BufferHistoryEdgeError {}

/// One selected parent/child edge, scoped by the enclosing History stamps.
///
/// Construction checks only structural non-identity. Core proves membership,
/// edit-child freshness, retention and selection before atomic installation;
/// a caller-provided edge is not authority to add or traverse a tree link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferHistoryEdge {
    parent: BufferHistoryNodeIdentity,
    child: BufferHistoryNodeIdentity,
}

impl BufferHistoryEdge {
    /// Binds explicit endpoints without ordering or node-ID arithmetic.
    ///
    /// # Errors
    ///
    /// Returns `SelfEdge` when both endpoints identify the same node.
    pub const fn new(
        parent: BufferHistoryNodeIdentity,
        child: BufferHistoryNodeIdentity,
    ) -> Result<Self, BufferHistoryEdgeError> {
        if parent.value() == child.value() {
            return Err(BufferHistoryEdgeError::SelfEdge);
        }
        Ok(Self { parent, child })
    }

    /// Returns the parent node of this explicit edge.
    #[must_use]
    pub const fn parent(self) -> BufferHistoryNodeIdentity {
        self.parent
    }

    /// Returns the child node of this explicit edge.
    #[must_use]
    pub const fn child(self) -> BufferHistoryNodeIdentity {
        self.child
    }

    /// Returns the node from which a selected traversal starts.
    #[must_use]
    pub const fn from(
        self,
        direction: BufferHistoryDirection,
    ) -> BufferHistoryNodeIdentity {
        match direction {
            BufferHistoryDirection::Undo => self.child,
            BufferHistoryDirection::Redo => self.parent,
        }
    }

    /// Returns the node selected by the traversal.
    #[must_use]
    pub const fn to(
        self,
        direction: BufferHistoryDirection,
    ) -> BufferHistoryNodeIdentity {
        match direction {
            BufferHistoryDirection::Undo => self.parent,
            BufferHistoryDirection::Redo => self.child,
        }
    }
}
