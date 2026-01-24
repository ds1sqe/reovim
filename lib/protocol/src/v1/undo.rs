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
    reovim_kernel::api::v1::{Edit, Position, UndoTree},
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
mod tests {
    use super::*;

    fn create_test_tree() -> SerializableUndoTree {
        SerializableUndoTree {
            nodes: vec![
                SerializableUndoNode {
                    edits: vec![],
                    cursor_before: SerializablePosition::default(),
                    cursor_after: SerializablePosition::default(),
                    relative_time_secs: 0.0,
                    parent: None,
                    children: vec![1],
                    seq_num: 0,
                },
                SerializableUndoNode {
                    edits: vec![SerializableEdit::Insert {
                        position: SerializablePosition::new(0, 0),
                        text: "hello".to_string(),
                    }],
                    cursor_before: SerializablePosition::new(0, 0),
                    cursor_after: SerializablePosition::new(0, 5),
                    relative_time_secs: 1.5,
                    parent: Some(0),
                    children: vec![],
                    seq_num: 1,
                },
            ],
            current: 1,
            seq_counter: 1,
            max_nodes: 10000,
            active_branches: vec![0, 0],
        }
    }

    #[test]
    fn test_undo_file_format_roundtrip() {
        let tree = create_test_tree();
        let format = UndoFileFormat::new("/test/file.rs".to_string(), tree);

        let bytes = format.to_bytes().expect("serialization should succeed");
        let restored = UndoFileFormat::from_bytes(&bytes).expect("deserialization should succeed");

        assert_eq!(restored.version, UNDO_FILE_VERSION);
        assert_eq!(restored.original_path, "/test/file.rs");
        assert_eq!(restored.tree.current, 1);
        assert_eq!(restored.tree.nodes.len(), 2);
    }

    #[test]
    fn test_magic_bytes_validation() {
        // Too short
        assert!(matches!(UndoFileFormat::from_bytes(&[0, 1, 2]), Err(UndoFileError::TooShort)));

        // Wrong magic
        assert!(matches!(
            UndoFileFormat::from_bytes(b"XXXX...."),
            Err(UndoFileError::InvalidMagic)
        ));
    }

    #[test]
    fn test_version_field_preserved() {
        let tree = create_test_tree();
        let format = UndoFileFormat::new("/test.rs".to_string(), tree);

        let bytes = format.to_bytes().unwrap();
        let restored = UndoFileFormat::from_bytes(&bytes).unwrap();

        assert_eq!(restored.version, UNDO_FILE_VERSION);
    }

    #[test]
    fn test_position_serialization() {
        let pos = SerializablePosition::new(10, 20);
        let bytes = rmp_serde::to_vec(&pos).unwrap();
        let restored: SerializablePosition = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(pos, restored);
    }

    #[test]
    fn test_edit_serialization() {
        let insert = SerializableEdit::Insert {
            position: SerializablePosition::new(5, 10),
            text: "test".to_string(),
        };
        let bytes = rmp_serde::to_vec(&insert).unwrap();
        let restored: SerializableEdit = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(insert, restored);

        let delete = SerializableEdit::Delete {
            position: SerializablePosition::new(1, 0),
            text: "removed".to_string(),
        };
        let bytes = rmp_serde::to_vec(&delete).unwrap();
        let restored: SerializableEdit = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(delete, restored);
    }

    #[test]
    fn test_empty_tree_roundtrip() {
        let tree = SerializableUndoTree {
            nodes: vec![SerializableUndoNode {
                edits: vec![],
                cursor_before: SerializablePosition::default(),
                cursor_after: SerializablePosition::default(),
                relative_time_secs: 0.0,
                parent: None,
                children: vec![],
                seq_num: 0,
            }],
            current: 0,
            seq_counter: 0,
            max_nodes: 10000,
            active_branches: vec![0],
        };
        let format = UndoFileFormat::new("/empty.rs".to_string(), tree);

        let bytes = format.to_bytes().unwrap();
        let restored = UndoFileFormat::from_bytes(&bytes).unwrap();

        assert_eq!(restored.tree.nodes.len(), 1);
        assert_eq!(restored.tree.current, 0);
    }

    #[test]
    fn test_multi_branch_tree_roundtrip() {
        // Tree structure:
        //       0 (root)
        //      / \
        //     1   2
        //    /
        //   3
        let tree = SerializableUndoTree {
            nodes: vec![
                SerializableUndoNode {
                    edits: vec![],
                    cursor_before: SerializablePosition::default(),
                    cursor_after: SerializablePosition::default(),
                    relative_time_secs: 0.0,
                    parent: None,
                    children: vec![1, 2],
                    seq_num: 0,
                },
                SerializableUndoNode {
                    edits: vec![SerializableEdit::Insert {
                        position: SerializablePosition::new(0, 0),
                        text: "a".to_string(),
                    }],
                    cursor_before: SerializablePosition::new(0, 0),
                    cursor_after: SerializablePosition::new(0, 1),
                    relative_time_secs: 1.0,
                    parent: Some(0),
                    children: vec![3],
                    seq_num: 1,
                },
                SerializableUndoNode {
                    edits: vec![SerializableEdit::Insert {
                        position: SerializablePosition::new(0, 0),
                        text: "b".to_string(),
                    }],
                    cursor_before: SerializablePosition::new(0, 0),
                    cursor_after: SerializablePosition::new(0, 1),
                    relative_time_secs: 2.0,
                    parent: Some(0),
                    children: vec![],
                    seq_num: 2,
                },
                SerializableUndoNode {
                    edits: vec![SerializableEdit::Insert {
                        position: SerializablePosition::new(0, 1),
                        text: "c".to_string(),
                    }],
                    cursor_before: SerializablePosition::new(0, 1),
                    cursor_after: SerializablePosition::new(0, 2),
                    relative_time_secs: 3.0,
                    parent: Some(1),
                    children: vec![],
                    seq_num: 3,
                },
            ],
            current: 3,
            seq_counter: 3,
            max_nodes: 10000,
            active_branches: vec![0, 0, 0, 0],
        };
        let format = UndoFileFormat::new("/branched.rs".to_string(), tree);

        let bytes = format.to_bytes().unwrap();
        let restored = UndoFileFormat::from_bytes(&bytes).unwrap();

        assert_eq!(restored.tree.nodes.len(), 4);
        assert_eq!(restored.tree.current, 3);
        assert_eq!(restored.tree.nodes[0].children, vec![1, 2]);
        assert_eq!(restored.tree.nodes[1].children, vec![3]);
    }

    // ========================================================================
    // Kernel conversion tests
    // ========================================================================

    #[test]
    fn test_position_kernel_conversion() {
        let kernel_pos = Position::new(10, 20);
        let serializable: SerializablePosition = kernel_pos.into();
        assert_eq!(serializable.line, 10);
        assert_eq!(serializable.column, 20);

        let back: Position = serializable.into();
        assert_eq!(back, kernel_pos);
    }

    #[test]
    fn test_edit_kernel_conversion() {
        let insert = Edit::insert(Position::new(5, 10), "test");
        let serializable = SerializableEdit::from(&insert);
        assert!(matches!(
            &serializable,
            SerializableEdit::Insert { position, text }
            if position.line == 5 && position.column == 10 && text == "test"
        ));

        let back: Edit = serializable.into();
        assert_eq!(back.position(), Position::new(5, 10));
        assert_eq!(back.text(), "test");
        assert!(back.is_insert());

        let delete = Edit::delete(Position::new(1, 0), "removed");
        let serializable = SerializableEdit::from(&delete);
        let back: Edit = serializable.into();
        assert!(back.is_delete());
        assert_eq!(back.text(), "removed");
    }

    #[test]
    fn test_undo_tree_kernel_roundtrip() {
        let mut tree = UndoTree::new();

        // Make some edits
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "hello")],
            Position::new(0, 0),
            Position::new(0, 5),
        );
        tree.push(
            vec![Edit::insert(Position::new(0, 5), " world")],
            Position::new(0, 5),
            Position::new(0, 11),
        );

        // Convert to serializable
        let serializable = from_undo_tree(&tree);
        assert_eq!(serializable.nodes.len(), 3); // root + 2 edits
        assert_eq!(serializable.current, 2);

        // Convert back
        let restored = to_undo_tree(&serializable);
        assert_eq!(restored.node_count(), 3);
        assert_eq!(restored.current_index(), 2);

        // Verify undo still works
        let mut restored = restored;
        let undo_result = restored.undo();
        assert!(undo_result.is_some());
        assert_eq!(restored.current_index(), 1);
    }

    #[test]
    fn test_timestamp_ordering_preserved() {
        // Create a tree with nodes that have increasing relative times
        let serializable = SerializableUndoTree {
            nodes: vec![
                SerializableUndoNode {
                    edits: vec![],
                    cursor_before: SerializablePosition::default(),
                    cursor_after: SerializablePosition::default(),
                    relative_time_secs: 0.0,
                    parent: None,
                    children: vec![1, 2, 3, 4],
                    seq_num: 0,
                },
                SerializableUndoNode {
                    edits: vec![],
                    cursor_before: SerializablePosition::default(),
                    cursor_after: SerializablePosition::default(),
                    relative_time_secs: 1.0,
                    parent: Some(0),
                    children: vec![],
                    seq_num: 1,
                },
                SerializableUndoNode {
                    edits: vec![],
                    cursor_before: SerializablePosition::default(),
                    cursor_after: SerializablePosition::default(),
                    relative_time_secs: 2.5,
                    parent: Some(0),
                    children: vec![],
                    seq_num: 2,
                },
                SerializableUndoNode {
                    edits: vec![],
                    cursor_before: SerializablePosition::default(),
                    cursor_after: SerializablePosition::default(),
                    relative_time_secs: 3.0,
                    parent: Some(0),
                    children: vec![],
                    seq_num: 3,
                },
                SerializableUndoNode {
                    edits: vec![],
                    cursor_before: SerializablePosition::default(),
                    cursor_after: SerializablePosition::default(),
                    relative_time_secs: 5.0,
                    parent: Some(0),
                    children: vec![],
                    seq_num: 4,
                },
            ],
            current: 4,
            seq_counter: 4,
            max_nodes: 10000,
            active_branches: vec![0, 0, 0, 0, 0],
        };

        // Convert to kernel tree
        let tree = to_undo_tree(&serializable);

        // Verify timestamp ordering is preserved
        // Node with relative_time_secs = 1.0 should have timestamp before node with 2.5, etc.
        for i in 1..tree.node_count() {
            for j in (i + 1)..tree.node_count() {
                let node_i = tree.node(i).unwrap();
                let node_j = tree.node(j).unwrap();

                // Get original relative times
                let time_i = serializable.nodes[i].relative_time_secs;
                let time_j = serializable.nodes[j].relative_time_secs;

                // If node i was earlier than node j in original, it should still be
                if time_i < time_j {
                    assert!(
                        node_i.timestamp() <= node_j.timestamp(),
                        "Timestamp ordering violated: node {i} (rel: {time_i}) should be <= node {j} (rel: {time_j})"
                    );
                } else if time_i > time_j {
                    assert!(
                        node_i.timestamp() >= node_j.timestamp(),
                        "Timestamp ordering violated: node {i} (rel: {time_i}) should be >= node {j} (rel: {time_j})"
                    );
                }
            }
        }
    }

    #[test]
    fn test_full_roundtrip_through_file_format() {
        // Create kernel tree
        let mut tree = UndoTree::new();
        tree.push(
            vec![Edit::insert(Position::new(0, 0), "a")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "b")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        // Undo to create a branch point
        tree.undo();

        // Add different edit (creates branch)
        tree.push(
            vec![Edit::insert(Position::new(0, 1), "c")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        // Full roundtrip: kernel -> serializable -> bytes -> serializable -> kernel
        let serializable = from_undo_tree(&tree);
        let format = UndoFileFormat::new("/test.rs".to_string(), serializable);
        let bytes = format.to_bytes().unwrap();
        let restored_format = UndoFileFormat::from_bytes(&bytes).unwrap();
        let restored_tree = to_undo_tree(&restored_format.tree);

        // Verify structure preserved
        assert_eq!(restored_tree.node_count(), tree.node_count());
        assert_eq!(restored_tree.current_index(), tree.current_index());

        // Verify branches preserved
        let original_root = tree.node(0).unwrap();
        let restored_root = restored_tree.node(0).unwrap();
        assert_eq!(original_root.children().len(), restored_root.children().len());
    }
}
