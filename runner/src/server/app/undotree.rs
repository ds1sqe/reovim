//! Undotree panel state for visualization.
//!
//! Tracks the undotree panel UI: visibility, navigation, diff preview.

use {
    reovim_driver_display::WindowId,
    reovim_kernel::api::v1::BufferId,
};

// ============================================================================
// Undotree Panel State
// ============================================================================

/// A rendered line for the undotree panel display.
#[derive(Debug, Clone)]
pub struct UndotreeRenderLine {
    /// The text content of this line.
    pub text: String,
    /// Whether this is the current node in the undo tree.
    pub is_current: bool,
    /// Whether this is the currently selected node.
    pub is_selected: bool,
}

/// A line in the diff preview display.
#[derive(Debug, Clone)]
pub struct DiffPreviewLine {
    /// The text content.
    pub text: String,
    /// Whether this is an insertion line.
    pub is_insert: bool,
    /// Whether this is a deletion line.
    pub is_delete: bool,
    /// Whether this is a header line.
    pub is_header: bool,
}

/// State for the undotree panel visualization.
///
/// Tracks the panel's visibility, associated window, source buffer,
/// and navigation state. This is a runtime concern (window manager level),
/// not kernel - the kernel provides the `UndoTree` data structure, while
/// the runner manages the panel UI.
#[derive(Debug, Clone, Default)]
pub struct UndotreeState {
    /// Whether the undotree panel is currently visible.
    panel_open: bool,
    /// Window ID of the panel when open.
    panel_window_id: Option<WindowId>,
    /// Buffer whose undo tree is being displayed.
    source_buffer_id: Option<BufferId>,
    /// Currently selected node index in the tree (for navigation).
    selected_node: usize,
    /// Scroll offset for rendering long trees.
    scroll_offset: usize,
    /// The window that was focused before the panel opened.
    ///
    /// Used to restore focus when the panel is closed.
    previous_window_id: Option<WindowId>,
    /// Rendered lines for display (cached).
    rendered_lines: Vec<UndotreeRenderLine>,
    /// Whether diff preview is currently active.
    preview_active: bool,
    /// Node index being previewed (may differ from selected).
    preview_node: Option<usize>,
    /// Cached diff lines for the previewed node.
    preview_diff_lines: Vec<DiffPreviewLine>,
}

impl UndotreeState {
    /// Create a new undotree state (panel closed by default).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            panel_open: false,
            panel_window_id: None,
            source_buffer_id: None,
            selected_node: 0,
            scroll_offset: 0,
            previous_window_id: None,
            rendered_lines: Vec::new(),
            preview_active: false,
            preview_node: None,
            preview_diff_lines: Vec::new(),
        }
    }

    /// Check if the panel is currently open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.panel_open
    }

    /// Get the panel's window ID if open.
    #[must_use]
    pub const fn panel_window_id(&self) -> Option<WindowId> {
        self.panel_window_id
    }

    /// Get the source buffer ID being displayed.
    #[must_use]
    pub const fn source_buffer_id(&self) -> Option<BufferId> {
        self.source_buffer_id
    }

    /// Get the currently selected node index.
    #[must_use]
    pub const fn selected_node(&self) -> usize {
        self.selected_node
    }

    /// Set the selected node index.
    pub const fn set_selected_node(&mut self, node: usize) {
        self.selected_node = node;
    }

    /// Get the scroll offset.
    #[must_use]
    pub const fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Set the scroll offset.
    pub const fn set_scroll_offset(&mut self, offset: usize) {
        self.scroll_offset = offset;
    }

    /// Get the window that was focused before the panel opened.
    #[must_use]
    pub const fn previous_window_id(&self) -> Option<WindowId> {
        self.previous_window_id
    }

    /// Open the panel with the given window and source buffer.
    ///
    /// # Arguments
    ///
    /// * `window_id` - The window ID assigned to the panel
    /// * `buffer_id` - The buffer whose undo tree to display
    /// * `previous_window` - The window that was focused before opening
    pub const fn open(
        &mut self,
        window_id: WindowId,
        buffer_id: BufferId,
        previous_window: Option<WindowId>,
    ) {
        self.panel_open = true;
        self.panel_window_id = Some(window_id);
        self.source_buffer_id = Some(buffer_id);
        self.selected_node = 0; // Reset to root/current on open
        self.scroll_offset = 0;
        self.previous_window_id = previous_window;
    }

    /// Close the panel, returning the previous window ID for focus restoration.
    pub fn close(&mut self) -> Option<WindowId> {
        self.panel_open = false;
        self.panel_window_id = None;
        // Keep source_buffer_id for reference, but clear selection state
        self.selected_node = 0;
        self.scroll_offset = 0;
        self.rendered_lines.clear();
        // Clear preview state
        self.preview_active = false;
        self.preview_node = None;
        self.preview_diff_lines.clear();
        self.previous_window_id.take()
    }

    /// Update the rendered lines for display.
    pub fn set_rendered_lines(&mut self, lines: Vec<UndotreeRenderLine>) {
        self.rendered_lines = lines;
    }

    /// Get the rendered lines for display.
    #[must_use]
    pub fn rendered_lines(&self) -> &[UndotreeRenderLine] {
        &self.rendered_lines
    }

    /// Check if preview is currently active.
    #[must_use]
    pub const fn is_preview_active(&self) -> bool {
        self.preview_active
    }

    /// Get the node being previewed.
    #[must_use]
    pub const fn preview_node(&self) -> Option<usize> {
        self.preview_node
    }

    /// Set preview state for a node.
    pub fn set_preview(&mut self, node: usize, diff_lines: Vec<DiffPreviewLine>) {
        self.preview_active = true;
        self.preview_node = Some(node);
        self.preview_diff_lines = diff_lines;
    }

    /// Clear preview state.
    pub fn clear_preview(&mut self) {
        self.preview_active = false;
        self.preview_node = None;
        self.preview_diff_lines.clear();
    }

    /// Get the preview diff lines.
    #[must_use]
    pub fn preview_diff_lines(&self) -> &[DiffPreviewLine] {
        &self.preview_diff_lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undotree_state_default_closed() {
        let state = UndotreeState::new();

        assert!(!state.is_open());
        assert!(state.panel_window_id().is_none());
        assert!(state.source_buffer_id().is_none());
        assert_eq!(state.selected_node(), 0);
        assert_eq!(state.scroll_offset(), 0);
        assert!(state.previous_window_id().is_none());
    }

    #[test]
    fn test_undotree_state_open_close() {
        let mut state = UndotreeState::new();

        let window_id = WindowId::new(42);
        let buffer_id = BufferId::new();
        let prev_window = WindowId::new(1);

        // Open the panel
        state.open(window_id, buffer_id, Some(prev_window));

        assert!(state.is_open());
        assert_eq!(state.panel_window_id(), Some(window_id));
        assert_eq!(state.source_buffer_id(), Some(buffer_id));
        assert_eq!(state.selected_node(), 0); // Reset on open
        assert_eq!(state.previous_window_id(), Some(prev_window));

        // Close the panel
        let restored = state.close();

        assert!(!state.is_open());
        assert!(state.panel_window_id().is_none());
        // source_buffer_id is kept for reference
        assert!(state.source_buffer_id().is_some());
        assert_eq!(restored, Some(prev_window));
        assert!(state.previous_window_id().is_none()); // Taken during close
    }

    #[test]
    fn test_undotree_state_navigation() {
        let mut state = UndotreeState::new();

        // Set selection
        state.set_selected_node(5);
        assert_eq!(state.selected_node(), 5);

        state.set_selected_node(10);
        assert_eq!(state.selected_node(), 10);

        // Set scroll offset
        state.set_scroll_offset(3);
        assert_eq!(state.scroll_offset(), 3);
    }

    #[test]
    fn test_undotree_state_open_without_previous_window() {
        let mut state = UndotreeState::new();

        let window_id = WindowId::new(42);
        let buffer_id = BufferId::new();

        // Open without previous window (first window scenario)
        state.open(window_id, buffer_id, None);

        assert!(state.is_open());
        assert!(state.previous_window_id().is_none());

        // Close should return None
        let restored = state.close();
        assert!(restored.is_none());
    }

    #[test]
    fn test_undotree_state_preview_initially_inactive() {
        let state = UndotreeState::new();

        assert!(!state.is_preview_active());
        assert!(state.preview_node().is_none());
        assert!(state.preview_diff_lines().is_empty());
    }

    #[test]
    fn test_undotree_state_set_preview_activates() {
        let mut state = UndotreeState::new();

        let diff_lines = vec![
            DiffPreviewLine {
                text: "+hello".to_string(),
                is_insert: true,
                is_delete: false,
                is_header: false,
            },
            DiffPreviewLine {
                text: "-world".to_string(),
                is_insert: false,
                is_delete: true,
                is_header: false,
            },
        ];

        state.set_preview(5, diff_lines);

        assert!(state.is_preview_active());
        assert_eq!(state.preview_node(), Some(5));
        assert_eq!(state.preview_diff_lines().len(), 2);
    }

    #[test]
    fn test_undotree_state_clear_preview_deactivates() {
        let mut state = UndotreeState::new();

        let diff_lines = vec![DiffPreviewLine {
            text: "+test".to_string(),
            is_insert: true,
            is_delete: false,
            is_header: false,
        }];

        state.set_preview(3, diff_lines);
        assert!(state.is_preview_active());

        state.clear_preview();

        assert!(!state.is_preview_active());
        assert!(state.preview_node().is_none());
        assert!(state.preview_diff_lines().is_empty());
    }

    #[test]
    fn test_undotree_state_close_clears_preview() {
        let mut state = UndotreeState::new();

        let window_id = WindowId::new(42);
        let buffer_id = BufferId::new();

        state.open(window_id, buffer_id, None);

        let diff_lines = vec![DiffPreviewLine {
            text: "@@header@@".to_string(),
            is_insert: false,
            is_delete: false,
            is_header: true,
        }];
        state.set_preview(2, diff_lines);

        assert!(state.is_preview_active());

        state.close();

        assert!(!state.is_preview_active());
        assert!(state.preview_node().is_none());
        assert!(state.preview_diff_lines().is_empty());
    }

    #[test]
    fn test_undotree_state_preview_node_tracking() {
        let mut state = UndotreeState::new();

        // Preview different nodes
        state.set_preview(1, vec![]);
        assert_eq!(state.preview_node(), Some(1));

        state.set_preview(7, vec![]);
        assert_eq!(state.preview_node(), Some(7));

        // Selection is independent of preview
        state.set_selected_node(3);
        assert_eq!(state.selected_node(), 3);
        assert_eq!(state.preview_node(), Some(7));
    }
}
