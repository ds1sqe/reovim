//! Explorer extension state bridge.
//!
//! Serializes [`ExplorerState`] and tree data to JSON for gRPC
//! transmission to clients. Both TUI and Web extensions consume
//! the same JSON format.

use reovim_driver_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use crate::{
    KIND,
    state::{ExplorerInputMode, ExplorerState},
    tree::node::NodeType,
};

/// Bridge for explorer sidebar state.
///
/// Reads [`ExplorerState`] from the client's `ExtensionMap` and
/// serializes it to JSON matching what TUI and Web extensions expect.
pub struct ExplorerBridge;

impl ExtensionStateBridge for ExplorerBridge {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn scope(&self) -> ExtensionScope {
        ExtensionScope::Client
    }

    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value> {
        let state = extensions.get::<ExplorerState>()?;

        if !state.active {
            return Some(serde_json::json!({
                "active": false,
            }));
        }

        let input_mode_str = match state.input_mode {
            ExplorerInputMode::None => "none",
            ExplorerInputMode::CreateFile => "createFile",
            ExplorerInputMode::CreateDir => "createDir",
            ExplorerInputMode::Rename => "rename",
            ExplorerInputMode::ConfirmDelete => "confirmDelete",
        };

        // Delta detection: if tree_generation matches snapshot_generation,
        // the client already has the current nodes — emit metadata only.
        let tree_gen = state.tree_generation();
        let snap_gen = state.snapshot_generation();
        let is_delta = tree_gen == snap_gen;

        let mut json = serde_json::json!({
            "active": true,
            "rootName": state.root_path.file_name()
                .map_or_else(
                    || state.root_path.to_string_lossy().to_string(),
                    |n| n.to_string_lossy().to_string(),
                ),
            "cursorIndex": state.cursor_index,
            "scrollOffset": state.scroll_offset,
            "width": state.width,
            "inputMode": input_mode_str,
            "inputBuffer": state.input_buffer,
            "showHidden": state.show_hidden,
            "showGitignored": state.show_gitignored,
        });

        if !is_delta {
            // Full snapshot: include nodes and record the generation.
            let nodes_json = state.cached_nodes_json().unwrap_or_else(|| {
                let nodes: Vec<serde_json::Value> = state
                    .tree
                    .as_ref()
                    .map(|tree| {
                        tree.flatten_with_metadata(state.show_hidden, state.show_gitignored)
                            .into_iter()
                            .map(|flat| {
                                let (is_dir, is_expanded, size) = match &flat.node.node_type {
                                    NodeType::File { size } => (false, false, *size),
                                    NodeType::Directory { expanded, .. } => (true, *expanded, 0),
                                    NodeType::Symlink { .. } => (false, false, 0),
                                };
                                let is_symlink =
                                    matches!(flat.node.node_type, NodeType::Symlink { .. });
                                serde_json::json!({
                                    "name": flat.node.name,
                                    "depth": flat.node.depth,
                                    "isDir": is_dir,
                                    "isExpanded": is_expanded,
                                    "isHidden": flat.node.is_hidden,
                                    "isLast": flat.is_last_child,
                                    "verticalLines": flat.vertical_lines,
                                    "isSymlink": is_symlink,
                                    "size": size,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                state.set_cached_nodes_json(nodes.clone());
                nodes
            });
            json["nodes"] = serde_json::json!(nodes_json);
            state.set_snapshot_generation(tree_gen);
        }

        if let Some(ref cut) = state.cut_path {
            json["cutPath"] = serde_json::json!(cut.to_string_lossy());
        }

        if let Some(ref msg) = state.message {
            json["message"] = serde_json::json!(msg);
        }

        Some(json)
    }

    fn is_active(&self, extensions: &ExtensionMap) -> bool {
        extensions.get::<ExplorerState>().is_some_and(|s| s.active)
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
