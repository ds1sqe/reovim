//! Display line movement commands.
//!
//! Provides cursor movement based on display (visual) lines rather than buffer lines.
//! When text wraps across multiple terminal lines, gj/gk move one visual line
//! rather than one buffer line.
//!
//! These commands are unicode-aware and handle:
//! - Tab characters (expand based on `tabstop` setting)
//! - CJK double-width characters (take 2 display columns)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::{CommandId, OptionScopeId, Position, events::CursorMoved},
};

use {super::super::display_lines, crate::ids};

/// Get the tabstop setting for a buffer.
fn get_tabstop(runtime: &SessionRuntime<'_>, buffer_id: reovim_kernel::api::v1::BufferId) -> usize {
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    runtime
        .kernel()
        .options
        .get("tabstop", OptionScopeId::Buffer(buffer_id))
        .and_then(|v| v.as_int())
        .map_or(8, |n| n.max(1) as usize)
}

/// Move cursor down one display line (gj).
///
/// When text wraps across multiple terminal lines, this moves down one
/// visual line rather than one buffer line. On unwrapped lines, behaves
/// like regular `j`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorDisplayDown;

impl Command for CursorDisplayDown {
    fn id(&self) -> CommandId {
        ids::CURSOR_DISPLAY_DOWN
    }

    fn description(&self) -> &'static str {
        "Move cursor down one display line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of display lines",
        )]
    }
}

impl CommandHandler for CursorDisplayDown {
    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = runtime.kernel().buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Terminal width: use 80 as default
        // Note: In the future, this could be retrieved from session state
        // or passed through CommandContext.
        let terminal_width = 80;
        let tabstop = get_tabstop(runtime, buffer_id);

        let count = args.count().unwrap_or(1);
        let (old_pos, new_pos) = {
            let mut buffer = buffer_arc.write();
            let old_pos = buffer.position();
            let line_count = buffer.line_count();

            // Get current line content
            let current_line = buffer.line(old_pos.line).unwrap_or("");

            // Use unicode-aware functions for proper tab and CJK handling
            let display_lines_in_current =
                display_lines::display_line_count_unicode(current_line, terminal_width, tabstop);
            let (current_display_line, display_col) = display_lines::display_position_unicode(
                current_line,
                old_pos.column,
                terminal_width,
                tabstop,
            );

            // Calculate how many display lines we can move within this buffer line
            let remaining_display_lines =
                display_lines_in_current.saturating_sub(current_display_line + 1);

            let new_pos = if count <= remaining_display_lines {
                // Stay on same buffer line, move to next display line
                let target_display_line = current_display_line + count;
                let target_display_col = target_display_line * terminal_width + display_col;
                let new_col = display_lines::buffer_col_from_display_col_unicode(
                    current_line,
                    target_display_col,
                    tabstop,
                );
                // Clamp to line length
                let line_len = current_line.chars().count();
                let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
                Position::new(old_pos.line, clamped_col)
            } else {
                // Need to move to next buffer line(s)
                let mut lines_to_move = count - remaining_display_lines;
                let mut new_line = old_pos.line + 1;

                while lines_to_move > 0 && new_line < line_count {
                    let line = buffer.line(new_line).unwrap_or("");
                    let display_count =
                        display_lines::display_line_count_unicode(line, terminal_width, tabstop);

                    if lines_to_move <= display_count {
                        // Target is within this line
                        let target_display = lines_to_move - 1;
                        let target_display_col = target_display * terminal_width + display_col;
                        let new_col = display_lines::buffer_col_from_display_col_unicode(
                            line,
                            target_display_col,
                            tabstop,
                        );
                        let line_len = line.chars().count();
                        let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
                        buffer.set_position(Position::new(new_line, clamped_col));
                        drop(buffer);

                        runtime.kernel().event_bus.emit(CursorMoved {
                            buffer_id: buffer_id.as_usize() as u64,
                            from: (old_pos.line as u32, old_pos.column as u32),
                            to: (new_line as u32, clamped_col as u32),
                        });

                        return CommandResult::Success;
                    }
                    lines_to_move -= display_count;
                    new_line += 1;
                }

                // Reached end of buffer - go to last line, last display line
                let last_line = line_count.saturating_sub(1);
                let last_content = buffer.line(last_line).unwrap_or("");
                let last_display_count = display_lines::display_line_count_unicode(
                    last_content,
                    terminal_width,
                    tabstop,
                );
                let target_display = last_display_count.saturating_sub(1);
                let target_display_col = target_display * terminal_width + display_col;
                let new_col = display_lines::buffer_col_from_display_col_unicode(
                    last_content,
                    target_display_col,
                    tabstop,
                );
                let line_len = last_content.chars().count();
                let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
                Position::new(last_line, clamped_col)
            };

            buffer.set_position(new_pos);
            drop(buffer);
            (old_pos, new_pos)
        };

        runtime.kernel().event_bus.emit(CursorMoved {
            buffer_id: buffer_id.as_usize() as u64,
            from: (old_pos.line as u32, old_pos.column as u32),
            to: (new_pos.line as u32, new_pos.column as u32),
        });

        CommandResult::Success
    }
}

/// Move cursor up one display line (gk).
///
/// When text wraps across multiple terminal lines, this moves up one
/// visual line rather than one buffer line. On unwrapped lines, behaves
/// like regular `k`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorDisplayUp;

impl Command for CursorDisplayUp {
    fn id(&self) -> CommandId {
        ids::CURSOR_DISPLAY_UP
    }

    fn description(&self) -> &'static str {
        "Move cursor up one display line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of display lines",
        )]
    }
}

impl CommandHandler for CursorDisplayUp {
    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = runtime.kernel().buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Terminal width: use 80 as default
        // Note: In the future, this could be retrieved from session state
        // or passed through CommandContext.
        let terminal_width = 80;
        let tabstop = get_tabstop(runtime, buffer_id);

        let count = args.count().unwrap_or(1);
        let (old_pos, new_pos) = {
            let mut buffer = buffer_arc.write();
            let old_pos = buffer.position();

            // Get current line content
            let current_line = buffer.line(old_pos.line).unwrap_or("");

            // Use unicode-aware functions for proper tab and CJK handling
            let (current_display_line, display_col) = display_lines::display_position_unicode(
                current_line,
                old_pos.column,
                terminal_width,
                tabstop,
            );

            let new_pos = if count <= current_display_line {
                // Stay on same buffer line, move to previous display line
                let target_display_line = current_display_line - count;
                let target_display_col = target_display_line * terminal_width + display_col;
                let new_col = display_lines::buffer_col_from_display_col_unicode(
                    current_line,
                    target_display_col,
                    tabstop,
                );
                // Clamp to line length
                let line_len = current_line.chars().count();
                let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
                Position::new(old_pos.line, clamped_col)
            } else {
                // Need to move to previous buffer line(s)
                let mut lines_to_move = count - current_display_line;
                let mut new_line = old_pos.line;

                while lines_to_move > 0 && new_line > 0 {
                    new_line -= 1;
                    let line = buffer.line(new_line).unwrap_or("");
                    let display_count =
                        display_lines::display_line_count_unicode(line, terminal_width, tabstop);

                    if lines_to_move <= display_count {
                        // Target is within this line (from bottom)
                        let target_display = display_count - lines_to_move;
                        let target_display_col = target_display * terminal_width + display_col;
                        let new_col = display_lines::buffer_col_from_display_col_unicode(
                            line,
                            target_display_col,
                            tabstop,
                        );
                        let line_len = line.chars().count();
                        let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
                        buffer.set_position(Position::new(new_line, clamped_col));
                        drop(buffer);

                        runtime.kernel().event_bus.emit(CursorMoved {
                            buffer_id: buffer_id.as_usize() as u64,
                            from: (old_pos.line as u32, old_pos.column as u32),
                            to: (new_line as u32, clamped_col as u32),
                        });

                        return CommandResult::Success;
                    }
                    lines_to_move -= display_count;
                }

                // Reached beginning of buffer - go to first line, first display line
                let first_content = buffer.line(0).unwrap_or("");
                let new_col = display_lines::buffer_col_from_display_col_unicode(
                    first_content,
                    display_col,
                    tabstop,
                );
                let line_len = first_content.chars().count();
                let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
                Position::new(0, clamped_col)
            };

            buffer.set_position(new_pos);
            drop(buffer);
            (old_pos, new_pos)
        };

        runtime.kernel().event_bus.emit(CursorMoved {
            buffer_id: buffer_id.as_usize() as u64,
            from: (old_pos.line as u32, old_pos.column as u32),
            to: (new_pos.line as u32, new_pos.column as u32),
        });

        CommandResult::Success
    }
}
