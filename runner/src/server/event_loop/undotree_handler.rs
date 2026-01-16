//! Undotree panel action handlers for the event loop.
//!
//! Handles undotree visualization: toggle, close, navigation, diff preview.

use reovim_kernel::api::v1::{
    BufferId, ModeId, ModuleId, UndoResult,
    events::{WindowClosed, WindowCreated, WindowFocused},
};

use super::EventLoop;
use crate::server::{AppState, app::{DiffPreviewLine, UndotreeRenderLine}};

impl<F: reovim_driver_input::InputFallbackHandler<AppState>> EventLoop<F> {
    /// Handle an undotree action from undotree commands.
    pub(super) fn handle_undotree_action(&mut self, action: reovim_driver_command::UndotreeAction) {
        use reovim_driver_command::UndotreeAction;

        tracing::debug!(?action, "Handling undotree action");

        match action {
            UndotreeAction::Toggle { buffer_id } => {
                self.toggle_undotree_panel(buffer_id);
            }
            UndotreeAction::Close => {
                self.close_undotree_panel();
            }
            UndotreeAction::MoveUp => {
                self.undotree_move_up();
            }
            UndotreeAction::MoveDown => {
                self.undotree_move_down();
            }
            UndotreeAction::GotoNode { node_index } => {
                self.undotree_goto_node(node_index);
            }
            UndotreeAction::GotoSelected => {
                let selected = self.app.undotree_state.selected_node();
                self.undotree_goto_node(selected);
            }
            UndotreeAction::PreviewDiff => {
                self.undotree_preview_diff();
            }
            UndotreeAction::ClearPreview => {
                self.app.undotree_state.clear_preview();
                self.refresh_undotree_panel();
            }
        }
    }

    /// Toggle the undotree panel for the given buffer.
    ///
    /// If the panel is closed, opens it with a vertical split on the right.
    /// If the panel is open, closes it and restores focus.
    pub(super) fn toggle_undotree_panel(&mut self, buffer_id: usize) {
        let buffer_id = BufferId::from_raw(buffer_id);

        if self.app.undotree_state.is_open() {
            // Panel is open - close it
            self.close_undotree_panel();
        } else {
            // Panel is closed - open it
            let previous_window = self.app.windows.active_window();

            // Create a vertical split on the right for the panel
            let Some(active) = previous_window else {
                self.set_error("No active window for undotree panel");
                return;
            };

            let Some(panel_id) = self.app.windows.split_vertical(active) else {
                self.set_error("Failed to create undotree panel");
                return;
            };

            // Open the panel state
            self.app
                .undotree_state
                .open(panel_id, buffer_id, previous_window);

            // Focus the panel window
            self.app.windows.set_active_window(panel_id);

            // Emit window created event
            self.app.kernel.event_bus.emit(WindowCreated {
                window_id: panel_id.raw() as u64,
            });

            // Push undotree mode onto the mode stack
            let undotree_mode = ModeId::new(ModuleId::new("undotree"), "undotree");
            self.app.mode_stack.push(undotree_mode);

            tracing::info!(
                panel_window = panel_id.raw(),
                source_buffer = buffer_id.as_usize(),
                "Opened undotree panel"
            );

            // Render initial panel content
            self.refresh_undotree_panel();
            self.last_error = None;
        }
    }

    /// Close the undotree panel.
    pub(super) fn close_undotree_panel(&mut self) {
        if !self.app.undotree_state.is_open() {
            // Already closed - no-op
            self.last_error = None;
            return;
        }

        let panel_id = self.app.undotree_state.panel_window_id();

        // Close the panel state and get the previous window for focus restoration
        let previous_window = self.app.undotree_state.close();

        // Close the panel window
        if let Some(panel_id) = panel_id
            && self.app.windows.close_window(panel_id)
        {
            self.app.kernel.event_bus.emit(WindowClosed {
                window_id: panel_id.raw() as u64,
            });
        }

        // Restore focus to the previous window
        if let Some(prev) = previous_window {
            let old_focus = self.app.windows.active_window();
            self.app.windows.set_active_window(prev);
            self.app.kernel.event_bus.emit(WindowFocused {
                from: old_focus.map(|w| w.raw() as u64),
                to: prev.raw() as u64,
            });
        }

        // Pop undotree mode from the mode stack
        if let Some(popped) = self.app.mode_stack.pop() {
            tracing::debug!(mode = %popped, "Popped undotree mode");
        }

        tracing::info!("Closed undotree panel");
        self.last_error = None;
    }

    /// Move selection up in the undotree (toward parent).
    pub(super) fn undotree_move_up(&mut self) {
        if !self.app.undotree_state.is_open() {
            return;
        }

        let Some(buffer_id) = self.app.undotree_state.source_buffer_id() else {
            return;
        };

        let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
            return;
        };

        let current_selection = self.app.undotree_state.selected_node();
        let Some(node) = tree.node(current_selection) else {
            return;
        };

        // Move to parent if available
        if let Some(parent_idx) = node.parent() {
            self.app.undotree_state.set_selected_node(parent_idx);

            // Clear preview when navigating away from the previewed node
            if self.app.undotree_state.is_preview_active()
                && self.app.undotree_state.preview_node() != Some(parent_idx)
            {
                self.app.undotree_state.clear_preview();
            }

            tracing::debug!(
                from = current_selection,
                to = parent_idx,
                "Undotree selection moved up"
            );
            // Refresh the panel display
            self.refresh_undotree_panel();
        }
        // At root - no-op

        self.last_error = None;
    }

    /// Move selection down in the undotree (toward child).
    pub(super) fn undotree_move_down(&mut self) {
        if !self.app.undotree_state.is_open() {
            return;
        }

        let Some(buffer_id) = self.app.undotree_state.source_buffer_id() else {
            return;
        };

        let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
            return;
        };

        let current_selection = self.app.undotree_state.selected_node();
        let Some(node) = tree.node(current_selection) else {
            return;
        };

        // Move to first child if available
        let children = node.children();
        if !children.is_empty() {
            // Use the first child (could use active branch in future)
            let child_idx = children[0];
            self.app.undotree_state.set_selected_node(child_idx);

            // Clear preview when navigating away from the previewed node
            if self.app.undotree_state.is_preview_active()
                && self.app.undotree_state.preview_node() != Some(child_idx)
            {
                self.app.undotree_state.clear_preview();
            }

            tracing::debug!(
                from = current_selection,
                to = child_idx,
                "Undotree selection moved down"
            );
            // Refresh the panel display
            self.refresh_undotree_panel();
        }
        // At leaf - no-op

        self.last_error = None;
    }

    /// Navigate to a specific node in the undotree.
    ///
    /// This applies the necessary undo/redo operations to move the buffer
    /// state to the target node. Uses the simple approach of undoing to
    /// root and redoing to target.
    pub(super) fn undotree_goto_node(&mut self, target_node: usize) {
        if !self.app.undotree_state.is_open() {
            return;
        }

        // Clear preview before navigation
        self.app.undotree_state.clear_preview();

        let Some(buffer_id) = self.app.undotree_state.source_buffer_id() else {
            return;
        };

        // Check if we're already at the target
        {
            let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
                return;
            };
            if tree.current_index() == target_node {
                tracing::debug!(node = target_node, "Already at target node");
                self.last_error = None;
                return;
            }
        }

        // Calculate path from root to target
        let path_to_target = {
            let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
                return;
            };
            Self::calculate_path_to_node(tree, target_node)
        };

        // Undo to root
        loop {
            // Check if at root
            let at_root = self
                .app
                .undo_registry
                .get_tree(buffer_id)
                .is_some_and(|tree| tree.current_index() == 0);

            if at_root {
                break;
            }

            if let Some(result) = self.app.undo_registry.undo(buffer_id) {
                self.apply_undo_result(buffer_id, result);
            } else {
                break;
            }
        }

        // Redo following the path to target
        for &(node_idx, branch_idx) in &path_to_target {
            // Skip root
            if node_idx == 0 {
                continue;
            }

            if let Some(result) = self.app.undo_registry.redo_branch(buffer_id, branch_idx) {
                self.apply_undo_result(buffer_id, result);

                // Verify we reached the expected node
                let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
                    break;
                };
                if tree.current_index() != node_idx {
                    tracing::warn!(
                        expected = node_idx,
                        actual = tree.current_index(),
                        "Unexpected node after redo_branch"
                    );
                    break;
                }
            } else {
                tracing::warn!(target = node_idx, branch = branch_idx, "Failed to redo_branch");
                break;
            }
        }

        tracing::info!(target = target_node, "Navigated to undotree node");

        // Refresh the panel display
        self.refresh_undotree_panel();
        self.last_error = None;
    }

    /// Refresh the undotree panel display.
    ///
    /// Re-renders the tree with current selection highlighting and
    /// updates the cached render lines in the undotree state.
    pub(super) fn refresh_undotree_panel(&mut self) {
        use reovim_module_undotree::UndotreeRenderer;

        if !self.app.undotree_state.is_open() {
            return;
        }

        let Some(buffer_id) = self.app.undotree_state.source_buffer_id() else {
            return;
        };

        let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
            return;
        };

        let selected_node = self.app.undotree_state.selected_node();

        // Render the tree
        let renderer = UndotreeRenderer::new();
        let render_lines = renderer.render(tree, selected_node);

        // Convert to our render line type
        let mut lines: Vec<UndotreeRenderLine> = render_lines
            .into_iter()
            .map(|rl| UndotreeRenderLine {
                text: rl.text,
                is_current: rl.is_current,
                is_selected: rl.is_selected,
            })
            .collect();

        // If preview is active, append separator and diff lines
        if self.app.undotree_state.is_preview_active() {
            lines.push(UndotreeRenderLine {
                text: String::from("─── Diff Preview ───"),
                is_current: false,
                is_selected: false,
            });

            for dl in self.app.undotree_state.preview_diff_lines() {
                lines.push(UndotreeRenderLine {
                    text: dl.text.clone(),
                    is_current: false,
                    // Highlight insertions and deletions
                    is_selected: dl.is_insert || dl.is_delete,
                });
            }
        }

        self.app.undotree_state.set_rendered_lines(lines);
        tracing::debug!("Refreshed undotree panel display");
    }

    /// Preview the diff of the currently selected undotree node.
    ///
    /// Extracts edits from the selected node and formats them as diff output,
    /// displaying them in the undotree panel below the tree visualization.
    pub(super) fn undotree_preview_diff(&mut self) {
        use reovim_module_undotree::{DiffLineType, format_edits_as_diff};

        if !self.app.undotree_state.is_open() {
            return;
        }

        let Some(buffer_id) = self.app.undotree_state.source_buffer_id() else {
            return;
        };

        let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
            return;
        };

        let selected_node = self.app.undotree_state.selected_node();
        let Some(node) = tree.node(selected_node) else {
            return;
        };

        // Format edits as diff
        let edits = node.edits();
        let diff_lines = format_edits_as_diff(edits);

        // Convert to preview lines
        let preview_lines: Vec<DiffPreviewLine> = diff_lines
            .into_iter()
            .map(|dl| DiffPreviewLine {
                text: dl.text,
                is_insert: dl.line_type == DiffLineType::Insert,
                is_delete: dl.line_type == DiffLineType::Delete,
                is_header: dl.line_type == DiffLineType::Header,
            })
            .collect();

        self.app
            .undotree_state
            .set_preview(selected_node, preview_lines);

        tracing::debug!(node = selected_node, edit_count = edits.len(), "Showing diff preview");

        // Refresh display to show preview
        self.refresh_undotree_panel();
        self.last_error = None;
    }

    /// Calculate the path from root to a target node.
    ///
    /// Returns a list of (`node_index`, `branch_index`) pairs representing
    /// the path from root to target. The `branch_index` indicates which
    /// child to follow at each step.
    pub(super) fn calculate_path_to_node(
        tree: &reovim_kernel::api::v1::UndoTree,
        target: usize,
    ) -> Vec<(usize, usize)> {
        // Build path from target back to root
        let mut path = Vec::new();
        let mut current = target;

        while let Some(node) = tree.node(current) {
            if let Some(parent) = node.parent() {
                // Find which branch index leads to current from parent
                if let Some(parent_node) = tree.node(parent) {
                    let branch_idx = parent_node
                        .children()
                        .iter()
                        .position(|&c| c == current)
                        .unwrap_or(0);
                    path.push((current, branch_idx));
                }
                current = parent;
            } else {
                // At root
                path.push((current, 0));
                break;
            }
        }

        // Reverse to get path from root to target
        path.reverse();
        path
    }

    /// Apply an undo/redo result to a buffer.
    pub(super) fn apply_undo_result(&self, buffer_id: BufferId, result: UndoResult) {
        use reovim_kernel::api::v1::Edit;

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            return;
        };

        let mut buffer = buffer_arc.write();
        for edit in result.edits {
            match edit {
                Edit::Insert { position, text } => {
                    buffer.insert_at(position, &text);
                }
                Edit::Delete { position, text } => {
                    buffer.delete_at(position, text.chars().count());
                }
            }
        }
        buffer.set_position(result.cursor);
    }
}
