//! Explorer rendering

use crate::highlight::{ColorMode, Style, Theme};
use std::fmt::Write;

use super::node::FileNode;
use super::state::{ExplorerInputMode, ExplorerState};

/// Render the explorer state to a vector of styled lines
#[must_use]
pub fn render_explorer(
    state: &ExplorerState,
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
        let is_selected = i == state.cursor_index;
        let line = render_node(node, is_selected, state.width, theme, color_mode);
        lines.push(line);
    }

    // Pad with empty lines if needed (but leave room for input prompt)
    while lines.len() < tree_height {
        lines.push(String::new());
    }

    // Add input prompt if in input mode
    if state.input_mode != ExplorerInputMode::None {
        let prompt = render_input_prompt(state, state.width, theme, color_mode);
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

/// Render a single node to a styled string
fn render_node(
    node: &FileNode,
    is_selected: bool,
    width: u16,
    theme: &Theme,
    color_mode: ColorMode,
) -> String {
    let mut result = String::new();

    // Apply selection style if selected
    let base_style = if is_selected {
        &theme.selection.visual
    } else {
        &Style::default()
    };

    // Build the line content
    let indent = "  ".repeat(node.depth);
    let icon = node.icon();
    let name = &node.name;

    // Calculate available width for the name
    let prefix_len = indent.len() + icon.len();
    let available_width = (width as usize).saturating_sub(prefix_len);

    // Truncate name if needed
    let display_name = if name.len() > available_width {
        format!("{}...", &name[..available_width.saturating_sub(3)])
    } else {
        name.clone()
    };

    // Build the full line
    let content = format!("{indent}{icon}{display_name}");

    // Pad to full width for selection highlight
    let padded = if is_selected {
        format!("{content:<width$}", width = width as usize)
    } else {
        content
    };

    // Apply styling
    if is_selected || node.is_dir() {
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
pub fn render_header(state: &ExplorerState, width: u16, theme: &Theme, color_mode: ColorMode) -> String {
    let root_path = state.tree.root_path();
    let path_str = root_path
        .file_name()
        .map_or_else(|| root_path.to_string_lossy().to_string(), |n| n.to_string_lossy().to_string());

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
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_render_explorer() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("a.txt")).unwrap();
        File::create(dir.path().join("b.txt")).unwrap();

        let state = ExplorerState::new(dir.path().to_path_buf()).unwrap();
        let theme = Theme::default();
        let lines = render_explorer(&state, 10, &theme, ColorMode::Ansi16);

        // Should have lines for root + 2 files + padding
        assert_eq!(lines.len(), 10);
    }
}
