//! Undo tree serialization types.
//!
//! This module provides serde-enabled types for serializing undo trees
//! to disk. These are "snapshot" types that mirror kernel types but
//! can be serialized.
//!
//! # Architecture
//!
//! Following the protocol layer pattern, these types exist separately
//! from kernel types to maintain kernel purity (no serde in kernel).
//!
//! # File Format
//!
//! Undo files use `MessagePack` binary format with a 4-byte magic header:
//! - Magic: `RUND` (Reovim Undo)
//! - Payload: `MessagePack`-encoded `UndoFileFormat`

use serde::{Deserialize, Serialize};

/// Magic bytes for undo file format.
pub const UNDO_FILE_MAGIC: [u8; 4] = *b"RUND";

/// Current undo file format version.
pub const UNDO_FILE_VERSION: u32 = 1;

/// Undo file format with header, metadata, and tree data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoFileFormat {
    /// File format version for forward compatibility.
    pub version: u32,
    /// Original file path this undo history belongs to.
    pub original_path: String,
    /// Unix timestamp when undo file was created.
    pub created_at: u64,
    /// Reovim version that created this file.
    pub reovim_version: String,
    /// The serialized undo tree.
    pub tree: SerializableUndoTree,
}

/// Serializable representation of `UndoTree`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableUndoTree {
    /// All nodes in the tree.
    pub nodes: Vec<SerializableUndoNode>,
    /// Index of current position in the tree.
    pub current: usize,
    /// Sequential change counter.
    pub seq_counter: u64,
    /// Maximum number of nodes to retain.
    pub max_nodes: usize,
    /// Index of the preferred/active branch at each node.
    pub active_branches: Vec<usize>,
}

/// Serializable representation of `UndoNode`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableUndoNode {
    /// Edits that were made (in order applied).
    pub edits: Vec<SerializableEdit>,
    /// Cursor position before these edits were applied.
    pub cursor_before: SerializablePosition,
    /// Cursor position after these edits were applied.
    pub cursor_after: SerializablePosition,
    /// Relative time in seconds since tree creation.
    /// Note: `Instant` is not serializable; we store relative time.
    pub relative_time_secs: f64,
    /// Parent node index (None for root).
    pub parent: Option<usize>,
    /// Child node indices (branches).
    pub children: Vec<usize>,
    /// Sequential change number.
    pub seq_num: u64,
    /// Origin of this edit (which client made it).
    #[serde(default)]
    pub origin: SerializableEditOrigin,
}

/// Serializable representation of `EditOrigin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SerializableEditOrigin {
    /// Edit made by a specific client.
    Client(usize),
    /// System-generated edit.
    #[default]
    System,
}

/// Serializable position (line, column).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SerializablePosition {
    /// Line number (0-indexed).
    pub line: usize,
    /// Column number (0-indexed).
    pub column: usize,
}

impl SerializablePosition {
    /// Create a new position.
    #[must_use]
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// Serializable edit operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SerializableEdit {
    /// Insert text at a position.
    Insert {
        /// Position where text was inserted.
        position: SerializablePosition,
        /// Text that was inserted.
        text: String,
    },
    /// Delete text at a position.
    Delete {
        /// Position where text was deleted.
        position: SerializablePosition,
        /// Text that was deleted.
        text: String,
    },
}

impl UndoFileFormat {
    /// Create a new undo file format with metadata.
    #[must_use]
    pub fn new(original_path: String, tree: SerializableUndoTree) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        Self {
            version: UNDO_FILE_VERSION,
            original_path,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            reovim_version: env!("CARGO_PKG_VERSION").to_string(),
            tree,
        }
    }

    /// Serialize to `MessagePack` bytes with magic header.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        let mut bytes = UNDO_FILE_MAGIC.to_vec();
        let payload = rmp_serde::to_vec(self)?;
        bytes.extend(payload);
        Ok(bytes)
    }

    /// Deserialize from bytes (validates magic header).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File is too short (< 4 bytes)
    /// - Magic bytes don't match
    /// - `MessagePack` deserialization fails
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, UndoFileError> {
        if bytes.len() < 4 {
            return Err(UndoFileError::TooShort);
        }
        if bytes[..4] != UNDO_FILE_MAGIC {
            return Err(UndoFileError::InvalidMagic);
        }
        rmp_serde::from_slice(&bytes[4..]).map_err(UndoFileError::Deserialize)
    }
}

/// Errors that can occur when reading undo files.
#[derive(Debug)]
pub enum UndoFileError {
    /// File is too short to contain magic bytes.
    TooShort,
    /// Invalid magic bytes (not an undo file).
    InvalidMagic,
    /// Deserialization failed.
    Deserialize(rmp_serde::decode::Error),
    /// I/O error.
    Io(std::io::Error),
}

impl std::fmt::Display for UndoFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "File too short to be valid undo file"),
            Self::InvalidMagic => write!(f, "Invalid undo file magic bytes"),
            Self::Deserialize(e) => write!(f, "Failed to deserialize undo file: {e}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for UndoFileError {}

impl From<std::io::Error> for UndoFileError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ============================================================================
// Kernel Type Conversions
// ============================================================================

use {
    reovim_kernel::api::v1::{Edit, EditOrigin, Position, UndoTree},
    std::time::Duration,
};

// ----------------------------------------------------------------------------
// Position conversions
// ----------------------------------------------------------------------------

impl From<Position> for SerializablePosition {
    fn from(pos: Position) -> Self {
        Self {
            line: pos.line,
            column: pos.column,
        }
    }
}

impl From<SerializablePosition> for Position {
    fn from(pos: SerializablePosition) -> Self {
        Self::new(pos.line, pos.column)
    }
}

// ----------------------------------------------------------------------------
// Edit conversions
// ----------------------------------------------------------------------------

impl From<&Edit> for SerializableEdit {
    fn from(edit: &Edit) -> Self {
        match edit {
            Edit::Insert { position, text } => Self::Insert {
                position: (*position).into(),
                text: text.clone(),
            },
            Edit::Delete { position, text } => Self::Delete {
                position: (*position).into(),
                text: text.clone(),
            },
        }
    }
}

impl From<SerializableEdit> for Edit {
    fn from(edit: SerializableEdit) -> Self {
        match edit {
            SerializableEdit::Insert { position, text } => Self::insert(position.into(), text),
            SerializableEdit::Delete { position, text } => Self::delete(position.into(), text),
        }
    }
}

// ----------------------------------------------------------------------------
// EditOrigin conversions
// ----------------------------------------------------------------------------

impl From<EditOrigin> for SerializableEditOrigin {
    fn from(origin: EditOrigin) -> Self {
        match origin {
            EditOrigin::Client(id) => Self::Client(id),
            EditOrigin::System => Self::System,
        }
    }
}

impl From<SerializableEditOrigin> for EditOrigin {
    fn from(origin: SerializableEditOrigin) -> Self {
        match origin {
            SerializableEditOrigin::Client(id) => Self::Client(id),
            SerializableEditOrigin::System => Self::System,
        }
    }
}

// ----------------------------------------------------------------------------
// UndoTree conversions
// ----------------------------------------------------------------------------

/// Convert a kernel `UndoTree` to a serializable representation.
///
/// Timestamps are converted to relative seconds from the tree's root node.
#[must_use]
pub fn from_undo_tree(tree: &UndoTree) -> SerializableUndoTree {
    // Get reference timestamp from root node
    let root_timestamp = tree
        .node(0)
        .map_or_else(std::time::Instant::now, reovim_kernel::api::UndoNode::timestamp);

    let nodes: Vec<SerializableUndoNode> = tree
        .node_indices()
        .filter_map(|idx| tree.node(idx))
        .map(|node| {
            let relative_time = node
                .timestamp()
                .duration_since(root_timestamp)
                .as_secs_f64();

            SerializableUndoNode {
                edits: node.edits().iter().map(SerializableEdit::from).collect(),
                cursor_before: node.cursor_before().into(),
                cursor_after: node.cursor_after().into(),
                relative_time_secs: relative_time,
                parent: node.parent(),
                children: node.children().to_vec(),
                seq_num: node.seq_num(),
                origin: node.origin().into(),
            }
        })
        .collect();

    // Collect active branches for all nodes
    let active_branches: Vec<usize> = tree
        .node_indices()
        .filter_map(|idx| tree.active_branch_at(idx))
        .collect();

    SerializableUndoTree {
        nodes,
        current: tree.current_index(),
        seq_counter: tree.seq_counter(),
        max_nodes: tree.max_nodes(),
        active_branches,
    }
}

/// Convert a serializable tree back to a kernel `UndoTree`.
///
/// Timestamps are reconstructed as relative offsets from "now".
#[must_use]
pub fn to_undo_tree(serializable: &SerializableUndoTree) -> UndoTree {
    let nodes_data: Vec<_> = serializable
        .nodes
        .iter()
        .map(|snode| {
            (
                snode.edits.iter().cloned().map(Edit::from).collect(),
                snode.cursor_before.into(),
                snode.cursor_after.into(),
                Duration::from_secs_f64(snode.relative_time_secs),
                snode.parent,
                snode.children.clone(),
                snode.seq_num,
                snode.origin.into(),
            )
        })
        .collect();

    UndoTree::from_serializable(
        nodes_data,
        serializable.current,
        serializable.seq_counter,
        serializable.max_nodes,
        serializable.active_branches.clone(),
    )
}

#[cfg(test)]
#[path = "undo_tests.rs"]
mod tests;
