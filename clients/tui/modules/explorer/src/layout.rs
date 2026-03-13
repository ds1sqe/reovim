//! Sidebar layout calculation for the explorer module.

/// Layout bounds for the explorer sidebar within its allocated chrome region.
#[derive(Debug, Clone, Copy)]
pub struct SidebarBounds {
    /// X position (from chrome bounds).
    pub x: u16,
    /// Y position (from chrome bounds).
    pub y: u16,
    /// Sidebar width in columns.
    pub width: u16,
    /// Total height of the sidebar region.
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
    /// Calculate sidebar layout from chrome bounds and input mode.
    ///
    /// Layout within the allocated region:
    /// - Row 0: header (root directory name)
    /// - Rows 1..height-2: tree nodes (or height-1 if no input prompt)
    /// - Last row: input prompt (if in input mode)
    #[must_use]
    pub const fn calculate(x: u16, y: u16, width: u16, height: u16, has_input: bool) -> Self {
        let header_y = y;
        let tree_start_y = y + 1;

        let (tree_height, input_y) = if has_input && height > 2 {
            let input_row = y + height - 1;
            (height.saturating_sub(2), Some(input_row))
        } else {
            (height.saturating_sub(1), None)
        };

        Self {
            x,
            y,
            width,
            height,
            header_y,
            tree_start_y,
            tree_height,
            input_y,
        }
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
