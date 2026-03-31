//! Rope data structure for efficient text storage.
//!
//! A balanced B-tree of text chunks providing O(log n) insert/delete,
//! O(1) clone via structural sharing (`Arc<Node>`), and O(log n)
//! position conversion.
//!
//! # Design
//!
//! Leaf nodes hold text chunks of up to [`MAX_CHUNK_BYTES`]. Chunks are
//! split on newline boundaries so `line(idx)` returns `&str` into a
//! single leaf — zero allocation.
//!
//! Each node caches [`Metrics`] (byte length, char count, line count)
//! for O(log n) navigation by any dimension.
//!
//! Structural sharing via `Arc<RopeNode>` makes clone O(1). Mutations
//! return a new [`Rope`] sharing unchanged subtrees with the original —
//! only the path from root to the modified leaf is copied (O(log n)).
//! This enables:
//! - **Undo snapshots**: `block::Snapshot::capture()` clones the buffer's
//!   rope in O(1) instead of deep-copying every line.
//! - **Buffer clone**: `Buffer::clone()` is O(1) for any buffer size.
//! - **Crash recovery**: Snapshots share structure with the live buffer,
//!   so memory overhead is proportional to the number of edits, not
//!   the number of snapshots.
//!
//! # Invariants
//!
//! - Every non-last leaf ends with `'\n'` (newline-aligned chunks).
//!   This guarantees no line spans multiple leaves.
//! - Internal nodes have 4..=[`B_MAX`] children (except the root,
//!   which may have fewer).
//! - All leaves are at the same depth (balanced B-tree).
//! - A single line longer than `MAX_CHUNK_BYTES` gets its own
//!   oversized leaf (the only exception to the size limit).

use std::{fmt, ops::Range, sync::Arc};

// ─── Constants ──────────────────────────────────────────────────────────────

/// Maximum children per internal node.
const B_MAX: usize = 8;

/// Target maximum bytes per leaf chunk.
const MAX_CHUNK_BYTES: usize = 1024;

// ─── Metrics ────────────────────────────────────────────────────────────────

/// Cached metrics for a rope subtree.
///
/// With the newline-alignment invariant, `line_count` is additive
/// across siblings: the total line count of an internal node equals
/// the sum of its children's line counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Metrics {
    /// Total bytes in this subtree.
    byte_len: usize,
    /// Total Unicode scalar values in this subtree.
    char_len: usize,
    /// Number of lines in this subtree.
    ///
    /// For a leaf ending with `'\n'`: equals the newline count.
    /// For a leaf NOT ending with `'\n'`: equals newline count + 1.
    /// For empty text: 0.
    line_count: usize,
}

impl Metrics {
    /// Compute metrics from a text string (single pass).
    fn from_text(text: &str) -> Self {
        if text.is_empty() {
            return Self::default();
        }
        let byte_len = text.len();
        let mut char_len: usize = 0;
        let mut newline_count: usize = 0;
        for c in text.chars() {
            char_len += 1;
            if c == '\n' {
                newline_count += 1;
            }
        }
        let line_count = if text.as_bytes()[byte_len - 1] == b'\n' {
            newline_count
        } else {
            newline_count + 1
        };
        Self {
            byte_len,
            char_len,
            line_count,
        }
    }

    /// Sum metrics from an iterator of child metrics.
    fn sum(iter: impl Iterator<Item = Self>) -> Self {
        let mut result = Self::default();
        for m in iter {
            result.byte_len += m.byte_len;
            result.char_len += m.char_len;
            result.line_count += m.line_count;
        }
        result
    }
}

// ─── Node ───────────────────────────────────────────────────────────────────

/// The kind of a rope node.
enum NodeKind {
    /// A leaf holding a text chunk.
    Leaf { text: String },
    /// An internal node with child pointers.
    Internal { children: Vec<Arc<RopeNode>> },
}

/// A node in the rope B-tree.
struct RopeNode {
    /// Cached aggregate metrics for this subtree.
    metrics: Metrics,
    /// Node payload (leaf text or internal children).
    kind: NodeKind,
}

impl RopeNode {
    /// Create a new leaf node.
    fn new_leaf(text: String) -> Arc<Self> {
        let metrics = Metrics::from_text(&text);
        Arc::new(Self {
            metrics,
            kind: NodeKind::Leaf { text },
        })
    }

    /// Create a new internal node from children.
    fn new_internal(children: Vec<Arc<Self>>) -> Arc<Self> {
        debug_assert!(!children.is_empty(), "internal node must have children");
        let metrics = Metrics::sum(children.iter().map(|c| c.metrics));
        Arc::new(Self {
            metrics,
            kind: NodeKind::Internal { children },
        })
    }

    /// Returns `true` if this node is a leaf.
    const fn is_leaf(&self) -> bool {
        matches!(self.kind, NodeKind::Leaf { .. })
    }
}

// ─── Rope ───────────────────────────────────────────────────────────────────

/// A rope data structure for efficient text storage.
///
/// Clone is O(1) — only the `Arc` root pointer is cloned.
/// Mutations return a new `Rope`, sharing unchanged subtrees.
///
/// # Example
///
/// ```ignore
/// let r = Rope::from_str("hello\nworld");
/// assert_eq!(r.line_count(), 2);
/// assert_eq!(r.line(0), Some("hello"));
/// let r2 = r.insert(5, " there");
/// assert_eq!(r2.line(0), Some("hello there"));
/// assert_eq!(r.line(0), Some("hello")); // original unchanged
/// ```
#[derive(Clone)]
pub struct Rope {
    /// Root of the B-tree. An empty rope uses an empty leaf.
    root: Arc<RopeNode>,
}

// ── Construction ────────────────────────────────────────────────────────────

impl Rope {
    /// Create an empty rope.
    pub fn new() -> Self {
        Self {
            root: RopeNode::new_leaf(String::new()),
        }
    }

    /// Create a rope from a string slice.
    pub fn from_str(s: &str) -> Self {
        if s.is_empty() {
            return Self::new();
        }
        let leaves = chunk_text_to_leaves(s);
        Self {
            root: build_tree(leaves),
        }
    }
}

// ── Accessors ───────────────────────────────────────────────────────────────

impl Rope {
    /// Returns `true` if the rope contains no text.
    pub fn is_empty(&self) -> bool {
        self.root.metrics.byte_len == 0
    }

    /// Total byte length of the text.
    pub fn byte_len(&self) -> usize {
        self.root.metrics.byte_len
    }

    /// Total number of Unicode scalar values.
    pub fn char_len(&self) -> usize {
        self.root.metrics.char_len
    }

    /// Number of lines in the rope.
    ///
    /// Uses line-separator semantics: an empty rope has 0 lines,
    /// `"hello"` has 1 line, `"hello\n"` has 2 lines (trailing empty line),
    /// `"hello\nworld"` has 2 lines.
    pub fn line_count(&self) -> usize {
        let count = self.root.metrics.line_count;
        // Trailing \n produces an empty line (line-separator semantics).
        // The internal Metrics use terminator semantics for correct tree
        // summation, so we adjust at the public API boundary.
        if count > 0 && last_byte(&self.root) == Some(b'\n') {
            count + 1
        } else {
            count
        }
    }

    /// Get a line by index (0-based).
    ///
    /// Returns `None` if `idx >= line_count()`.
    /// The returned `&str` borrows from a single leaf node — zero allocation.
    /// For the trailing empty line after a final `'\n'`, returns `Some("")`.
    pub fn line(&self, idx: usize) -> Option<&str> {
        if idx >= self.line_count() {
            return None;
        }
        // Trailing empty line: exists only when text ends with \n.
        // Not stored in any leaf, so return a static empty str.
        if idx == self.root.metrics.line_count {
            return Some("");
        }
        line_at(&self.root, idx)
    }

    /// Get the length of a line in characters.
    ///
    /// Returns `None` if `idx >= line_count()`.
    pub fn line_len(&self, idx: usize) -> Option<usize> {
        self.line(idx).map(|l| l.chars().count())
    }

    /// Materialize the full content as a `String`.
    ///
    /// O(n) — allocates and copies all text.
    pub fn content(&self) -> String {
        let mut buf = String::with_capacity(self.byte_len());
        collect_text(&self.root, &mut buf);
        buf
    }
}

// ── Position conversion ─────────────────────────────────────────────────────

impl Rope {
    /// Convert a (line, column) position to a byte offset.
    ///
    /// `col` is measured in Unicode scalar values (chars), not bytes.
    /// Clamps to valid ranges.
    pub fn position_to_byte(&self, line: usize, col: usize) -> usize {
        if self.is_empty() {
            return 0;
        }
        pos_to_byte(&self.root, line, col)
    }

    /// Convert a byte offset to a (line, column) position.
    ///
    /// Column is measured in Unicode scalar values (chars).
    pub fn byte_to_position(&self, byte_offset: usize) -> (usize, usize) {
        if self.is_empty() {
            return (0, 0);
        }
        byte_to_pos(&self.root, byte_offset)
    }

    /// Convert a char offset (Unicode scalar index) to a byte offset.
    pub fn char_to_byte(&self, char_offset: usize) -> usize {
        if self.is_empty() {
            return 0;
        }
        char_to_byte_offset(&self.root, char_offset)
    }

    /// Convert a byte offset to a char offset (Unicode scalar index).
    pub fn byte_to_char(&self, byte_offset: usize) -> usize {
        if self.is_empty() {
            return 0;
        }
        byte_to_char_offset(&self.root, byte_offset)
    }
}

// ── Mutation ────────────────────────────────────────────────────────────────

impl Rope {
    /// Insert text at a byte offset, returning a new rope.
    ///
    /// The original rope is unchanged (structural sharing via Arc).
    /// Panics in debug mode if `byte_offset` is not on a char boundary.
    pub fn insert(&self, byte_offset: usize, text: &str) -> Self {
        if text.is_empty() {
            return self.clone();
        }
        if self.is_empty() {
            return Self::from_str(text);
        }
        let offset = byte_offset.min(self.root.metrics.byte_len);
        let nodes = insert_at(&self.root, offset, text);
        Self {
            root: nodes_to_root(nodes),
        }
    }

    /// Remove a byte range, returning a new rope.
    ///
    /// The original rope is unchanged (structural sharing via Arc).
    /// Panics in debug mode if range boundaries are not on char boundaries.
    pub fn remove(&self, range: Range<usize>) -> Self {
        if range.is_empty() || self.is_empty() {
            return self.clone();
        }
        let s = range.start.min(self.byte_len());
        let e = range.end.min(self.byte_len());
        if s >= e {
            return self.clone();
        }
        let nodes = remove_range(&self.root, s..e);
        if nodes.is_empty() {
            Self::new()
        } else {
            Self {
                root: nodes_to_root(nodes),
            }
        }
    }
}

// ── Iterators ───────────────────────────────────────────────────────────────

impl Rope {
    /// Iterate over lines (yields `&str` per line).
    // Used in tests and Phase 3+ snapshot migration.
    #[allow(dead_code)]
    pub const fn lines(&self) -> RopeLines<'_> {
        RopeLines { rope: self, idx: 0 }
    }

    /// Iterate over raw leaf chunks (yields `&str` per chunk).
    pub fn chunks(&self) -> RopeChunks<'_> {
        RopeChunks {
            stack: vec![&*self.root],
        }
    }
}

/// Iterator over lines of a rope.
#[allow(dead_code)]
pub struct RopeLines<'a> {
    rope: &'a Rope,
    idx: usize,
}

impl<'a> Iterator for RopeLines<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let line = self.rope.line(self.idx)?;
        self.idx += 1;
        Some(line)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.rope.line_count().saturating_sub(self.idx);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for RopeLines<'_> {}

/// Iterator over raw leaf chunks of a rope.
pub struct RopeChunks<'a> {
    /// Stack for depth-first traversal (nodes to visit).
    stack: Vec<&'a RopeNode>,
}

impl<'a> Iterator for RopeChunks<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let node = self.stack.pop()?;
            match &node.kind {
                NodeKind::Leaf { text } => {
                    if text.is_empty() {
                        continue;
                    }
                    return Some(text.as_str());
                }
                NodeKind::Internal { children } => {
                    // Push children in reverse so leftmost is popped first
                    for child in children.iter().rev() {
                        self.stack.push(child);
                    }
                }
            }
        }
    }
}

// ── Trait Implementations ───────────────────────────────────────────────────

impl fmt::Debug for Rope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let content = self.content();
        let preview = if content.len() > 80 {
            format!("{}...", &content[..77])
        } else {
            content
        };
        f.debug_struct("Rope")
            .field("byte_len", &self.byte_len())
            .field("line_count", &self.line_count())
            .field("content", &preview)
            .finish()
    }
}

impl PartialEq for Rope {
    fn eq(&self, other: &Self) -> bool {
        if self.byte_len() != other.byte_len() {
            return false;
        }
        // Compare chunk by chunk
        let mut left = self.chunks();
        let mut right = other.chunks();
        let mut l_remaining: &str = "";
        let mut r_remaining: &str = "";
        loop {
            if l_remaining.is_empty() {
                match left.next() {
                    Some(chunk) => l_remaining = chunk,
                    None => return r_remaining.is_empty() && right.next().is_none(),
                }
            }
            if r_remaining.is_empty() {
                match right.next() {
                    Some(chunk) => r_remaining = chunk,
                    None => return l_remaining.is_empty(),
                }
            }
            let cmp_len = l_remaining.len().min(r_remaining.len());
            if l_remaining[..cmp_len] != r_remaining[..cmp_len] {
                return false;
            }
            l_remaining = &l_remaining[cmp_len..];
            r_remaining = &r_remaining[cmp_len..];
        }
    }
}

impl Eq for Rope {}

impl fmt::Display for Rope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for chunk in self.chunks() {
            f.write_str(chunk)?;
        }
        Ok(())
    }
}

impl Default for Rope {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Construction Helpers ───────────────────────────────────────────────────

/// Split text into leaf nodes at newline boundaries.
fn chunk_text_to_leaves(text: &str) -> Vec<Arc<RopeNode>> {
    debug_assert!(!text.is_empty());
    chunk_text(text)
        .into_iter()
        .map(RopeNode::new_leaf)
        .collect()
}

/// Split text into chunks at newline boundaries, each up to
/// `MAX_CHUNK_BYTES`. A line longer than `MAX_CHUNK_BYTES` gets
/// its own oversized chunk.
fn chunk_text(text: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= MAX_CHUNK_BYTES {
            chunks.push(remaining.to_string());
            break;
        }

        // Find the last '\n' within the first MAX_CHUNK_BYTES
        let safe_end = floor_char_boundary(remaining, MAX_CHUNK_BYTES);
        if let Some(pos) = remaining[..safe_end].rfind('\n') {
            // Split right after the newline
            chunks.push(remaining[..=pos].to_string());
            remaining = &remaining[pos + 1..];
        } else {
            // No newline within limit — find the next newline anywhere
            if let Some(pos) = remaining.find('\n') {
                chunks.push(remaining[..=pos].to_string());
                remaining = &remaining[pos + 1..];
            } else {
                // No newline at all — entire remaining is one oversized chunk
                chunks.push(remaining.to_string());
                break;
            }
        }
    }

    chunks
}

/// Find the largest char boundary <= `byte_idx`.
fn floor_char_boundary(s: &str, byte_idx: usize) -> usize {
    if byte_idx >= s.len() {
        return s.len();
    }
    let bytes = s.as_bytes();
    let mut idx = byte_idx;
    // UTF-8 continuation bytes have the pattern 10xxxxxx
    while idx > 0 && (bytes[idx] & 0xC0) == 0x80 {
        idx -= 1;
    }
    idx
}

/// Build a balanced B-tree from leaf nodes.
fn build_tree(mut nodes: Vec<Arc<RopeNode>>) -> Arc<RopeNode> {
    debug_assert!(!nodes.is_empty());
    if nodes.len() == 1 {
        return nodes.into_iter().next().expect("checked non-empty");
    }

    // Build bottom-up: group into internal nodes.
    while nodes.len() > B_MAX {
        let mut parents = Vec::new();
        let total = nodes.len();
        let mut i = 0;

        while i < total {
            let remaining = total - i;
            // Ensure every group has at least 4 children.
            // When remaining <= 2 * B_MAX, split evenly.
            let group_size = if remaining <= B_MAX {
                remaining
            } else if remaining <= 2 * B_MAX {
                // Split so both halves >= 4
                remaining.div_ceil(2)
            } else {
                B_MAX
            };

            let children: Vec<Arc<RopeNode>> = nodes[i..i + group_size].to_vec();
            parents.push(RopeNode::new_internal(children));
            i += group_size;
        }

        nodes = parents;
    }

    if nodes.len() == 1 {
        nodes.into_iter().next().expect("checked non-empty")
    } else {
        RopeNode::new_internal(nodes)
    }
}

/// Convert a list of replacement nodes (from insert/remove) into a root.
fn nodes_to_root(nodes: Vec<Arc<RopeNode>>) -> Arc<RopeNode> {
    match nodes.len() {
        0 => RopeNode::new_leaf(String::new()),
        1 => nodes.into_iter().next().expect("len==1"),
        n if n <= B_MAX => RopeNode::new_internal(nodes),
        _ => build_tree(nodes),
    }
}

// ─── Navigation Helpers ─────────────────────────────────────────────────────

/// Return the last byte in the tree (O(log n) — walks to rightmost leaf).
fn last_byte(node: &RopeNode) -> Option<u8> {
    match &node.kind {
        NodeKind::Leaf { text } => text.as_bytes().last().copied(),
        NodeKind::Internal { children } => children.last().and_then(|c| last_byte(c)),
    }
}

/// Find line `idx` within a node's subtree.
fn line_at(node: &RopeNode, idx: usize) -> Option<&str> {
    match &node.kind {
        NodeKind::Leaf { text } => {
            let mut current = 0usize;
            let mut start = 0usize;
            for (i, &byte) in text.as_bytes().iter().enumerate() {
                if byte == b'\n' {
                    if current == idx {
                        return Some(&text[start..i]);
                    }
                    current += 1;
                    start = i + 1;
                }
            }
            // Last segment (no trailing newline)
            if current == idx {
                return Some(&text[start..]);
            }
            None
        }
        NodeKind::Internal { children } => {
            let mut remaining = idx;
            for child in children {
                let lc = child.metrics.line_count;
                if remaining < lc {
                    return line_at(child, remaining);
                }
                remaining -= lc;
            }
            None
        }
    }
}

/// Collect all text from a node into a buffer.
fn collect_text(node: &RopeNode, buf: &mut String) {
    match &node.kind {
        NodeKind::Leaf { text } => buf.push_str(text),
        NodeKind::Internal { children } => {
            for child in children {
                collect_text(child, buf);
            }
        }
    }
}

/// Convert (line, col) to byte offset within a node.
fn pos_to_byte(node: &RopeNode, target_line: usize, target_col: usize) -> usize {
    match &node.kind {
        NodeKind::Leaf { text } => {
            let mut current_line = 0usize;
            let mut line_start = 0usize;
            for (i, &b) in text.as_bytes().iter().enumerate() {
                if b == b'\n' {
                    if current_line == target_line {
                        let line = &text[line_start..i];
                        return line_start + char_to_byte_in(line, target_col);
                    }
                    current_line += 1;
                    line_start = i + 1;
                }
            }
            // Last line (no trailing newline)
            if current_line == target_line {
                let line = &text[line_start..];
                return line_start + char_to_byte_in(line, target_col);
            }
            text.len()
        }
        NodeKind::Internal { children } => {
            let mut remaining = target_line;
            let mut offset = 0usize;
            for child in children {
                if remaining < child.metrics.line_count {
                    return offset + pos_to_byte(child, remaining, target_col);
                }
                remaining -= child.metrics.line_count;
                offset += child.metrics.byte_len;
            }
            offset
        }
    }
}

/// Convert byte offset to (line, col) within a node.
fn byte_to_pos(node: &RopeNode, byte_offset: usize) -> (usize, usize) {
    match &node.kind {
        NodeKind::Leaf { text } => {
            let off = byte_offset.min(text.len());
            let mut line = 0usize;
            let mut line_start = 0usize;
            for (i, &b) in text.as_bytes()[..off].iter().enumerate() {
                if b == b'\n' {
                    line += 1;
                    line_start = i + 1;
                }
            }
            let col = text[line_start..off].chars().count();
            (line, col)
        }
        NodeKind::Internal { children } => {
            let mut remaining = byte_offset;
            let mut line_offset = 0usize;
            for child in children {
                if remaining < child.metrics.byte_len {
                    let (cl, col) = byte_to_pos(child, remaining);
                    return (line_offset + cl, col);
                }
                remaining -= child.metrics.byte_len;
                line_offset += child.metrics.line_count;
            }
            // At end of text — return last position
            children.last().map_or((0, 0), |last| {
                let (cl, col) = byte_to_pos(last, last.metrics.byte_len);
                (line_offset - last.metrics.line_count + cl, col)
            })
        }
    }
}

/// Convert char offset to byte offset within a node.
fn char_to_byte_offset(node: &RopeNode, char_offset: usize) -> usize {
    match &node.kind {
        NodeKind::Leaf { text } => text
            .char_indices()
            .nth(char_offset)
            .map_or(text.len(), |(b, _)| b),
        NodeKind::Internal { children } => {
            let mut remaining = char_offset;
            let mut offset = 0usize;
            for child in children {
                if remaining < child.metrics.char_len {
                    return offset + char_to_byte_offset(child, remaining);
                }
                remaining -= child.metrics.char_len;
                offset += child.metrics.byte_len;
            }
            offset
        }
    }
}

/// Convert byte offset to char offset within a node.
fn byte_to_char_offset(node: &RopeNode, byte_offset: usize) -> usize {
    match &node.kind {
        NodeKind::Leaf { text } => {
            let off = byte_offset.min(text.len());
            text[..off].chars().count()
        }
        NodeKind::Internal { children } => {
            let mut remaining = byte_offset;
            let mut char_off = 0usize;
            for child in children {
                if remaining < child.metrics.byte_len {
                    return char_off + byte_to_char_offset(child, remaining);
                }
                remaining -= child.metrics.byte_len;
                char_off += child.metrics.char_len;
            }
            char_off
        }
    }
}

/// Convert a char index to a byte offset within a `&str`.
fn char_to_byte_in(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map_or(s.len(), |(b, _)| b)
}

// ─── Mutation Helpers ───────────────────────────────────────────────────────

/// Insert text at `byte_offset` within `node`.
///
/// Returns a list of replacement nodes (usually 1, may be 2+ if a
/// leaf or internal node splits).
fn insert_at(node: &RopeNode, byte_offset: usize, text: &str) -> Vec<Arc<RopeNode>> {
    match &node.kind {
        NodeKind::Leaf { text: leaf_text } => {
            let offset = byte_offset.min(leaf_text.len());
            debug_assert!(
                leaf_text.is_char_boundary(offset),
                "insert byte_offset not on char boundary"
            );
            let mut new_text = String::with_capacity(leaf_text.len() + text.len());
            new_text.push_str(&leaf_text[..offset]);
            new_text.push_str(text);
            new_text.push_str(&leaf_text[offset..]);
            // Re-chunk to maintain size and newline-alignment invariants
            chunk_text_to_leaves(&new_text)
        }
        NodeKind::Internal { children } => {
            // Find which child to recurse into
            let (child_idx, local_offset) = find_child_for_byte(children, byte_offset);

            // Recurse
            let replacement = insert_at(&children[child_idx], local_offset, text);

            // Build new children list with replacement
            let mut new_children = Vec::with_capacity(children.len() + replacement.len());
            new_children.extend(children[..child_idx].iter().cloned());
            new_children.extend(replacement);
            new_children.extend(children[child_idx + 1..].iter().cloned());

            // Split if too many children
            if new_children.len() <= B_MAX {
                vec![RopeNode::new_internal(new_children)]
            } else {
                split_children(&new_children)
            }
        }
    }
}

/// Remove a byte range from `node`.
///
/// Returns replacement nodes (0 if entirely removed, 1+ otherwise).
fn remove_range(node: &RopeNode, range: Range<usize>) -> Vec<Arc<RopeNode>> {
    if range.is_empty() {
        return vec![Arc::new(RopeNode {
            metrics: node.metrics,
            kind: match &node.kind {
                NodeKind::Leaf { text } => NodeKind::Leaf { text: text.clone() },
                NodeKind::Internal { children } => NodeKind::Internal {
                    children: children.clone(),
                },
            },
        })];
    }

    match &node.kind {
        NodeKind::Leaf { text } => {
            let s = range.start.min(text.len());
            let e = range.end.min(text.len());
            debug_assert!(
                text.is_char_boundary(s) && text.is_char_boundary(e),
                "remove range not on char boundaries"
            );
            if s == 0 && e >= text.len() {
                return vec![];
            }
            let mut new_text = String::with_capacity(text.len() - (e - s));
            new_text.push_str(&text[..s]);
            new_text.push_str(&text[e..]);
            if new_text.is_empty() {
                vec![]
            } else {
                chunk_text_to_leaves(&new_text)
            }
        }
        NodeKind::Internal { children } => {
            let mut new_children: Vec<Arc<RopeNode>> = Vec::new();
            let mut offset = 0usize;
            let mut modified = false;

            for child in children {
                let cb = child.metrics.byte_len;
                let cs = offset;
                let ce = offset + cb;
                offset = ce;

                if range.end <= cs || range.start >= ce {
                    // No overlap — keep as-is
                    new_children.push(Arc::clone(child));
                } else if range.start <= cs && range.end >= ce {
                    // Entirely within range — remove
                    modified = true;
                } else {
                    // Partial overlap — recurse
                    let ls = range.start.saturating_sub(cs);
                    let le = (range.end - cs).min(cb);
                    let replacement = remove_range(child, ls..le);
                    modified = true;
                    new_children.extend(replacement);
                }
            }

            if modified {
                // Fixup newline alignment between adjacent children
                fixup_alignment(&mut new_children);
            }

            if new_children.is_empty() {
                vec![]
            } else if new_children.len() == 1 {
                // Collapse single-child wrapper
                vec![Arc::clone(&new_children[0])]
            } else {
                vec![RopeNode::new_internal(new_children)]
            }
        }
    }
}

/// Find which child contains `byte_offset`, returning `(child_index, local_offset)`.
fn find_child_for_byte(children: &[Arc<RopeNode>], byte_offset: usize) -> (usize, usize) {
    let mut offset = 0usize;
    for (i, child) in children.iter().enumerate() {
        let cb = child.metrics.byte_len;
        // Use <= so inserting at end of child i stays in child i
        if byte_offset <= offset + cb {
            return (i, byte_offset - offset);
        }
        offset += cb;
    }
    // Past end — go to last child
    let last = children.len() - 1;
    (last, children[last].metrics.byte_len)
}

/// Split a children vec that exceeds `B_MAX` into multiple internal nodes.
fn split_children(children: &[Arc<RopeNode>]) -> Vec<Arc<RopeNode>> {
    let mut result = Vec::new();
    let total = children.len();
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
        result.push(RopeNode::new_internal(children[i..i + group_size].to_vec()));
        i += group_size;
    }

    result
}

/// Get the last byte of a node's text (for alignment checking).
fn last_byte_of(node: &RopeNode) -> Option<u8> {
    match &node.kind {
        NodeKind::Leaf { text } => text.as_bytes().last().copied(),
        NodeKind::Internal { children } => children.last().and_then(|c| last_byte_of(c)),
    }
}

/// Fixup newline alignment: merge adjacent children where the left
/// one's text doesn't end with '\n'. This restores the invariant that
/// no line spans multiple leaves.
fn fixup_alignment(children: &mut Vec<Arc<RopeNode>>) {
    let mut i = 0;
    while i + 1 < children.len() {
        let left_last = last_byte_of(&children[i]);
        if left_last.is_some() && left_last != Some(b'\n') {
            // Left child's text doesn't end with newline — merge with right
            let mut merged = String::new();
            collect_text(&children[i], &mut merged);
            collect_text(&children[i + 1], &mut merged);
            let new_leaves = chunk_text_to_leaves(&merged);

            // If the merged result has many leaves, group them
            let replacement: Vec<Arc<RopeNode>> = if new_leaves.len() > B_MAX {
                vec![build_tree(new_leaves)]
            } else if new_leaves.len() > 1 {
                // If both original children were leaves, keep as leaves
                // If either was internal, wrap in a node
                if children[i].is_leaf() && children[i + 1].is_leaf() {
                    new_leaves
                } else {
                    vec![RopeNode::new_internal(new_leaves)]
                }
            } else {
                new_leaves
            };

            children.splice(i..=i + 1, replacement);
            // Don't increment — check the new node too
        } else {
            i += 1;
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "tests/rope.rs"]
mod tests;
