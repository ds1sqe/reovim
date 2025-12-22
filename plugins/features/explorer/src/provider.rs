//! Explorer buffer provider for virtual buffer

use reovim_core::{
    content::{BufferContext, InputResult, PluginBufferProvider},
    event::KeyEvent,
    screen::Position,
};

use reovim_sys::event::KeyCode;

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

                // Reserve last line for message/prompt if present
                let available_height = if explorer.message.is_some() {
                    ctx.height.saturating_sub(1) as usize
                } else {
                    ctx.height as usize
                };

                // Calculate visible range based on scroll offset
                let start = explorer.scroll_offset;
                let end = (start + available_height).min(nodes.len());

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

                // Add message/input prompt at the bottom if present
                tracing::debug!("ExplorerBufferProvider: message={:?}, input_buffer={:?}", explorer.message, explorer.input_buffer);
                if let Some(ref message) = explorer.message {
                    let input_display = if !explorer.input_buffer.is_empty() {
                        explorer.input_buffer.as_str()
                    } else {
                        "_" // Show cursor placeholder
                    };
                    let prompt_line = format!("{}{}", message, input_display);
                    tracing::info!("ExplorerBufferProvider: Adding prompt line: {}", prompt_line);
                    lines.push(prompt_line);
                }

                // Pad with empty lines if needed to fill remaining height
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

    fn on_input(&mut self, key: KeyEvent, ctx: &mut BufferContext) -> InputResult {
        use crate::state::ExplorerInputMode;

        tracing::info!("ExplorerBufferProvider::on_input called with key: {:?}", key);

        // Check if explorer is in input mode (create file, rename, etc.)
        let in_input_mode = ctx.state
            .with::<ExplorerState, _, _>(|explorer| {
                !matches!(explorer.input_mode, ExplorerInputMode::None)
            })
            .unwrap_or(false);

        tracing::info!("ExplorerBufferProvider::on_input: in_input_mode={}", in_input_mode);

        if !in_input_mode {
            return InputResult::Unhandled;
        }

        // Handle character input
        if let KeyCode::Char(c) = key.code {
            tracing::info!("ExplorerBufferProvider::on_input: Adding char '{}' to input buffer", c);
            ctx.state.with_mut::<ExplorerState, _, _>(|explorer| {
                explorer.input_buffer.push(c);
            });
            return InputResult::Handled;
        }

        // Let other keys (Enter, Escape, Backspace) be handled by commands
        InputResult::Unhandled
    }

    fn is_editable(&self) -> bool {
        // Explorer is not directly editable
        false
    }
}
