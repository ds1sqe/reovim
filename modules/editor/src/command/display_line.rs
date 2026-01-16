//! Display line movement commands.
//!
//! Provides cursor movement based on display (visual) lines rather than buffer lines.
//! When text wraps across multiple terminal lines, gj/gk move one visual line
//! rather than one buffer line.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_kernel::api::v1::{CommandId, KernelContext, Position, events::CursorMoved},
};

use super::super::{display_lines, mode::EDITOR_MODULE};

/// Move cursor down one display line (gj).
///
/// When text wraps across multiple terminal lines, this moves down one
/// visual line rather than one buffer line. On unwrapped lines, behaves
/// like regular `j`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorDisplayDown;

impl Command for CursorDisplayDown {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "cursor-display-down")
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Terminal width: use 80 as default
        // Note: In the future, this could be retrieved from session state
        // or passed through CommandContext. See issue #248 for tracking.
        let terminal_width = 80;

        let count = args.count().unwrap_or(1);
        let (old_pos, new_pos) = {
            let mut buffer = buffer_arc.write();
            let old_pos = buffer.position();
            let line_count = buffer.line_count();

            // Get current line content
            let current_line = buffer.line(old_pos.line).unwrap_or("");
            let display_lines_in_current =
                display_lines::display_line_count(current_line, terminal_width);
            let (current_display_line, display_col) =
                display_lines::display_position(old_pos.column, terminal_width);

            // Calculate how many display lines we can move within this buffer line
            let remaining_display_lines =
                display_lines_in_current.saturating_sub(current_display_line + 1);

            let new_pos = if count <= remaining_display_lines {
                // Stay on same buffer line, move to next display line
                let target_display_line = current_display_line + count;
                let new_col =
                    display_lines::buffer_column(target_display_line, display_col, terminal_width);
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
                    let display_count = display_lines::display_line_count(line, terminal_width);

                    if lines_to_move <= display_count {
                        // Target is within this line
                        let target_display = lines_to_move - 1;
                        let new_col = display_lines::buffer_column(
                            target_display,
                            display_col,
                            terminal_width,
                        );
                        let line_len = line.chars().count();
                        let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
                        buffer.set_position(Position::new(new_line, clamped_col));
                        drop(buffer);

                        ctx.event_bus.emit(CursorMoved {
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
                let last_display_count =
                    display_lines::display_line_count(last_content, terminal_width);
                let target_display = last_display_count.saturating_sub(1);
                let new_col =
                    display_lines::buffer_column(target_display, display_col, terminal_width);
                let line_len = last_content.chars().count();
                let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
                Position::new(last_line, clamped_col)
            };

            buffer.set_position(new_pos);
            drop(buffer);
            (old_pos, new_pos)
        };

        ctx.event_bus.emit(CursorMoved {
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
        CommandId::new(EDITOR_MODULE, "cursor-display-up")
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Terminal width: use 80 as default
        // Note: In the future, this could be retrieved from session state
        // or passed through CommandContext. See issue #248 for tracking.
        let terminal_width = 80;

        let count = args.count().unwrap_or(1);
        let (old_pos, new_pos) = {
            let mut buffer = buffer_arc.write();
            let old_pos = buffer.position();

            // Get current line content
            let current_line = buffer.line(old_pos.line).unwrap_or("");
            let (current_display_line, display_col) =
                display_lines::display_position(old_pos.column, terminal_width);

            let new_pos = if count <= current_display_line {
                // Stay on same buffer line, move to previous display line
                let target_display_line = current_display_line - count;
                let new_col =
                    display_lines::buffer_column(target_display_line, display_col, terminal_width);
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
                    let display_count = display_lines::display_line_count(line, terminal_width);

                    if lines_to_move <= display_count {
                        // Target is within this line (from bottom)
                        let target_display = display_count - lines_to_move;
                        let new_col = display_lines::buffer_column(
                            target_display,
                            display_col,
                            terminal_width,
                        );
                        let line_len = line.chars().count();
                        let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
                        buffer.set_position(Position::new(new_line, clamped_col));
                        drop(buffer);

                        ctx.event_bus.emit(CursorMoved {
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
                let new_col = display_lines::buffer_column(0, display_col, terminal_width);
                let line_len = first_content.chars().count();
                let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
                Position::new(0, clamped_col)
            };

            buffer.set_position(new_pos);
            drop(buffer);
            (old_pos, new_pos)
        };

        ctx.event_bus.emit(CursorMoved {
            buffer_id: buffer_id.as_usize() as u64,
            from: (old_pos.line as u32, old_pos.column as u32),
            to: (new_pos.line as u32, new_pos.column as u32),
        });

        CommandResult::Success
    }
}
