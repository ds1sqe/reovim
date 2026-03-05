//! Sidebar layout calculation for the explorer extension.

/// Layout bounds for the explorer sidebar.
#[derive(Debug, Clone, Copy)]
pub struct SidebarBounds {
    /// X position (always 0 — left edge).
    pub x: u16,
    /// Y position (always 0 — top edge).
    pub y: u16,
    /// Sidebar width in columns.
    pub width: u16,
    /// Total terminal height.
    pub height: u16,
    /// Row for the root name header.
    pub header_y: u16,
    /// First row of tree content.
    pub tree_start_y: u16,
    /// Number of rows available for tree nodes.
    pub tree_height: u16,
    /// Row for input prompt (when in input mode).
    pub input_y: Option<u16>,
}

impl SidebarBounds {
    /// Calculate sidebar layout from terminal dimensions and explorer state.
    ///
    /// The sidebar fills the full terminal height on the left side.
    /// Layout:
    /// - Row 0: header (root directory name)
    /// - Rows 1..height-2: tree nodes (or height-1 if no input prompt)
    /// - Last row: input prompt (if in input mode)
    #[must_use]
    pub const fn calculate(width: u16, terminal_height: u16, has_input: bool) -> Self {
        let header_y = 0;
        let tree_start_y = 1;

        let (tree_height, input_y) = if has_input && terminal_height > 2 {
            let input_row = terminal_height - 1;
            (terminal_height.saturating_sub(2), Some(input_row))
        } else {
            (terminal_height.saturating_sub(1), None)
        };

        Self {
            x: 0,
            y: 0,
            width,
            height: terminal_height,
            header_y,
            tree_start_y,
            tree_height,
            input_y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_layout() {
        let bounds = SidebarBounds::calculate(30, 24, false);
        assert_eq!(bounds.x, 0);
        assert_eq!(bounds.y, 0);
        assert_eq!(bounds.width, 30);
        assert_eq!(bounds.height, 24);
        assert_eq!(bounds.header_y, 0);
        assert_eq!(bounds.tree_start_y, 1);
        assert_eq!(bounds.tree_height, 23);
        assert!(bounds.input_y.is_none());
    }

    #[test]
    fn layout_with_input() {
        let bounds = SidebarBounds::calculate(30, 24, true);
        assert_eq!(bounds.tree_height, 22);
        assert_eq!(bounds.input_y, Some(23));
    }

    #[test]
    fn layout_small_terminal() {
        let bounds = SidebarBounds::calculate(30, 3, false);
        assert_eq!(bounds.tree_height, 2);
        assert!(bounds.input_y.is_none());
    }

    #[test]
    fn layout_small_terminal_with_input() {
        let bounds = SidebarBounds::calculate(30, 3, true);
        assert_eq!(bounds.tree_height, 1);
        assert_eq!(bounds.input_y, Some(2));
    }

    #[test]
    fn layout_tiny_terminal_no_input() {
        let bounds = SidebarBounds::calculate(30, 1, false);
        assert_eq!(bounds.tree_height, 0);
        assert!(bounds.input_y.is_none());
    }

    #[test]
    fn layout_tiny_terminal_with_input() {
        let bounds = SidebarBounds::calculate(30, 2, false);
        assert_eq!(bounds.tree_height, 1);
        assert!(bounds.input_y.is_none());
    }

    #[test]
    fn layout_too_small_for_input() {
        // height <= 2 means no room for input row
        let bounds = SidebarBounds::calculate(30, 2, true);
        assert_eq!(bounds.tree_height, 1);
        assert!(bounds.input_y.is_none());
    }
}
