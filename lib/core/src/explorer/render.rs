//! Explorer rendering

use {
    crate::highlight::{ColorMode, Style, Theme},
    std::fmt::Write,
};

use super::{
    node::{FileNode, format_size},
    state::{ExplorerInputMode, ExplorerState},
};

/// Render the explorer state to a vector of styled lines
#[must_use]
pub fn render_explorer(
    state: &ExplorerState,
    width: u16,
    height: u16,
    theme: &Theme,
    color_mode: ColorMode,
) -> Vec<String> {
    let nodes = state.visible_nodes();
    let mut lines = Vec::with_capacity(height as usize);

    // Reserve space for input prompt if in input mode
    let tree_height = if state.input_mode == ExplorerInputMode::None {
        height as usize
    } else {
        height.saturating_sub(1) as usize
    };

    // Calculate visible range based on scroll offset
    let start = state.scroll_offset;
    let end = (start + tree_height).min(nodes.len());

    for (i, node) in nodes.iter().enumerate().skip(start).take(end - start) {
        let is_cursor = i == state.cursor_index;
        let is_marked = state.is_selected(&node.path);
        let line =
            render_node(node, is_cursor, is_marked, state.show_sizes, width, theme, color_mode);
        lines.push(line);
    }

    // Pad with full-width empty lines to prevent editor content bleeding through
    while lines.len() < tree_height {
        lines.push(" ".repeat(width as usize));
    }

    // Add input prompt if in input mode
    if state.input_mode != ExplorerInputMode::None {
        let prompt = render_input_prompt(state, width, theme, color_mode);
        lines.push(prompt);
    }

    lines
}

/// Render the input prompt line
fn render_input_prompt(
    state: &ExplorerState,
    width: u16,
    theme: &Theme,
    color_mode: ColorMode,
) -> String {
    let mut result = String::new();

    // Get the prompt text
    let prompt = state.message.as_deref().unwrap_or("");
    let input = &state.input_buffer;

    // Combine prompt and input
    let content = format!("{prompt}{input}");

    // Truncate if needed
    let available = width as usize;
    let display = if content.len() > available {
        let start = content.len().saturating_sub(available);
        format!("…{}", &content[start + 1..])
    } else {
        content
    };

    // Style the prompt (use status line style for visibility)
    result.push_str(&theme.statusline.background.to_ansi_start(color_mode));
    let _ = write!(result, "{display:<width$}", width = width as usize);
    result.push_str(Style::ansi_reset());

    result
}

/// Width reserved for size column (including trailing space)
const SIZE_COLUMN_WIDTH: usize = 6;

/// Render a single node to a styled string
fn render_node(
    node: &FileNode,
    is_cursor: bool,
    is_marked: bool,
    show_sizes: bool,
    width: u16,
    theme: &Theme,
    color_mode: ColorMode,
) -> String {
    let mut result = String::new();

    // Apply cursor style if under cursor
    let base_style = if is_cursor {
        &theme.selection.visual
    } else {
        &Style::default()
    };

    // Build the line content
    let indent = "  ".repeat(node.depth);
    let icon = node.icon();
    let name = &node.name;

    // Selection marker
    let marker = if is_marked { "*" } else { " " };

    // Get size string if showing sizes
    let size_str = if show_sizes {
        node.size()
            .map_or_else(|| " ".repeat(SIZE_COLUMN_WIDTH), |s| format!("{:>5} ", format_size(s)))
    } else {
        String::new()
    };

    // Calculate available width for the name (account for marker)
    let prefix_len = marker.len() + indent.len() + icon.len() + size_str.len();
    let available_width = (width as usize).saturating_sub(prefix_len);

    // Truncate name if needed
    let display_name = if name.len() > available_width {
        format!("{}...", &name[..available_width.saturating_sub(3)])
    } else {
        name.clone()
    };

    // Build the full line (marker at start)
    let content = format!("{marker}{indent}{icon}{display_name}{size_str}");

    // Pad ALL lines to full width to prevent editor content bleeding through
    let padded = format!("{content:<width$}", width = width as usize);

    // Apply styling
    if is_cursor || is_marked || node.is_dir() {
        result.push_str(&base_style.to_ansi_start(color_mode));
        result.push_str(&padded);
        result.push_str(Style::ansi_reset());
    } else {
        result.push_str(&padded);
    }

    result
}

/// Render the explorer header (shows current directory)
#[must_use]
pub fn render_header(
    state: &ExplorerState,
    width: u16,
    theme: &Theme,
    color_mode: ColorMode,
) -> String {
    let root_path = state.tree.root_path();
    let path_str = root_path.file_name().map_or_else(
        || root_path.to_string_lossy().to_string(),
        |n| n.to_string_lossy().to_string(),
    );

    // Truncate if needed
    let available = (width as usize).saturating_sub(2);
    let display = if path_str.len() > available {
        format!("...{}", &path_str[path_str.len().saturating_sub(available - 3)..])
    } else {
        path_str
    };

    // Style the header
    let styled = format!(
        "{} {} {}",
        theme.statusline.background.to_ansi_start(color_mode),
        display,
        Style::ansi_reset()
    );

    // Pad to full width
    format!("{styled:<width$}", width = width as usize)
}

#[cfg(test)]
mod tests {
    use {super::*, std::fs::File, tempfile::tempdir};

    #[test]
    fn test_render_explorer() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("a.txt")).unwrap();
        File::create(dir.path().join("b.txt")).unwrap();

        let state = ExplorerState::new(dir.path().to_path_buf()).unwrap();
        let theme = Theme::default();
        let lines = render_explorer(&state, 30, 10, &theme, ColorMode::Ansi16);

        // Should have lines for root + 2 files + padding
        assert_eq!(lines.len(), 10);
    }

    #[test]
    fn test_all_lines_padded_to_full_width() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("a.txt")).unwrap();
        File::create(dir.path().join("short.txt")).unwrap();

        let state = ExplorerState::new(dir.path().to_path_buf()).unwrap();
        let theme = Theme::default();
        let width = 40u16;
        let lines = render_explorer(&state, width, 10, &theme, ColorMode::Ansi16);

        // All lines should be at least width characters (accounting for ANSI codes)
        // Content lines have ANSI escape codes, so check visible length
        for (i, line) in lines.iter().enumerate() {
            // Strip ANSI codes and check length
            let visible_len = strip_ansi_codes(line).chars().count();
            assert!(
                visible_len >= width as usize,
                "Line {i} has visible length {visible_len}, expected at least {width}"
            );
        }
    }

    #[test]
    fn test_empty_padding_lines_full_width() {
        let dir = tempdir().unwrap();
        // Create just one file so we have many padding lines
        File::create(dir.path().join("only.txt")).unwrap();

        let state = ExplorerState::new(dir.path().to_path_buf()).unwrap();
        let theme = Theme::default();
        let width = 30u16;
        let height = 20u16;
        let lines = render_explorer(&state, width, height, &theme, ColorMode::Ansi16);

        assert_eq!(lines.len(), height as usize);

        // Padding lines (after content) should be full width spaces
        // Root + 1 file = 2 content lines, rest are padding
        for line in lines.iter().skip(2) {
            let visible_len = strip_ansi_codes(line).chars().count();
            assert!(
                visible_len >= width as usize,
                "Padding line has length {visible_len}, expected {width}"
            );
        }
    }

    #[test]
    fn test_resize_maintains_padding() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("file1.txt")).unwrap();
        File::create(dir.path().join("file2.txt")).unwrap();

        let state = ExplorerState::new(dir.path().to_path_buf()).unwrap();
        let theme = Theme::default();

        // Render at original size
        let lines_original = render_explorer(&state, 40, 15, &theme, ColorMode::Ansi16);
        for line in &lines_original {
            let visible_len = strip_ansi_codes(line).chars().count();
            assert!(visible_len >= 40, "Original: line too short");
        }

        // Render at smaller size (simulating resize)
        let lines_smaller = render_explorer(&state, 25, 10, &theme, ColorMode::Ansi16);
        for line in &lines_smaller {
            let visible_len = strip_ansi_codes(line).chars().count();
            assert!(visible_len >= 25, "After resize: line too short");
        }

        // Render at larger size
        let lines_larger = render_explorer(&state, 50, 20, &theme, ColorMode::Ansi16);
        for line in &lines_larger {
            let visible_len = strip_ansi_codes(line).chars().count();
            assert!(visible_len >= 50, "After resize larger: line too short");
        }
    }

    /// Helper to strip ANSI escape codes from a string
    fn strip_ansi_codes(s: &str) -> String {
        let mut result = String::new();
        let mut in_escape = false;

        for c in s.chars() {
            if c == '\x1b' {
                in_escape = true;
            } else if in_escape {
                if c == 'm' {
                    in_escape = false;
                }
            } else {
                result.push(c);
            }
        }

        result
    }
}
