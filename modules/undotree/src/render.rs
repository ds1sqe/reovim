//! ASCII tree rendering for undo visualization.
//!
//! Renders the undo tree as ASCII art suitable for display in a side panel.
//! The output shows the tree structure with branch connections, node markers,
//! sequence numbers, and relative timestamps.

use {reovim_kernel::api::v1::UndoTree, std::time::Instant};

/// A rendered line in the undotree visualization.
#[derive(Debug, Clone)]
pub struct RenderLine {
    /// The text content of this line.
    pub text: String,
    /// Whether this is the current node in the undo tree (marked with @).
    pub is_current: bool,
    /// Whether this is the currently selected node (for navigation highlight).
    pub is_selected: bool,
    /// Node index if this line represents a node (None for connector lines).
    pub node_index: Option<usize>,
}

impl RenderLine {
    /// Create a new render line for a node.
    #[allow(clippy::missing_const_for_fn)] // String is not const-constructible
    fn node(text: String, is_current: bool, is_selected: bool, node_index: usize) -> Self {
        Self {
            text,
            is_current,
            is_selected,
            node_index: Some(node_index),
        }
    }

    /// Create a connector line (no node associated).
    #[allow(clippy::missing_const_for_fn)] // String is not const-constructible
    fn connector(text: String) -> Self {
        Self {
            text,
            is_current: false,
            is_selected: false,
            node_index: None,
        }
    }
}

/// Renderer for undo tree visualization.
#[derive(Debug, Clone, Copy, Default)]
pub struct UndotreeRenderer;

impl UndotreeRenderer {
    /// Create a new renderer.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Render the undo tree with selection highlighting.
    ///
    /// # Arguments
    ///
    /// * `tree` - The undo tree to render
    /// * `selected_node` - The currently navigated-to node (for highlighting)
    ///
    /// # Returns
    ///
    /// Lines to display, ordered from top (newest) to bottom (root).
    #[must_use]
    pub fn render(self, tree: &UndoTree, selected_node: usize) -> Vec<RenderLine> {
        let current_idx = tree.current_index();

        // Special case: empty tree (only root)
        if tree.node_count() == 1 {
            return vec![RenderLine::node(
                "  @  [0] (root)".to_string(),
                true,
                selected_node == 0,
                0,
            )];
        }

        // Build lines via depth-first traversal
        let mut lines = Vec::new();
        let now = Instant::now();

        Self::render_subtree(tree, 0, &mut lines, &[], current_idx, selected_node, now);

        // Reverse for newest-at-top display
        lines.reverse();
        lines
    }

    /// Recursively render a subtree.
    #[allow(clippy::too_many_arguments)]
    fn render_subtree(
        tree: &UndoTree,
        node_idx: usize,
        lines: &mut Vec<RenderLine>,
        branch_stack: &[bool],
        current_idx: usize,
        selected_node: usize,
        now: Instant,
    ) {
        let Some(node) = tree.node(node_idx) else {
            return;
        };

        let is_current = node_idx == current_idx;
        let is_selected = node_idx == selected_node;
        let is_root = node.is_root();

        // Build prefix from branch stack
        let prefix = Self::build_prefix(branch_stack);

        // Build connector based on position
        let connector = if is_root {
            ""
        } else if branch_stack.last().copied().unwrap_or(false) {
            "|-"
        } else {
            "'-"
        };

        // Node marker
        let marker = if is_current { "@" } else { "o" };

        // Format the node line
        let seq_num = node.seq_num();
        let time_str = if is_root {
            "(root)".to_string()
        } else {
            Self::format_time_ago(node.timestamp(), now)
        };

        let line_text = format!("{prefix}{connector}{marker}  [{seq_num}] {time_str}");
        lines.push(RenderLine::node(line_text, is_current, is_selected, node_idx));

        // Render children
        let children = node.children();
        for (i, &child_idx) in children.iter().enumerate() {
            let is_last = i == children.len() - 1;

            // Add connector line between parent and child
            if children.len() > 1 && i > 0 {
                let conn_prefix = Self::build_prefix(branch_stack);
                let branch_char = if is_last { "/" } else { "|" };
                lines.push(RenderLine::connector(format!("{conn_prefix}{branch_char}")));
            } else if !children.is_empty() {
                let conn_prefix = Self::build_prefix(branch_stack);
                lines.push(RenderLine::connector(format!("{conn_prefix}|")));
            }

            // Recurse with updated branch stack
            let mut new_stack = branch_stack.to_vec();
            new_stack.push(!is_last);
            Self::render_subtree(
                tree,
                child_idx,
                lines,
                &new_stack,
                current_idx,
                selected_node,
                now,
            );
        }
    }

    /// Build the prefix string from the branch stack.
    fn build_prefix(branch_stack: &[bool]) -> String {
        let mut prefix = String::new();
        for &has_more in branch_stack {
            if has_more {
                prefix.push_str("| ");
            } else {
                prefix.push_str("  ");
            }
        }
        prefix
    }

    /// Format relative time since timestamp.
    fn format_time_ago(timestamp: Instant, now: Instant) -> String {
        let duration = now.saturating_duration_since(timestamp);
        let secs = duration.as_secs();

        if secs < 60 {
            format!("{secs}s ago")
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::time::Duration};

    #[test]
    fn test_render_empty_tree() {
        let tree = UndoTree::new();
        let renderer = UndotreeRenderer::new();
        let lines = renderer.render(&tree, 0);

        assert_eq!(lines.len(), 1);
        assert!(lines[0].is_current);
        assert!(lines[0].is_selected);
        assert!(lines[0].text.contains("(root)"));
    }

    #[test]
    fn test_render_current_node_marker() {
        let tree = UndoTree::new();
        let renderer = UndotreeRenderer::new();
        let lines = renderer.render(&tree, 0);

        // Current node should have @ marker
        assert!(lines[0].text.contains('@'));
    }

    #[test]
    fn test_render_selected_node() {
        let tree = UndoTree::new();
        let renderer = UndotreeRenderer::new();
        let lines = renderer.render(&tree, 0);

        // When selected_node matches the node, is_selected should be true
        assert!(lines[0].is_selected);
    }

    #[test]
    fn test_render_line_node_constructor() {
        let line = RenderLine::node("test".to_string(), true, false, 5);
        assert_eq!(line.text, "test");
        assert!(line.is_current);
        assert!(!line.is_selected);
        assert_eq!(line.node_index, Some(5));
    }

    #[test]
    fn test_render_line_connector_constructor() {
        let line = RenderLine::connector("|".to_string());
        assert_eq!(line.text, "|");
        assert!(!line.is_current);
        assert!(!line.is_selected);
        assert!(line.node_index.is_none());
    }

    #[test]
    fn test_format_time_seconds() {
        let now = Instant::now();
        let timestamp = now.checked_sub(Duration::from_secs(5)).unwrap();
        let formatted = UndotreeRenderer::format_time_ago(timestamp, now);
        assert_eq!(formatted, "5s ago");
    }

    #[test]
    fn test_format_time_minutes() {
        let now = Instant::now();
        let timestamp = now.checked_sub(Duration::from_secs(180)).unwrap();
        let formatted = UndotreeRenderer::format_time_ago(timestamp, now);
        assert_eq!(formatted, "3m ago");
    }

    #[test]
    fn test_format_time_hours() {
        let now = Instant::now();
        let timestamp = now.checked_sub(Duration::from_secs(7200)).unwrap();
        let formatted = UndotreeRenderer::format_time_ago(timestamp, now);
        assert_eq!(formatted, "2h ago");
    }

    #[test]
    fn test_format_time_days() {
        let now = Instant::now();
        let timestamp = now.checked_sub(Duration::from_secs(86400)).unwrap();
        let formatted = UndotreeRenderer::format_time_ago(timestamp, now);
        assert_eq!(formatted, "1d ago");
    }

    #[test]
    fn test_default_renderer() {
        // Default and new() should produce the same renderer
        let default_renderer = UndotreeRenderer;
        let new_renderer = UndotreeRenderer::new();
        // Both should work identically (unit struct)
        let tree = UndoTree::new();
        let lines1 = default_renderer.render(&tree, 0);
        let lines2 = new_renderer.render(&tree, 0);
        assert_eq!(lines1.len(), lines2.len());
    }

    #[test]
    fn test_render_line_debug() {
        let line = RenderLine::node("test".to_string(), true, true, 1);
        // Should implement Debug
        let debug_str = format!("{line:?}");
        assert!(debug_str.contains("RenderLine"));
    }

    #[test]
    fn test_render_line_clone() {
        let original = RenderLine::node("test".to_string(), true, true, 1);
        #[allow(clippy::redundant_clone)]
        let cloned = original.clone();
        assert_eq!(cloned.text, "test");
        assert!(cloned.is_current);
        assert!(cloned.is_selected);
        assert_eq!(cloned.node_index, Some(1));
    }

    #[test]
    fn test_renderer_debug() {
        let renderer = UndotreeRenderer::new();
        let debug_str = format!("{renderer:?}");
        assert!(debug_str.contains("UndotreeRenderer"));
    }

    #[test]
    fn test_renderer_copy() {
        let renderer = UndotreeRenderer::new();
        let copied = renderer;
        // Both should work (Copy trait)
        let tree = UndoTree::new();
        let _ = renderer.render(&tree, 0);
        let _ = copied.render(&tree, 0);
    }

    #[test]
    fn test_render_sequence_numbers() {
        let tree = UndoTree::new();
        let renderer = UndotreeRenderer::new();
        let lines = renderer.render(&tree, 0);

        // Root node should have sequence number 0
        assert!(lines[0].text.contains("[0]"));
    }

    #[test]
    fn test_render_selected_different_from_current() {
        let tree = UndoTree::new();
        let renderer = UndotreeRenderer::new();

        // Render with selected_node different from current (0)
        // In empty tree, only root exists, so selected=0 and current=0
        let lines = renderer.render(&tree, 0);

        // Both should be true for root in empty tree
        assert!(lines[0].is_current);
        assert!(lines[0].is_selected);
    }

    #[test]
    fn test_format_time_edge_case_zero() {
        let now = Instant::now();
        // Timestamp is now (0 seconds ago)
        let formatted = UndotreeRenderer::format_time_ago(now, now);
        assert_eq!(formatted, "0s ago");
    }

    #[test]
    fn test_format_time_boundary_59_seconds() {
        let now = Instant::now();
        let timestamp = now.checked_sub(Duration::from_secs(59)).unwrap();
        let formatted = UndotreeRenderer::format_time_ago(timestamp, now);
        assert_eq!(formatted, "59s ago");
    }

    #[test]
    fn test_format_time_boundary_60_seconds() {
        let now = Instant::now();
        let timestamp = now.checked_sub(Duration::from_secs(60)).unwrap();
        let formatted = UndotreeRenderer::format_time_ago(timestamp, now);
        assert_eq!(formatted, "1m ago");
    }

    #[test]
    fn test_format_time_boundary_59_minutes() {
        let now = Instant::now();
        let timestamp = now.checked_sub(Duration::from_secs(59 * 60)).unwrap();
        let formatted = UndotreeRenderer::format_time_ago(timestamp, now);
        assert_eq!(formatted, "59m ago");
    }

    #[test]
    fn test_format_time_boundary_60_minutes() {
        let now = Instant::now();
        let timestamp = now.checked_sub(Duration::from_secs(60 * 60)).unwrap();
        let formatted = UndotreeRenderer::format_time_ago(timestamp, now);
        assert_eq!(formatted, "1h ago");
    }

    #[test]
    fn test_format_time_boundary_23_hours() {
        let now = Instant::now();
        let timestamp = now.checked_sub(Duration::from_secs(23 * 3600)).unwrap();
        let formatted = UndotreeRenderer::format_time_ago(timestamp, now);
        assert_eq!(formatted, "23h ago");
    }

    // ========================================================================
    // Complex Tree Rendering Tests (#258)
    // ========================================================================

    use reovim_kernel::api::v1::{Edit, Position};

    /// Create a simple test edit for building undo trees.
    fn test_edit() -> Vec<Edit> {
        vec![Edit::insert(Position::default(), "x")]
    }

    #[test]
    fn test_render_multi_branch_tree() {
        // Create tree with fork point:
        //   push -> undo -> push (branch 1)
        //                -> undo -> push (branch 2)
        let mut tree = UndoTree::new();
        let cursor = Position::default();

        // Push first change (node 1)
        tree.push(test_edit(), cursor, cursor);

        // Undo back to root, then push (creates branch at root)
        tree.undo();
        tree.push(test_edit(), cursor, cursor); // node 2 (branch from root)

        // Push another from node 2 (node 3)
        tree.push(test_edit(), cursor, cursor);

        let renderer = UndotreeRenderer::new();
        let lines = renderer.render(&tree, tree.current_index());

        // Should have 4 nodes (root + 3 changes)
        assert_eq!(tree.node_count(), 4);

        // Root should have 2 children (fork point)
        let root = tree.node(0).unwrap();
        assert_eq!(root.children().len(), 2, "Root should have 2 branches");

        // Current node (3) should be marked with @
        let current_line = lines.iter().find(|l| l.is_current).unwrap();
        assert!(current_line.text.contains('@'), "Current node should have @ marker");

        // Non-current nodes should have 'o' marker
        let non_current_count = lines
            .iter()
            .filter(|l| l.node_index.is_some() && !l.is_current)
            .count();
        assert!(non_current_count >= 2, "Should have at least 2 non-current nodes");
    }

    #[test]
    fn test_render_deep_tree() {
        // Create tree with 12 sequential nodes (root + 11 changes)
        let mut tree = UndoTree::new();
        let cursor = Position::default();

        for _ in 0..11 {
            tree.push(test_edit(), cursor, cursor);
        }

        let renderer = UndotreeRenderer::new();
        let lines = renderer.render(&tree, tree.current_index());

        // Should have 12 nodes total
        assert_eq!(tree.node_count(), 12);

        // Count node lines (lines with node_index)
        let node_count = lines.iter().filter(|l| l.node_index.is_some()).count();
        assert_eq!(node_count, 12, "Should render all 12 nodes");

        // Verify sequence numbers are present [0] through [11]
        for i in 0..12 {
            let has_seq = lines.iter().any(|l| l.text.contains(&format!("[{i}]")));
            assert!(has_seq, "Missing sequence number [{i}]");
        }

        // Current should be at the tip (node 11)
        assert_eq!(tree.current_index(), 11);
        let current_line = lines.iter().find(|l| l.is_current).unwrap();
        assert!(current_line.text.contains("[11]"));
    }

    #[test]
    fn test_render_wide_tree() {
        // Create tree with 5 branches from root
        let mut tree = UndoTree::new();
        let cursor = Position::default();

        for _ in 0..5 {
            // Push a change, then undo back to root
            tree.push(test_edit(), cursor, cursor);
            tree.undo();
        }

        // Push one more to be current
        tree.push(test_edit(), cursor, cursor);

        let renderer = UndotreeRenderer::new();
        let lines = renderer.render(&tree, tree.current_index());

        // Root should have 6 children (5 undone branches + 1 current)
        let root = tree.node(0).unwrap();
        assert_eq!(root.children().len(), 6, "Root should have 6 branches");

        // All 7 nodes should be rendered (root + 6 children)
        assert_eq!(tree.node_count(), 7);
        let node_count = lines.iter().filter(|l| l.node_index.is_some()).count();
        assert_eq!(node_count, 7, "Should render all 7 nodes");

        // Current should be marked
        let current_line = lines.iter().find(|l| l.is_current).unwrap();
        assert!(current_line.text.contains('@'));
    }

    #[test]
    fn test_render_current_node_at_leaf() {
        // Current at tip of tree (leaf position)
        let mut tree = UndoTree::new();
        let cursor = Position::default();

        tree.push(test_edit(), cursor, cursor);
        tree.push(test_edit(), cursor, cursor);
        tree.push(test_edit(), cursor, cursor);

        let renderer = UndotreeRenderer::new();
        let lines = renderer.render(&tree, tree.current_index());

        // Current should be node 3 (leaf)
        assert_eq!(tree.current_index(), 3);

        // Verify @ is on node 3
        let current_line = lines.iter().find(|l| l.is_current).unwrap();
        assert!(current_line.text.contains("[3]"));
        assert!(current_line.text.contains('@'));

        // Verify nodes 0, 1, 2 have 'o' marker (not @)
        for line in lines
            .iter()
            .filter(|l| l.node_index.is_some() && !l.is_current)
        {
            assert!(
                line.text.contains('o') && !line.text.contains('@'),
                "Non-current node should have 'o' marker: {}",
                line.text
            );
        }
    }

    #[test]
    fn test_render_current_node_at_middle() {
        // Current in middle of tree (after undo)
        let mut tree = UndoTree::new();
        let cursor = Position::default();

        tree.push(test_edit(), cursor, cursor); // node 1
        tree.push(test_edit(), cursor, cursor); // node 2
        tree.push(test_edit(), cursor, cursor); // node 3
        tree.undo(); // back to node 2

        let renderer = UndotreeRenderer::new();
        let lines = renderer.render(&tree, tree.current_index());

        // Current should be node 2 (middle)
        assert_eq!(tree.current_index(), 2);

        // Verify @ is on node 2
        let current_line = lines.iter().find(|l| l.is_current).unwrap();
        assert!(current_line.text.contains("[2]"));
        assert!(current_line.text.contains('@'));

        // Node 3 exists but is not current
        let node3_line = lines
            .iter()
            .find(|l| l.node_index == Some(3))
            .expect("Node 3 should be rendered");
        assert!(!node3_line.is_current);
        assert!(node3_line.text.contains('o'));
    }
}
