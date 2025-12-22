//! Explorer buffer provider for virtual buffer

use reovim_core::{
    content::{BufferContext, InputResult, PluginBufferProvider},
    event::KeyEvent,
    screen::Position,
};

use crate::state::ExplorerState;

/// Explorer buffer provider - generates virtual buffer content for file tree
pub struct ExplorerBufferProvider;

impl PluginBufferProvider for ExplorerBufferProvider {
    fn get_lines(&self, ctx: &BufferContext) -> Vec<String> {
        // Get explorer state from plugin state registry
        let lines = ctx
            .state
            .with::<ExplorerState, _, _>(|explorer| {
                if !explorer.visible {
                    return Vec::new();
                }

                // Get visible nodes from tree
                let nodes = explorer.visible_nodes();
                let mut lines = Vec::with_capacity(ctx.height as usize);

                // Calculate visible range based on scroll offset
                let start = explorer.scroll_offset;
                let end = (start + ctx.height as usize).min(nodes.len());

                // Generate plain text lines for each visible node
                for (i, node) in nodes.iter().enumerate().skip(start).take(end - start) {
                    let is_cursor = i == explorer.cursor_index;
                    let is_marked = explorer.selection.selected.contains(&node.path);

                    // Build line with indent and icon
                    let indent = "  ".repeat(node.depth);
                    let icon = if node.is_dir() {
                        if node.is_expanded() { "▾ " } else { "▸ " }
                    } else {
                        "  "
                    };

                    let marker = if is_marked {
                        "* "
                    } else if is_cursor {
                        "> "
                    } else {
                        "  "
                    };

                    let size_display = if explorer.show_sizes && !node.is_dir() {
                        if let Some(size) = node.size() {
                            format!(" {:>5} ", super::node::format_size(size))
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    let line = format!(
                        "{marker}{indent}{icon}{}{}",
                        node.name, size_display
                    );
                    lines.push(line);
                }

                // Pad with empty lines if needed
                while lines.len() < ctx.height as usize {
                    lines.push(String::new());
                }

                lines
            })
            .unwrap_or_default();

        lines
    }

    fn on_cursor_move(&mut self, position: Position, ctx: &mut BufferContext) {
        // Update explorer cursor index when cursor moves
        let _ = ctx.state
            .with_mut::<ExplorerState, _, _>(|explorer| {
                let new_index = (explorer.scroll_offset + position.y as usize)
                    .min(explorer.visible_nodes().len().saturating_sub(1));
                explorer.cursor_index = new_index;
            });
    }

    fn on_input(&mut self, _key: KeyEvent, _ctx: &mut BufferContext) -> InputResult {
        // Explorer handles input via commands, not direct key handling
        InputResult::Unhandled
    }

    fn is_editable(&self) -> bool {
        // Explorer is not directly editable
        false
    }
}
