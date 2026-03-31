//! Piece table B-tree for large file editing.
//!
//! A balanced B-tree of [`Piece`] entries, indexed by cumulative byte metrics.
//! Each piece references either the original file (mmap'd, read-only) or the
//! append-only add buffer.
//!
//! # Structural Sharing
//!
//! `PieceTree` uses `Arc<PieceNode>` for O(1) clone, following the same
//! pattern as the existing [`Rope`](super::rope::Rope) B-tree.  Mutations
//! copy only the path from root to the modified leaf (O(log n) nodes).
//!
//! # Invariants
//!
//! - Internal nodes have 2..=[`B_MAX`] children (root may have fewer).
//! - All leaves are at the same depth.
//! - Cumulative metrics (`byte_len`, `char_count`, `line_count`) in each
//!   internal node equal the sum of its children's metrics.

use std::sync::Arc;

// ─── Constants ──────────────────────────────────────────────────────────────

/// Maximum children per internal node.
const B_MAX: usize = 8;

/// Minimum children per non-root internal node (ceil(`B_MAX`/2)).
#[cfg(test)]
const B_MIN: usize = 4;

// ─── Types ──────────────────────────────────────────────────────────────────

/// Where a piece's bytes come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceSource {
    /// Byte range in the original file (mmap'd, read-only).
    Original {
        /// Starting byte offset in the original file.
        byte_start: u64,
        /// Number of bytes from the original file.
        byte_len: u64,
    },
    /// Range in the append-only add buffer.
    Add {
        /// Starting byte offset in the add buffer.
        offset: usize,
        /// Number of bytes from the add buffer.
        len: usize,
    },
}

/// Cached metrics for a piece.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PieceMetrics {
    /// Total bytes in this piece/subtree.
    pub byte_len: u64,
    /// Total Unicode scalar values.
    pub char_count: u64,
    /// Number of newline characters (`\n`).
    pub line_count: u64,
}

impl PieceMetrics {
    /// Compute metrics from a UTF-8 string.
    #[must_use]
    pub fn compute(s: &str) -> Self {
        Self {
            byte_len: s.len() as u64,
            char_count: s.chars().count() as u64,
            line_count: s.bytes().filter(|&b| b == b'\n').count() as u64,
        }
    }

    /// Sum two metrics.
    #[must_use]
    pub const fn add(self, other: Self) -> Self {
        Self {
            byte_len: self.byte_len + other.byte_len,
            char_count: self.char_count + other.char_count,
            line_count: self.line_count + other.line_count,
        }
    }

    /// Sum metrics from children.
    fn sum(children: &[Arc<PieceNode>]) -> Self {
        let mut result = Self::default();
        for c in children {
            result = result.add(c.metrics);
        }
        result
    }
}

/// A single piece in the piece table.
#[derive(Debug, Clone)]
pub struct Piece {
    /// Where the bytes come from.
    pub source: PieceSource,
    /// Cached metrics for this piece.
    pub metrics: PieceMetrics,
}

// ─── Node ───────────────────────────────────────────────────────────────────

/// The kind of a piece tree node.
enum PieceNodeKind {
    /// A leaf holding a single piece.
    Leaf(Piece),
    /// An internal node with child pointers.
    Internal(Vec<Arc<PieceNode>>),
}

/// A node in the piece tree B-tree.
pub struct PieceNode {
    /// Cached aggregate metrics for this subtree.
    metrics: PieceMetrics,
    /// Node payload.
    kind: PieceNodeKind,
}

impl PieceNode {
    /// Create a new leaf node.
    fn new_leaf(piece: Piece) -> Arc<Self> {
        Arc::new(Self {
            metrics: piece.metrics,
            kind: PieceNodeKind::Leaf(piece),
        })
    }

    /// Create a new internal node from children.
    fn new_internal(children: Vec<Arc<Self>>) -> Arc<Self> {
        debug_assert!(!children.is_empty(), "internal node must have children");
        let metrics = PieceMetrics::sum(&children);
        Arc::new(Self {
            metrics,
            kind: PieceNodeKind::Internal(children),
        })
    }
}

impl std::fmt::Debug for PieceNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            PieceNodeKind::Leaf(piece) => f
                .debug_struct("Leaf")
                .field("metrics", &self.metrics)
                .field("piece", piece)
                .finish(),
            PieceNodeKind::Internal(children) => f
                .debug_struct("Internal")
                .field("metrics", &self.metrics)
                .field("children", &children.len())
                .finish(),
        }
    }
}

// ─── PieceTree ──────────────────────────────────────────────────────────────

/// B-tree of pieces, indexed by cumulative `PieceMetrics`.
///
/// Structural sharing via `Arc<PieceNode>` for O(1) clone.
/// Same pattern as the existing Rope B-tree.
#[derive(Clone, Debug)]
pub struct PieceTree {
    /// Root of the B-tree. `None` for an empty tree.
    root: Option<Arc<PieceNode>>,
    /// Number of pieces (leaves) in the tree.
    piece_count: usize,
}

impl PieceTree {
    /// Create an empty piece tree.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            root: None,
            piece_count: 0,
        }
    }

    /// Create a piece tree with a single initial piece.
    #[must_use]
    pub fn from_piece(piece: Piece) -> Self {
        Self {
            root: Some(PieceNode::new_leaf(piece)),
            piece_count: 1,
        }
    }

    /// Total byte length of all pieces.
    #[must_use]
    pub fn byte_len(&self) -> u64 {
        self.root.as_ref().map_or(0, |r| r.metrics.byte_len)
    }

    /// Total character count.
    #[must_use]
    #[allow(dead_code)]
    pub fn char_count(&self) -> u64 {
        self.root.as_ref().map_or(0, |r| r.metrics.char_count)
    }

    /// Total newline count.
    #[must_use]
    #[allow(dead_code)]
    pub fn line_count(&self) -> u64 {
        self.root.as_ref().map_or(0, |r| r.metrics.line_count)
    }

    /// Number of pieces (leaves) in the tree.
    #[must_use]
    pub const fn piece_count(&self) -> usize {
        self.piece_count
    }

    /// Whether the tree is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Insert a piece at a byte offset.
    ///
    /// If `byte_offset` falls in the middle of an existing piece, that piece
    /// is split and the new piece is inserted between the two halves.
    #[must_use]
    pub fn insert(&self, byte_offset: u64, piece: Piece) -> Self {
        if self.is_empty() {
            return Self::from_piece(piece);
        }

        let root = self.root.as_ref().expect("not empty");
        let leaves = collect_leaves(root);

        let mut new_leaves = Vec::with_capacity(leaves.len() + 2);
        let mut cumulative = 0u64;
        let mut inserted = false;

        for leaf in &leaves {
            if !inserted && cumulative + leaf.metrics.byte_len >= byte_offset {
                let offset_in_piece = byte_offset - cumulative;
                if offset_in_piece == 0 {
                    new_leaves.push(piece.clone());
                    new_leaves.push((*leaf).clone());
                } else if offset_in_piece >= leaf.metrics.byte_len {
                    new_leaves.push((*leaf).clone());
                    new_leaves.push(piece.clone());
                } else {
                    let (left, right) = split_piece(leaf, offset_in_piece);
                    new_leaves.push(left);
                    new_leaves.push(piece.clone());
                    new_leaves.push(right);
                }
                inserted = true;
            } else {
                new_leaves.push((*leaf).clone());
            }
            cumulative += leaf.metrics.byte_len;
        }

        if !inserted {
            new_leaves.push(piece);
        }

        Self::from_leaves(new_leaves)
    }

    /// Delete a byte range from the tree.
    ///
    /// Returns a new tree with the range removed.
    #[must_use]
    pub fn delete(&self, byte_start: u64, byte_len: u64) -> Self {
        if byte_len == 0 || self.is_empty() {
            return self.clone();
        }

        let root = self.root.as_ref().expect("not empty");
        let leaves = collect_leaves(root);

        let byte_end = byte_start + byte_len;
        let mut new_leaves = Vec::with_capacity(leaves.len());
        let mut cumulative = 0u64;

        for leaf in &leaves {
            let piece_start = cumulative;
            let piece_end = cumulative + leaf.metrics.byte_len;
            cumulative = piece_end;

            if piece_end <= byte_start || piece_start >= byte_end {
                new_leaves.push((*leaf).clone());
            } else if piece_start >= byte_start && piece_end <= byte_end {
                // Entirely inside delete range — skip
            } else if piece_start < byte_start && piece_end > byte_end {
                let left_len = byte_start - piece_start;
                let right_start = byte_end - piece_start;
                let (left, _) = split_piece(leaf, left_len);
                let (_, right) = split_piece(leaf, right_start);
                new_leaves.push(left);
                new_leaves.push(right);
            } else if piece_start < byte_start {
                let keep_len = byte_start - piece_start;
                let (left, _) = split_piece(leaf, keep_len);
                new_leaves.push(left);
            } else {
                let skip_len = byte_end - piece_start;
                let (_, right) = split_piece(leaf, skip_len);
                if right.metrics.byte_len > 0 {
                    new_leaves.push(right);
                }
            }
        }

        if new_leaves.is_empty() {
            Self::new()
        } else {
            Self::from_leaves(new_leaves)
        }
    }

    /// Find which piece contains a byte offset.
    ///
    /// Returns `(piece, offset_within_piece)` or `None` if the tree is empty.
    #[must_use]
    #[allow(dead_code)]
    pub fn piece_at(&self, byte_offset: u64) -> Option<(&Piece, u64)> {
        let root = self.root.as_ref()?;
        piece_at_offset(root, byte_offset)
    }

    /// Iterate over all pieces in order.
    pub fn iter_pieces(&self) -> PieceIter<'_> {
        let mut stack = Vec::new();
        if let Some(root) = &self.root {
            stack.push(&**root);
        }
        PieceIter { stack }
    }

    /// Build a tree from a list of pieces (leaves).
    fn from_leaves(pieces: Vec<Piece>) -> Self {
        if pieces.is_empty() {
            return Self::new();
        }
        let count = pieces.len();
        let leaf_nodes: Vec<Arc<PieceNode>> =
            pieces.into_iter().map(PieceNode::new_leaf).collect();
        Self {
            root: Some(build_tree(leaf_nodes)),
            piece_count: count,
        }
    }
}

impl Default for PieceTree {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Piece splitting ────────────────────────────────────────────────────────

/// Split a piece at a byte offset within it.
///
/// Splits the source range and proportionally distributes metrics.
/// The `VirtualBuffer` rebuilds exact metrics after mutation.
fn split_piece(piece: &Piece, byte_offset: u64) -> (Piece, Piece) {
    let total_bytes = piece.metrics.byte_len;
    debug_assert!(byte_offset <= total_bytes);

    let left_bytes = byte_offset;
    let right_bytes = total_bytes - byte_offset;

    let (left_source, right_source) = match piece.source {
        PieceSource::Original { byte_start, byte_len } => {
            debug_assert!(byte_offset <= byte_len);
            (
                PieceSource::Original {
                    byte_start,
                    byte_len: left_bytes,
                },
                PieceSource::Original {
                    byte_start: byte_start + left_bytes,
                    byte_len: right_bytes,
                },
            )
        }
        PieceSource::Add { offset, len } => {
            #[allow(clippy::cast_possible_truncation)]
            let left_len = byte_offset as usize;
            debug_assert!(left_len <= len);
            (
                PieceSource::Add {
                    offset,
                    len: left_len,
                },
                PieceSource::Add {
                    offset: offset + left_len,
                    len: len - left_len,
                },
            )
        }
    };

    // Proportional metric split (approximate for char/line counts).
    #[allow(clippy::cast_precision_loss)]
    let ratio = if total_bytes > 0 {
        left_bytes as f64 / total_bytes as f64
    } else {
        0.0
    };

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let left_chars = (piece.metrics.char_count as f64 * ratio).round() as u64;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let left_lines = (piece.metrics.line_count as f64 * ratio).round() as u64;

    let left = Piece {
        source: left_source,
        metrics: PieceMetrics {
            byte_len: left_bytes,
            char_count: left_chars,
            line_count: left_lines,
        },
    };

    let right = Piece {
        source: right_source,
        metrics: PieceMetrics {
            byte_len: right_bytes,
            char_count: piece.metrics.char_count - left_chars,
            line_count: piece.metrics.line_count - left_lines,
        },
    };

    (left, right)
}

// ─── Tree construction ──────────────────────────────────────────────────────

/// Build a balanced B-tree from leaf nodes.
fn build_tree(mut nodes: Vec<Arc<PieceNode>>) -> Arc<PieceNode> {
    debug_assert!(!nodes.is_empty());

    if nodes.len() == 1 {
        return nodes.into_iter().next().expect("checked non-empty");
    }

    while nodes.len() > B_MAX {
        let mut parents = Vec::new();
        let total = nodes.len();
        let mut i = 0;

        while i < total {
            let remaining = total - i;
            let group_size = if remaining <= B_MAX {
                remaining
            } else if remaining <= 2 * B_MAX {
                remaining.div_ceil(2)
            } else {
                B_MAX
            };

            let children: Vec<Arc<PieceNode>> = nodes[i..i + group_size].to_vec();
            parents.push(PieceNode::new_internal(children));
            i += group_size;
        }

        nodes = parents;
    }

    if nodes.len() == 1 {
        nodes.into_iter().next().expect("checked non-empty")
    } else {
        PieceNode::new_internal(nodes)
    }
}

// ─── Tree traversal ─────────────────────────────────────────────────────────

/// Collect all leaf pieces from a tree in order.
fn collect_leaves(node: &PieceNode) -> Vec<&Piece> {
    let mut result = Vec::new();
    collect_leaves_rec(node, &mut result);
    result
}

fn collect_leaves_rec<'a>(node: &'a PieceNode, out: &mut Vec<&'a Piece>) {
    match &node.kind {
        PieceNodeKind::Leaf(piece) => out.push(piece),
        PieceNodeKind::Internal(children) => {
            for child in children {
                collect_leaves_rec(child, out);
            }
        }
    }
}

/// Find the piece containing a byte offset.
#[allow(dead_code)]
fn piece_at_offset(node: &PieceNode, byte_offset: u64) -> Option<(&Piece, u64)> {
    match &node.kind {
        PieceNodeKind::Leaf(piece) => {
            if byte_offset <= piece.metrics.byte_len && piece.metrics.byte_len > 0 {
                Some((piece, byte_offset))
            } else {
                None
            }
        }
        PieceNodeKind::Internal(children) => {
            let mut remaining = byte_offset;
            let last_idx = children.len() - 1;
            for (idx, child) in children.iter().enumerate() {
                if remaining < child.metrics.byte_len {
                    return piece_at_offset(child, remaining);
                }
                if remaining == child.metrics.byte_len && idx == last_idx {
                    return piece_at_offset(child, remaining);
                }
                remaining -= child.metrics.byte_len;
            }
            None
        }
    }
}

// ─── Iterator ───────────────────────────────────────────────────────────────

/// Iterator over pieces in a `PieceTree`.
pub struct PieceIter<'a> {
    stack: Vec<&'a PieceNode>,
}

impl<'a> Iterator for PieceIter<'a> {
    type Item = &'a Piece;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let node = self.stack.pop()?;
            match &node.kind {
                PieceNodeKind::Leaf(piece) => return Some(piece),
                PieceNodeKind::Internal(children) => {
                    for child in children.iter().rev() {
                        self.stack.push(child);
                    }
                }
            }
        }
    }
}

// ─── Validation (for tests) ────────────────────────────────────────────────

impl PieceTree {
    /// Number of strong references to the root node.
    /// Returns 0 for an empty tree.
    #[cfg(test)]
    pub(crate) fn root_strong_count(&self) -> usize {
        self.root.as_ref().map_or(0, Arc::strong_count)
    }

    /// Validate B-tree invariants.
    ///
    /// # Errors
    ///
    /// Returns an error message string on invariant violation.
    #[cfg(test)]
    pub(crate) fn validate(&self) -> Result<(), String> {
        let Some(root) = &self.root else {
            return Ok(());
        };

        let depth = tree_depth(root);
        validate_node(root, depth, true)?;

        let actual_count = self.iter_pieces().count();
        if actual_count != self.piece_count {
            return Err(format!(
                "piece_count mismatch: stored={}, actual={}",
                self.piece_count, actual_count
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
fn tree_depth(node: &PieceNode) -> usize {
    match &node.kind {
        PieceNodeKind::Leaf(_) => 0,
        PieceNodeKind::Internal(children) => {
            1 + tree_depth(children.first().expect("non-empty"))
        }
    }
}

#[cfg(test)]
fn validate_node(node: &PieceNode, expected_depth: usize, is_root: bool) -> Result<(), String> {
    match &node.kind {
        PieceNodeKind::Leaf(_) => {
            if expected_depth != 0 {
                return Err(format!(
                    "leaf at wrong depth: expected {expected_depth}, got 0"
                ));
            }
            Ok(())
        }
        PieceNodeKind::Internal(children) => {
            if expected_depth == 0 {
                return Err("internal node at leaf depth".to_string());
            }

            if !is_root && children.len() < B_MIN {
                return Err(format!(
                    "underfull internal node: {} children (min {B_MIN})",
                    children.len(),
                ));
            }
            if children.len() > B_MAX {
                return Err(format!(
                    "overfull internal node: {} children (max {B_MAX})",
                    children.len(),
                ));
            }

            let computed = PieceMetrics::sum(children);
            if computed != node.metrics {
                return Err(format!(
                    "metrics mismatch: stored={:?}, computed={:?}",
                    node.metrics, computed
                ));
            }

            for child in children {
                validate_node(child, expected_depth - 1, false)?;
            }

            Ok(())
        }
    }
}
