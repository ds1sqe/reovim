//! Kernel ↔ Serializable type conversions for undo persistence.
//!
//! These conversions bridge kernel undo types (`UndoTree`, `Edit`, `Position`,
//! `EditOrigin`) with protocol serialization types (`Serializable*`). They live
//! in the undo module because this module is the sole consumer — the protocol
//! crate should not depend on the kernel.
//!
//! Uses explicit conversion functions rather than `From`/`Into` impls because
//! neither type is local to this crate (orphan rule).

use {
    reovim_domain_text::{Edit, EditOrigin, Position, UndoTree},
    reovim_protocol::v1::undo::{
        SerializableEdit, SerializableEditOrigin, SerializablePosition, SerializableUndoNode,
        SerializableUndoTree,
    },
    std::time::Duration,
};

// ----------------------------------------------------------------------------
// Position conversions
// ----------------------------------------------------------------------------

const fn position_to_serializable(pos: Position) -> SerializablePosition {
    SerializablePosition {
        line: pos.line,
        column: pos.column,
    }
}

const fn serializable_to_position(pos: SerializablePosition) -> Position {
    Position::new(pos.line, pos.column)
}

// ----------------------------------------------------------------------------
// Edit conversions
// ----------------------------------------------------------------------------

fn edit_to_serializable(edit: &Edit) -> SerializableEdit {
    match edit {
        Edit::Insert { position, text } => SerializableEdit::Insert {
            position: position_to_serializable(*position),
            text: text.clone(),
        },
        Edit::Delete { position, text } => SerializableEdit::Delete {
            position: position_to_serializable(*position),
            text: text.clone(),
        },
    }
}

fn serializable_to_edit(edit: SerializableEdit) -> Edit {
    match edit {
        SerializableEdit::Insert { position, text } => {
            Edit::insert(serializable_to_position(position), text)
        }
        SerializableEdit::Delete { position, text } => {
            Edit::delete(serializable_to_position(position), text)
        }
    }
}

// ----------------------------------------------------------------------------
// EditOrigin conversions
// ----------------------------------------------------------------------------

const fn origin_to_serializable(origin: EditOrigin) -> SerializableEditOrigin {
    match origin {
        EditOrigin::Client(id) => SerializableEditOrigin::Client(id),
        EditOrigin::System => SerializableEditOrigin::System,
    }
}

const fn serializable_to_origin(origin: SerializableEditOrigin) -> EditOrigin {
    match origin {
        SerializableEditOrigin::Client(id) => EditOrigin::Client(id),
        SerializableEditOrigin::System => EditOrigin::System,
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
        .map_or_else(std::time::Instant::now, reovim_domain_text::UndoNode::timestamp);

    let nodes: Vec<SerializableUndoNode> = tree
        .node_indices()
        .filter_map(|idx| tree.node(idx))
        .map(|node| {
            let relative_time = node
                .timestamp()
                .duration_since(root_timestamp)
                .as_secs_f64();

            SerializableUndoNode {
                edits: node.edits().iter().map(edit_to_serializable).collect(),
                cursor_before: position_to_serializable(node.cursor_before()),
                cursor_after: position_to_serializable(node.cursor_after()),
                relative_time_secs: relative_time,
                parent: node.parent(),
                children: node.children().to_vec(),
                seq_num: node.seq_num(),
                origin: origin_to_serializable(node.origin()),
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
                snode
                    .edits
                    .iter()
                    .cloned()
                    .map(serializable_to_edit)
                    .collect(),
                serializable_to_position(snode.cursor_before),
                serializable_to_position(snode.cursor_after),
                Duration::from_secs_f64(snode.relative_time_secs),
                snode.parent,
                snode.children.clone(),
                snode.seq_num,
                serializable_to_origin(snode.origin),
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
#[path = "conversion_tests.rs"]
mod tests;
