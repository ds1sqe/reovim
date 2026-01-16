//! Undo tree with branching support.
//!
//! Provides vim-style branching undo history where undoing and then
//! making new changes creates a new branch rather than losing history.

use {
    crate::mm::{Edit, Position},
    std::time::Instant,
};

/// A node in the undo tree.
///
/// Each node represents a set of edits made at a point in time,
/// along with cursor positions for proper restoration.
#[derive(Debug, Clone)]
pub struct UndoNode {
    /// Edits that were made (in order applied).
    edits: Vec<Edit>,
    /// Cursor position before these edits were applied.
    cursor_before: Position,
    /// Cursor position after these edits were applied.
    cursor_after: Position,
    /// When this node was created.
    timestamp: Instant,
    /// Parent node index (None for root).
    parent: Option<usize>,
    /// Child node indices (branches).
    children: Vec<usize>,
    /// Sequential change number.
    seq_num: u64,
}

impl UndoNode {
    /// Get the edits in this node.
    #[must_use]
    pub fn edits(&self) -> &[Edit] {
        &self.edits
    }

    /// Get cursor position before edits.
    #[must_use]
    pub const fn cursor_before(&self) -> Position {
        self.cursor_before
    }

    /// Get cursor position after edits.
    #[must_use]
    pub const fn cursor_after(&self) -> Position {
        self.cursor_after
    }

    /// Get timestamp when this node was created.
    #[must_use]
    pub const fn timestamp(&self) -> Instant {
        self.timestamp
    }

    /// Get sequential change number.
    #[must_use]
    pub const fn seq_num(&self) -> u64 {
        self.seq_num
    }

    /// Check if this is the root node.
    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    /// Get number of branches (children) from this node.
    #[must_use]
    pub const fn branch_count(&self) -> usize {
        self.children.len()
    }

    /// Get parent node index.
    #[must_use]
    pub const fn parent(&self) -> Option<usize> {
        self.parent
    }

    /// Get children node indices.
    #[must_use]
    pub fn children(&self) -> &[usize] {
        &self.children
    }
}

/// Result from an undo or redo operation.
///
/// Contains the edits to apply and the cursor position to restore.
#[derive(Debug, Clone)]
pub struct UndoResult {
    /// Edits to apply (already inverted for undo).
    pub edits: Vec<Edit>,
    /// Cursor position to restore.
    pub cursor: Position,
}

/// Undo tree with branching support.
///
/// Unlike a simple undo stack, an undo tree preserves all history.
/// When you undo and then make new changes, a new branch is created
/// rather than discarding the undone changes.
///
/// # Vim Comparison
///
/// This is similar to vim's `:undotree` feature. The `g-` and `g+`
/// commands traverse changes in time order (`seq_num`), while `u` and
/// `Ctrl-R` traverse the tree structure.
///
/// # Memory Management
///
/// The tree has a configurable maximum number of nodes. When exceeded,
/// the oldest nodes are pruned (but the path from root to current is
/// always preserved).
#[derive(Debug)]
pub struct UndoTree {
    /// All nodes in the tree.
    nodes: Vec<UndoNode>,
    /// Index of current position in the tree.
    current: usize,
    /// Sequential change counter.
    seq_counter: u64,
    /// Maximum number of nodes to retain.
    max_nodes: usize,
    /// Index of the preferred/active branch at each node.
    /// Used to remember which branch to follow on redo.
    active_branches: Vec<usize>,
}

impl Default for UndoTree {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoTree {
    /// Default maximum number of nodes.
    pub const DEFAULT_MAX_NODES: usize = 10000;

    /// Create a new undo tree.
    ///
    /// The tree starts with a root node representing the initial state.
    #[must_use]
    pub fn new() -> Self {
        Self::with_max_nodes(Self::DEFAULT_MAX_NODES)
    }

    /// Create a new undo tree with custom maximum nodes.
    #[must_use]
    pub fn with_max_nodes(max_nodes: usize) -> Self {
        let root = UndoNode {
            edits: Vec::new(),
            cursor_before: Position::default(),
            cursor_after: Position::default(),
            timestamp: Instant::now(),
            parent: None,
            children: Vec::new(),
            seq_num: 0,
        };

        Self {
            nodes: vec![root],
            current: 0,
            seq_counter: 0,
            max_nodes: max_nodes.max(1), // At least 1 node
            active_branches: vec![0],
        }
    }

    /// Push a new change onto the tree.
    ///
    /// Creates a new node as a child of the current node.
    /// If the current node already has children (we're in the middle
    /// of the tree after an undo), this creates a new branch.
    pub fn push(&mut self, edits: Vec<Edit>, cursor_before: Position, cursor_after: Position) {
        if edits.is_empty() {
            return;
        }

        self.seq_counter += 1;

        let new_node = UndoNode {
            edits,
            cursor_before,
            cursor_after,
            timestamp: Instant::now(),
            parent: Some(self.current),
            children: Vec::new(),
            seq_num: self.seq_counter,
        };

        let new_idx = self.nodes.len();
        self.nodes.push(new_node);

        // Add as child of current node
        self.nodes[self.current].children.push(new_idx);

        // Update active branch for current node
        let branch_idx = self.nodes[self.current].children.len() - 1;
        // Extend active_branches if needed
        while self.active_branches.len() <= self.current {
            self.active_branches.push(0);
        }
        self.active_branches[self.current] = branch_idx;

        // Ensure active_branches has entry for new node
        while self.active_branches.len() <= new_idx {
            self.active_branches.push(0);
        }

        // Move to new node
        self.current = new_idx;

        // Prune if over limit
        self.prune_if_needed();
    }

    /// Undo the current change.
    ///
    /// Moves to the parent node and returns the inverse edits
    /// along with the cursor position to restore.
    ///
    /// Returns `None` if at the root (nothing to undo).
    pub fn undo(&mut self) -> Option<UndoResult> {
        let current_node = &self.nodes[self.current];

        // Can't undo past root
        let parent_idx = current_node.parent?;

        // Get inverse edits and cursor to restore
        let result = UndoResult {
            edits: current_node.edits.iter().rev().map(Edit::inverse).collect(),
            cursor: current_node.cursor_before,
        };

        // Move to parent
        self.current = parent_idx;

        Some(result)
    }

    /// Redo the last undone change.
    ///
    /// Moves to the first child node (or the active branch) and
    /// returns the edits along with cursor position to restore.
    ///
    /// Returns `None` if there's nothing to redo.
    pub fn redo(&mut self) -> Option<UndoResult> {
        let current_node = &self.nodes[self.current];

        if current_node.children.is_empty() {
            return None;
        }

        // Get active branch index
        let branch_idx = self
            .active_branches
            .get(self.current)
            .copied()
            .unwrap_or(0)
            .min(current_node.children.len() - 1);

        let child_idx = current_node.children[branch_idx];
        let child_node = &self.nodes[child_idx];

        let result = UndoResult {
            edits: child_node.edits.clone(),
            cursor: child_node.cursor_after,
        };

        // Move to child
        self.current = child_idx;

        Some(result)
    }

    /// Redo a specific branch.
    ///
    /// Like `redo()` but follows a specific branch index.
    pub fn redo_branch(&mut self, branch_idx: usize) -> Option<UndoResult> {
        let current_node = &self.nodes[self.current];

        if branch_idx >= current_node.children.len() {
            return None;
        }

        // Update active branch
        if self.current < self.active_branches.len() {
            self.active_branches[self.current] = branch_idx;
        }

        let child_idx = current_node.children[branch_idx];
        let child_node = &self.nodes[child_idx];

        let result = UndoResult {
            edits: child_node.edits.clone(),
            cursor: child_node.cursor_after,
        };

        self.current = child_idx;

        Some(result)
    }

    /// Get the available branches (children) from current node.
    ///
    /// Returns indices that can be passed to `redo_branch()`.
    #[must_use]
    pub fn branches(&self) -> &[usize] {
        &self.nodes[self.current].children
    }

    /// Switch the active branch at current node.
    ///
    /// This affects which branch `redo()` will follow.
    /// Returns `false` if the branch index is invalid.
    pub fn switch_branch(&mut self, branch_idx: usize) -> bool {
        let current_node = &self.nodes[self.current];

        if branch_idx >= current_node.children.len() {
            return false;
        }

        while self.active_branches.len() <= self.current {
            self.active_branches.push(0);
        }
        self.active_branches[self.current] = branch_idx;

        true
    }

    /// Check if undo is possible.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.nodes[self.current].parent.is_some()
    }

    /// Check if redo is possible.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.nodes[self.current].children.is_empty()
    }

    /// Get the current node.
    #[must_use]
    pub fn current_node(&self) -> &UndoNode {
        &self.nodes[self.current]
    }

    /// Get the current node index.
    #[must_use]
    pub const fn current_index(&self) -> usize {
        self.current
    }

    /// Get total number of nodes in the tree.
    #[must_use]
    pub const fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get node by index.
    ///
    /// Returns `None` if index is out of bounds.
    #[must_use]
    pub fn node(&self, index: usize) -> Option<&UndoNode> {
        self.nodes.get(index)
    }

    /// Get all node indices for iteration.
    ///
    /// Returns a range that can be used to iterate over all valid node indices.
    #[must_use]
    pub const fn node_indices(&self) -> std::ops::Range<usize> {
        0..self.nodes.len()
    }

    /// Get active branch index at a node.
    ///
    /// Returns the preferred child index that `redo()` would follow
    /// from the specified node. Returns `None` if the node doesn't exist
    /// or has no active branch set.
    #[must_use]
    pub fn active_branch_at(&self, node_index: usize) -> Option<usize> {
        self.active_branches.get(node_index).copied()
    }

    /// Get the maximum nodes limit.
    #[must_use]
    pub const fn max_nodes(&self) -> usize {
        self.max_nodes
    }

    /// Set the maximum nodes limit.
    pub fn set_max_nodes(&mut self, max: usize) {
        self.max_nodes = max.max(1);
        self.prune_if_needed();
    }

    /// Get the current sequential change number.
    #[must_use]
    pub const fn seq_counter(&self) -> u64 {
        self.seq_counter
    }

    /// Prune old nodes if over the limit.
    ///
    /// This is a simple pruning strategy that removes the oldest
    /// nodes that are not on the path from root to current.
    fn prune_if_needed(&mut self) {
        if self.nodes.len() <= self.max_nodes {
            return;
        }

        // Build set of nodes on path from root to current
        let mut protected = vec![false; self.nodes.len()];
        let mut idx = self.current;
        loop {
            protected[idx] = true;
            match self.nodes[idx].parent {
                Some(parent) => idx = parent,
                None => break,
            }
        }

        // Find nodes to remove (oldest first, not protected)
        let to_remove = self.nodes.len() - self.max_nodes;
        let mut removed = 0;
        let mut remove_indices = Vec::new();

        for (i, node) in self.nodes.iter().enumerate() {
            if removed >= to_remove {
                break;
            }
            if !protected[i] && node.children.is_empty() {
                remove_indices.push(i);
                removed += 1;
            }
        }

        // Actually remove nodes (in reverse order to maintain indices)
        for &idx in remove_indices.iter().rev() {
            self.remove_node(idx);
        }
    }

    /// Remove a node from the tree.
    ///
    /// Updates parent's children list and adjusts indices.
    fn remove_node(&mut self, idx: usize) {
        if idx == 0 || idx == self.current {
            return; // Never remove root or current
        }

        // Remove from parent's children
        if let Some(parent_idx) = self.nodes[idx].parent {
            self.nodes[parent_idx].children.retain(|&c| c != idx);
        }

        // Update indices in all nodes
        for node in &mut self.nodes {
            if let Some(ref mut parent) = node.parent
                && *parent > idx
            {
                *parent -= 1;
            }
            for child in &mut node.children {
                if *child > idx {
                    *child -= 1;
                }
            }
        }

        // Update current if needed
        if self.current > idx {
            self.current -= 1;
        }

        // Update active_branches
        for branch in &mut self.active_branches {
            if *branch > idx {
                *branch -= 1;
            }
        }

        // Remove the node
        self.nodes.remove(idx);

        // Trim active_branches
        while self.active_branches.len() > self.nodes.len() {
            self.active_branches.pop();
        }
    }

    /// Clear all history except root.
    pub fn clear(&mut self) {
        let root_cursor = self.nodes[0].cursor_after;
        *self = Self::with_max_nodes(self.max_nodes);
        self.nodes[0].cursor_after = root_cursor;
    }
}
