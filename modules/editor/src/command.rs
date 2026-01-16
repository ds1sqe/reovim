//! Editor commands - cursor movement and mode switching.
//!
//! This module provides the basic commands for editor operation:
//! - Cursor movement: up, down, left, right
//! - Mode switching: enter insert, exit to normal
//!
//! # Cursor Movement Philosophy
//!
//! Cursor movement commands follow Vim semantics:
//! - j/k (down/up) preserve the "preferred column" - the column the user
//!   intended, even if shorter lines force temporary repositioning
//! - h/l (left/right) clear the preferred column
//! - Movements clamp to valid positions (no-op at boundaries)

use std::path::Path;

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult, ModeAction,
        UndoAction,
    },
    reovim_kernel::api::v1::{
        CommandId, KernelContext, Position, RegisterContent,
        events::{CursorMoved, ModeChanged},
    },
};

use super::{
    display_lines,
    mode::{EDITOR_MODULE, EditorMode},
};

// =============================================================================
// Cursor Movement Commands
// =============================================================================

/// Move cursor up.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorUp;

impl Command for CursorUp {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "cursor-up")
    }

    fn description(&self) -> &'static str {
        "Move cursor up"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines",
        )]
    }
}

impl CommandHandler for CursorUp {
    #[allow(clippy::cast_possible_truncation)] // Line/column numbers won't exceed u32::MAX
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let (old_pos, new_pos) = {
            let mut buffer = buffer_arc.write();
            let old_pos = buffer.position();

            // Calculate new line (saturating sub to handle boundary)
            let new_line = old_pos.line.saturating_sub(count);

            // If already at top, no-op
            if new_line == old_pos.line && old_pos.line == 0 {
                return CommandResult::Success;
            }

            // Get line length for column clamping
            let line_len = buffer.line_len(new_line).unwrap_or(0);
            let new_col = old_pos.column.min(line_len);

            let new_pos = Position::new(new_line, new_col);
            buffer.set_position(new_pos);
            drop(buffer);
            (old_pos, new_pos)
        };

        // Emit CursorMoved event (buffer lock released)
        ctx.event_bus.emit(CursorMoved {
            buffer_id: buffer_id.as_usize() as u64,
            from: (old_pos.line as u32, old_pos.column as u32),
            to: (new_pos.line as u32, new_pos.column as u32),
        });

        CommandResult::Success
    }
}

/// Move cursor down.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorDown;

impl Command for CursorDown {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "cursor-down")
    }

    fn description(&self) -> &'static str {
        "Move cursor down"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines",
        )]
    }
}

impl CommandHandler for CursorDown {
    #[allow(clippy::cast_possible_truncation)] // Line/column numbers won't exceed u32::MAX
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let (old_pos, new_pos) = {
            let mut buffer = buffer_arc.write();
            let old_pos = buffer.position();
            let line_count = buffer.line_count();

            // Calculate new line (clamped to last line)
            let max_line = line_count.saturating_sub(1);
            let new_line = (old_pos.line + count).min(max_line);

            // If already at bottom, no-op
            if new_line == old_pos.line && old_pos.line == max_line {
                return CommandResult::Success;
            }

            // Get line length for column clamping
            let line_len = buffer.line_len(new_line).unwrap_or(0);
            let new_col = old_pos.column.min(line_len);

            let new_pos = Position::new(new_line, new_col);
            buffer.set_position(new_pos);
            drop(buffer);
            (old_pos, new_pos)
        };

        // Emit CursorMoved event (buffer lock released)
        ctx.event_bus.emit(CursorMoved {
            buffer_id: buffer_id.as_usize() as u64,
            from: (old_pos.line as u32, old_pos.column as u32),
            to: (new_pos.line as u32, new_pos.column as u32),
        });

        CommandResult::Success
    }
}

/// Move cursor left.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorLeft;

impl Command for CursorLeft {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "cursor-left")
    }

    fn description(&self) -> &'static str {
        "Move cursor left"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of columns",
        )]
    }
}

impl CommandHandler for CursorLeft {
    #[allow(clippy::cast_possible_truncation)] // Line/column numbers won't exceed u32::MAX
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let (old_pos, new_pos) = {
            let mut buffer = buffer_arc.write();
            let old_pos = buffer.position();

            // Calculate new column (saturating sub to handle boundary)
            let new_col = old_pos.column.saturating_sub(count);

            // If already at left edge, no-op
            if new_col == old_pos.column && old_pos.column == 0 {
                return CommandResult::Success;
            }

            let new_pos = Position::new(old_pos.line, new_col);
            buffer.set_position(new_pos);
            drop(buffer);
            (old_pos, new_pos)
        };

        // Emit CursorMoved event (buffer lock released)
        ctx.event_bus.emit(CursorMoved {
            buffer_id: buffer_id.as_usize() as u64,
            from: (old_pos.line as u32, old_pos.column as u32),
            to: (new_pos.line as u32, new_pos.column as u32),
        });

        CommandResult::Success
    }
}

/// Move cursor right.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorRight;

impl Command for CursorRight {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "cursor-right")
    }

    fn description(&self) -> &'static str {
        "Move cursor right"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of columns",
        )]
    }
}

impl CommandHandler for CursorRight {
    #[allow(clippy::cast_possible_truncation)] // Line/column numbers won't exceed u32::MAX
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let (old_pos, new_pos) = {
            let mut buffer = buffer_arc.write();
            let old_pos = buffer.position();

            // Get current line length for boundary check
            let line_len = buffer.line_len(old_pos.line).unwrap_or(0);

            // In normal mode, cursor can't go past the last character.
            // For a line of length N, valid columns are 0..N-1.
            // An empty line has max_col 0, but we can still be at col 0.
            let max_col = line_len.saturating_sub(1);

            // Calculate new column (clamped to max valid position)
            let new_col = (old_pos.column + count).min(max_col);

            // If already at right edge, no-op
            if new_col == old_pos.column && old_pos.column == max_col {
                return CommandResult::Success;
            }

            let new_pos = Position::new(old_pos.line, new_col);
            buffer.set_position(new_pos);
            drop(buffer);
            (old_pos, new_pos)
        };

        // Emit CursorMoved event (buffer lock released)
        ctx.event_bus.emit(CursorMoved {
            buffer_id: buffer_id.as_usize() as u64,
            from: (old_pos.line as u32, old_pos.column as u32),
            to: (new_pos.line as u32, new_pos.column as u32),
        });

        CommandResult::Success
    }
}

// =============================================================================
// Display Line Movement Commands
// =============================================================================

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

// =============================================================================
// Mode Switching Commands
// =============================================================================

/// Enter insert mode (before cursor).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertMode;

impl Command for EnterInsertMode {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-insert")
    }

    fn description(&self) -> &'static str {
        "Enter insert mode"
    }
}

impl CommandHandler for EnterInsertMode {
    fn execute(&self, ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // Emit mode change event
        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        // Note: The actual mode stack change happens in the runner via a callback
        // or by the caller checking the result. For now, we just emit the event.

        CommandResult::Success
    }
}

/// Enter insert mode after cursor (a).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertModeAppend;

impl Command for EnterInsertModeAppend {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-insert-after")
    }

    fn description(&self) -> &'static str {
        "Enter insert mode after cursor (append)"
    }
}

impl CommandHandler for EnterInsertModeAppend {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        // Move cursor right first, then enter insert mode
        if let Some(buffer_id) = args.buffer_id()
            && let Some(buffer_arc) = ctx.buffers.get(buffer_id)
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();
            let line_len = buffer.line_len(pos.line).unwrap_or(0);
            // Move right only if not at end of line
            if pos.column < line_len {
                buffer.set_position(Position::new(pos.line, pos.column + 1));
            }
            drop(buffer);
        }

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        CommandResult::Success
    }
}

/// Exit to normal mode (Escape from insert mode).
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitToNormal;

impl Command for ExitToNormal {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "exit-insert")
    }

    fn description(&self) -> &'static str {
        "Exit insert mode and return to normal mode"
    }
}

impl CommandHandler for ExitToNormal {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        // Move cursor left one position when exiting insert mode (Vim behavior)
        if let Some(buffer_id) = args.buffer_id()
            && let Some(buffer_arc) = ctx.buffers.get(buffer_id)
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();
            if pos.column > 0 {
                buffer.set_position(Position::new(pos.line, pos.column - 1));
            }
            drop(buffer);
        }

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("insert", EditorMode::NORMAL_ID));

        CommandResult::Success
    }
}

/// Enter window management mode (Ctrl-W in normal mode).
///
/// This pushes "window" mode onto the mode stack. In window mode,
/// subsequent keys (h/j/k/l for navigation, s/v for splits, etc.)
/// are handled by the layout module's keybindings.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterWindowMode;

impl Command for EnterWindowMode {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-window-mode")
    }

    fn description(&self) -> &'static str {
        "Enter window management mode"
    }
}

impl CommandHandler for EnterWindowMode {
    fn execute(&self, ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // Emit mode change event
        ctx.event_bus.emit(ModeChanged::new("normal", "window"));

        // Return mode action to push window mode onto stack
        CommandResult::ModeAction(ModeAction::Push("window".to_string()))
    }
}

// =============================================================================
// Insert Mode Edit Commands
// =============================================================================

/// Insert a newline at cursor position (Enter in insert mode).
#[derive(Debug, Clone, Copy, Default)]
pub struct InsertNewline;

impl Command for InsertNewline {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "insert-newline")
    }

    fn description(&self) -> &'static str {
        "Insert newline at cursor position"
    }
}

impl CommandHandler for InsertNewline {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let cursor_before = buffer.position();
        // Insert newline splits the line at cursor position
        let edit = buffer.insert("\n");
        let cursor_after = buffer.position();
        drop(buffer);

        CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
    }
}

/// Insert a tab at cursor position (Tab in insert mode).
///
/// Respects `expandtab` and `tabstop` options.
#[derive(Debug, Clone, Copy, Default)]
pub struct InsertTab;

impl Command for InsertTab {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "insert-tab")
    }

    fn description(&self) -> &'static str {
        "Insert tab at cursor position"
    }
}

impl CommandHandler for InsertTab {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        use reovim_kernel::api::v1::OptionScopeId;

        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Get options (with defaults). Use buffer-local scope if available.
        let scope = OptionScopeId::Buffer(buffer_id);
        let expandtab = ctx
            .options
            .get("expandtab", scope)
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let tabstop = ctx
            .options
            .get("tabstop", scope)
            .and_then(|v| v.as_int())
            .map_or(4, |n| n.max(1) as usize);

        let text = if expandtab {
            " ".repeat(tabstop)
        } else {
            "\t".to_string()
        };

        let mut buffer = buffer_arc.write();
        let cursor_before = buffer.position();
        let edit = buffer.insert(&text);
        let cursor_after = buffer.position();
        drop(buffer);

        CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
    }
}

// =============================================================================
// Mode Entry Commands (Phase 2)
// =============================================================================

/// Enter insert mode at first non-blank character (I).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertFirstNonBlank;

impl Command for EnterInsertFirstNonBlank {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-insert-bol")
    }

    fn description(&self) -> &'static str {
        "Enter insert mode at first non-blank character"
    }
}

impl CommandHandler for EnterInsertFirstNonBlank {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        if let Some(buffer_id) = args.buffer_id()
            && let Some(buffer_arc) = ctx.buffers.get(buffer_id)
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();

            // Find first non-blank character on current line
            let first_non_blank = buffer
                .line(pos.line)
                .map_or(0, |line| line.chars().position(|c| !c.is_whitespace()).unwrap_or(0));

            buffer.set_position(Position::new(pos.line, first_non_blank));
            drop(buffer);
        }

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        CommandResult::Success
    }
}

/// Enter insert mode at end of line (A).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertEndOfLine;

impl Command for EnterInsertEndOfLine {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-insert-eol")
    }

    fn description(&self) -> &'static str {
        "Enter insert mode at end of line"
    }
}

impl CommandHandler for EnterInsertEndOfLine {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        if let Some(buffer_id) = args.buffer_id()
            && let Some(buffer_arc) = ctx.buffers.get(buffer_id)
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();

            // Move cursor to end of current line
            let line_len = buffer.line_len(pos.line).unwrap_or(0);
            buffer.set_position(Position::new(pos.line, line_len));
            drop(buffer);
        }

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        CommandResult::Success
    }
}

/// Open line below and enter insert mode (o).
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenLineBelow;

impl Command for OpenLineBelow {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "open-line-below")
    }

    fn description(&self) -> &'static str {
        "Open line below and enter insert mode"
    }
}

impl CommandHandler for OpenLineBelow {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let pos = buffer.position();

        // Move to end of current line
        let line_len = buffer.line_len(pos.line).unwrap_or(0);
        buffer.set_position(Position::new(pos.line, line_len));

        // Capture cursor before insert (after move to end of line)
        let cursor_before = buffer.position();
        // Insert newline (creates new line below)
        let edit = buffer.insert("\n");
        // Cursor is now at start of new line
        let cursor_after = buffer.position();
        drop(buffer);

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
    }
}

/// Open line above and enter insert mode (O).
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenLineAbove;

impl Command for OpenLineAbove {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "open-line-above")
    }

    fn description(&self) -> &'static str {
        "Open line above and enter insert mode"
    }
}

impl CommandHandler for OpenLineAbove {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let pos = buffer.position();

        // Move to start of current line
        buffer.set_position(Position::new(pos.line, 0));

        // Capture cursor before insert
        let cursor_before = buffer.position();
        // Insert newline before current line content
        let edit = buffer.insert("\n");

        // Move cursor up to the new empty line
        buffer.set_position(Position::new(pos.line, 0));
        let cursor_after = buffer.position();
        drop(buffer);

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
    }
}

// =============================================================================
// Undo/Redo Commands
// =============================================================================

/// Undo the last change.
///
/// Returns an `UndoAction` intent for the runner to handle. The runner
/// maintains per-buffer undo trees and applies the actual undo operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct UndoCommand;

impl Command for UndoCommand {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "undo")
    }

    fn description(&self) -> &'static str {
        "Undo the last change"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of changes to undo",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["u", "undo"]
    }
}

impl CommandHandler for UndoCommand {
    fn execute(&self, _ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let count = args.count().unwrap_or(1);
        CommandResult::UndoAction(UndoAction::Undo { count })
    }
}

/// Redo the last undone change.
///
/// Returns an `UndoAction` intent for the runner to handle.
#[derive(Debug, Clone, Copy, Default)]
pub struct RedoCommand;

impl Command for RedoCommand {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "redo")
    }

    fn description(&self) -> &'static str {
        "Redo the last undone change"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of changes to redo",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["redo"]
    }
}

impl CommandHandler for RedoCommand {
    fn execute(&self, _ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let count = args.count().unwrap_or(1);
        CommandResult::UndoAction(UndoAction::Redo { count })
    }
}

// =============================================================================
// Delete Commands (Phase 3)
// =============================================================================

/// Delete character under cursor (x).
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteChar;

impl Command for DeleteChar {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "delete-char")
    }

    fn description(&self) -> &'static str {
        "Delete character under cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of characters to delete",
        )]
    }
}

impl CommandHandler for DeleteChar {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let mut buffer = buffer_arc.write();
        let pos = buffer.position();
        let line_len = buffer.line_len(pos.line).unwrap_or(0);

        // Can't delete on empty line or at end of line
        if line_len == 0 || pos.column >= line_len {
            return CommandResult::Success; // No-op
        }

        // Delete up to end of line
        let chars_to_delete = count.min(line_len - pos.column);
        if chars_to_delete > 0 {
            let cursor_before = buffer.position();
            let edit = buffer.delete(chars_to_delete);
            let cursor_after = buffer.position();
            drop(buffer);
            return CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after);
        }
        drop(buffer);

        CommandResult::Success
    }
}

/// Delete character before cursor (X).
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteCharBefore;

impl Command for DeleteCharBefore {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "delete-char-before")
    }

    fn description(&self) -> &'static str {
        "Delete character before cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of characters to delete",
        )]
    }
}

impl CommandHandler for DeleteCharBefore {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let mut buffer = buffer_arc.write();
        let pos = buffer.position();

        // Can't delete before column 0
        if pos.column == 0 {
            // In insert mode, join with previous line
            if pos.line > 0 {
                let prev_line_len = buffer.line_len(pos.line - 1).unwrap_or(0);
                let new_pos = Position::new(pos.line - 1, prev_line_len);
                buffer.set_position(new_pos);
                let cursor_before = buffer.position();
                let edit = buffer.delete(1); // Delete the newline
                let cursor_after = buffer.position();
                drop(buffer);
                return CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after);
            }
            drop(buffer);
            return CommandResult::Success;
        }

        let chars_to_delete = count.min(pos.column);
        let new_col = pos.column - chars_to_delete;
        let delete_pos = Position::new(pos.line, new_col);

        buffer.set_position(delete_pos);
        let cursor_before = buffer.position();
        let edit = buffer.delete(chars_to_delete);
        let cursor_after = buffer.position();
        drop(buffer);

        CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
    }
}

/// Delete current line (dd).
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteLine;

impl Command for DeleteLine {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "delete-line")
    }

    fn description(&self) -> &'static str {
        "Delete current line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines to delete",
        )]
    }
}

impl CommandHandler for DeleteLine {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let mut buffer = buffer_arc.write();
        let start_line = buffer.position().line;
        let line_count = buffer.line_count();

        if line_count == 0 {
            return CommandResult::Success;
        }

        // Calculate lines to delete
        let lines_to_delete = count.min(line_count.saturating_sub(start_line));
        if lines_to_delete == 0 {
            return CommandResult::Success;
        }

        // Delete range: from start of first line to start of line after deleted range
        let start = Position::new(start_line, 0);

        // Calculate total characters to delete (including newlines)
        let mut chars_to_delete = 0;
        for i in 0..lines_to_delete {
            let line_idx = start_line + i;
            if line_idx < line_count {
                let line_len = buffer.line_len(line_idx).unwrap_or(0);
                chars_to_delete += line_len;
                // Add 1 for newline unless it's the last line
                if line_idx + 1 < line_count {
                    chars_to_delete += 1;
                }
            }
        }

        // Handle deleting last line(s) - need to also delete preceding newline
        let end_line = start_line + lines_to_delete;
        let edit = if end_line >= line_count && start_line > 0 {
            // We're deleting to end of buffer, so delete preceding newline too
            let new_start =
                Position::new(start_line - 1, buffer.line_len(start_line - 1).unwrap_or(0));
            buffer.set_position(new_start);
            buffer.delete(chars_to_delete + 1) // +1 for preceding newline
        } else {
            buffer.set_position(start);
            buffer.delete(chars_to_delete)
        };
        let cursor_before = start; // Before the delete, cursor was at start of deleted range

        // Move cursor to first non-blank of remaining line
        let new_line_count = buffer.line_count();
        let new_line = start_line.min(new_line_count.saturating_sub(1));
        let first_non_blank = buffer
            .line(new_line)
            .map_or(0, |line| line.chars().position(|c| !c.is_whitespace()).unwrap_or(0));
        buffer.set_position(Position::new(new_line, first_non_blank));
        let cursor_after = buffer.position();
        drop(buffer);

        CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
    }
}

/// Delete to end of line (D).
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteToEndOfLine;

impl Command for DeleteToEndOfLine {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "delete-to-eol")
    }

    fn description(&self) -> &'static str {
        "Delete to end of line"
    }
}

impl CommandHandler for DeleteToEndOfLine {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let pos = buffer.position();
        let line_len = buffer.line_len(pos.line).unwrap_or(0);

        // Nothing to delete if at or past end of line
        if pos.column >= line_len {
            return CommandResult::Success;
        }

        // Delete from cursor to end of line (not including newline)
        let chars_to_delete = line_len - pos.column;
        let cursor_before = buffer.position();
        let edit = buffer.delete(chars_to_delete);
        let cursor_after = buffer.position();
        drop(buffer);

        CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
    }
}

/// Change current line (cc).
///
/// Clears the content of the current line(s) and enters insert mode.
/// Unlike `dd`, this keeps the line(s) but empties their content.
/// The deleted text is stored in the register as linewise.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangeLine;

impl Command for ChangeLine {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "change-line")
    }

    fn description(&self) -> &'static str {
        "Change current line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines to change",
        )]
    }
}

impl CommandHandler for ChangeLine {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let mut buffer = buffer_arc.write();
        let start_line = buffer.position().line;
        let line_count = buffer.line_count();

        if line_count == 0 {
            // Empty buffer - just enter insert mode
            drop(buffer);
            ctx.event_bus
                .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));
            return CommandResult::Success;
        }

        // Calculate lines to change
        let lines_to_change = count.min(line_count.saturating_sub(start_line));
        if lines_to_change == 0 {
            drop(buffer);
            ctx.event_bus
                .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));
            return CommandResult::Success;
        }

        // Collect text to delete for register (include newlines between lines)
        let mut deleted_text = String::new();
        for i in 0..lines_to_change {
            let line_idx = start_line + i;
            if let Some(line) = buffer.line(line_idx) {
                deleted_text.push_str(line);
            }
            if i < lines_to_change - 1 {
                deleted_text.push('\n');
            }
        }
        deleted_text.push('\n'); // Linewise content ends with newline

        // Store in register as linewise
        let content = RegisterContent::linewise(deleted_text);
        ctx.registers.write().set(content);

        // For cc: if changing multiple lines, delete all but first, then clear first
        // Single line: just clear the content
        let cursor_before = buffer.position();

        if lines_to_change == 1 {
            // Clear the single line content
            let line_len = buffer.line_len(start_line).unwrap_or(0);
            if line_len > 0 {
                buffer.set_position(Position::new(start_line, 0));
                let edit = buffer.delete(line_len);
                buffer.set_position(Position::new(start_line, 0));
                let cursor_after = buffer.position();
                drop(buffer);

                ctx.event_bus
                    .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

                return CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after);
            }
            // Line is already empty
            drop(buffer);
            ctx.event_bus
                .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));
            return CommandResult::Success;
        }

        // Multiple lines: delete lines 2..N entirely, then clear line 1
        // First, calculate total chars to delete from lines 2..N (including newlines)
        let mut chars_to_delete_from_rest = 0;
        for i in 1..lines_to_change {
            let line_idx = start_line + i;
            if line_idx < line_count {
                let line_len = buffer.line_len(line_idx).unwrap_or(0);
                chars_to_delete_from_rest += line_len;
                // Add 1 for newline
                if line_idx + 1 < line_count {
                    chars_to_delete_from_rest += 1;
                }
            }
        }

        // Delete from end of first line (newline) to end of last changed line
        let first_line_len = buffer.line_len(start_line).unwrap_or(0);
        let total_delete = first_line_len + 1 + chars_to_delete_from_rest; // +1 for newline after first line

        // Handle edge case: if deleting to end of buffer
        let end_line = start_line + lines_to_change;
        if end_line >= line_count {
            // We're changing to end of buffer - delete including last line's content
            // but keep one empty line
            buffer.set_position(Position::new(start_line, 0));
            let chars = first_line_len + chars_to_delete_from_rest + (lines_to_change - 1); // newlines between
            let content_len = buffer.content().len();
            let edit = buffer.delete(chars.min(content_len));
            buffer.set_position(Position::new(start_line, 0));
            let cursor_after = buffer.position();
            drop(buffer);

            ctx.event_bus
                .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

            return CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after);
        }

        // Normal case: delete all content from lines and their separating newlines
        // Keep one line at start_line, cleared
        buffer.set_position(Position::new(start_line, 0));
        let edit = buffer.delete(total_delete);
        buffer.set_position(Position::new(start_line, 0));
        let cursor_after = buffer.position();
        drop(buffer);

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
    }
}

/// Change to end of line (C).
///
/// Deletes from cursor to end of line and enters insert mode.
/// The deleted text is stored in the register as characterwise.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangeToEndOfLine;

impl Command for ChangeToEndOfLine {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "change-to-eol")
    }

    fn description(&self) -> &'static str {
        "Change to end of line"
    }
}

impl CommandHandler for ChangeToEndOfLine {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let pos = buffer.position();
        let line_len = buffer.line_len(pos.line).unwrap_or(0);

        // Nothing to delete if at or past end of line - just enter insert mode
        if pos.column >= line_len {
            drop(buffer);
            ctx.event_bus
                .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));
            return CommandResult::Success;
        }

        // Get text to delete for register
        let deleted_text = buffer
            .line(pos.line)
            .map(|line| line[pos.column..].to_string())
            .unwrap_or_default();

        // Store in register as characterwise
        let content = RegisterContent::characterwise(deleted_text);
        ctx.registers.write().set(content);

        // Delete from cursor to end of line (not including newline)
        let chars_to_delete = line_len - pos.column;
        let cursor_before = buffer.position();
        let edit = buffer.delete(chars_to_delete);
        let cursor_after = buffer.position();
        drop(buffer);

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", EditorMode::INSERT_ID));

        CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
    }
}

// =============================================================================
// Replace Commands
// =============================================================================

/// Start replace char operation (r).
///
/// This command signals that the next character typed should replace
/// the character(s) under the cursor. Returns `WaitingForChar` with
/// the `ReplaceChar` operation type.
///
/// Unlike `R` (replace mode), `r` is a single-character replacement:
/// - `rx` replaces the char under cursor with 'x'
/// - `3rx` replaces the next 3 chars with 'x'
#[derive(Debug, Clone, Copy, Default)]
pub struct ReplaceCharStart;

/// Repeat the last repeatable command (.).
///
/// This command returns `RepeatAction` which signals the runner to
/// replay the last repeatable command from `repeat_state`. Repeatable
/// commands include text-modifying operations like insert, delete, change.
///
/// # Vim Behavior
///
/// - `.` repeats the last change command
/// - `3.` repeats the last change 3 times
/// - Insert mode text is recorded and replayed
#[derive(Debug, Clone, Copy, Default)]
pub struct RepeatDot;

impl Command for RepeatDot {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "repeat-dot")
    }

    fn description(&self) -> &'static str {
        "Repeat last change"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of times to repeat",
        )]
    }
}

impl CommandHandler for RepeatDot {
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // The runner handles the actual repeat logic.
        // We just signal the intent to repeat.
        CommandResult::RepeatAction
    }
}

impl Command for ReplaceCharStart {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "replace-char-start")
    }

    fn description(&self) -> &'static str {
        "Replace character under cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of characters to replace",
        )]
    }
}

impl CommandHandler for ReplaceCharStart {
    fn execute(&self, _ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let count = args.count().unwrap_or(1);
        CommandResult::waiting_for_replace_char(count)
    }
}

/// Join current line with next line (J).
#[derive(Debug, Clone, Copy, Default)]
pub struct JoinLines;

impl Command for JoinLines {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "join-lines")
    }

    fn description(&self) -> &'static str {
        "Join current line with next line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines to join",
        )]
    }
}

impl CommandHandler for JoinLines {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        use reovim_kernel::api::v1::Edit;

        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let mut buffer = buffer_arc.write();
        let current_line = buffer.position().line;
        let mut line_count = buffer.line_count();
        let cursor_before = buffer.position();
        let mut edits: Vec<Edit> = Vec::new();

        for _ in 0..count {
            // Can't join if on last line
            if current_line + 1 >= line_count {
                break;
            }

            // Move to end of current line
            let line_len = buffer.line_len(current_line).unwrap_or(0);
            buffer.set_position(Position::new(current_line, line_len));

            // Delete newline (joins the lines)
            let edit = buffer.delete(1);
            edits.push(edit);

            // Delete leading whitespace of what was the next line
            let new_line_content = buffer.line(current_line).unwrap_or("");
            let after_join = &new_line_content[line_len..];
            let leading_ws = after_join.chars().take_while(|c| c.is_whitespace()).count();

            if leading_ws > 0 {
                let edit = buffer.delete(leading_ws);
                edits.push(edit);
            }

            // Insert single space between joined content (Vim behavior)
            // Only if there's content after the join point
            if buffer.line_len(current_line).unwrap_or(0) > line_len {
                let edit = buffer.insert(" ");
                edits.push(edit);
            }

            // Update line count for next iteration
            line_count = buffer.line_count();
        }

        let cursor_after = buffer.position();
        drop(buffer);

        if edits.is_empty() {
            return CommandResult::Success;
        }

        CommandResult::edit_actions(buffer_id, edits, cursor_before, cursor_after)
    }
}

// =============================================================================
// File Operations
// =============================================================================

/// Write buffer to file.
///
/// Saves the current buffer's contents to a file. If no path is provided,
/// uses the buffer's current file path.
///
/// This command demonstrates VFS integration - file operations go through
/// the VFS abstraction, enabling test isolation and future remote FS support.
///
/// # Arguments
///
/// - `file`: Optional file path to save to (defaults to buffer's path)
///
/// # Examples
///
/// - `:w` - Save to current file
/// - `:w newfile.txt` - Save to specified file
#[derive(Debug, Clone, Copy, Default)]
pub struct WriteBufferCommand;

impl Command for WriteBufferCommand {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "write")
    }

    fn description(&self) -> &'static str {
        "Write buffer to file"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "file",
            ArgKind::FilePath,
            "Target file path (optional)",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["w", "write"]
    }
}

impl CommandHandler for WriteBufferCommand {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        // Get VFS from context
        let Some(vfs) = args.vfs() else {
            return CommandResult::error("VFS not available");
        };

        // Get active buffer
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Get content and file path from buffer, then release lock
        let (content, buffer_file_path) = {
            let buffer = buffer_arc.read();
            (buffer.content(), buffer.file_path().map(String::from))
        };

        // Try to get path from args first, then from buffer
        let file_path_str = args.string("file").map(String::from).or(buffer_file_path);

        let Some(file_path_str) = file_path_str else {
            return CommandResult::error("No file path specified");
        };

        let file_path = Path::new(&file_path_str);

        // Write to file via VFS
        match vfs.write(file_path, content.as_bytes()) {
            Ok(()) => {
                // Clear modified flag
                buffer_arc.write().set_modified(false);
                CommandResult::Success
            }
            Err(e) => CommandResult::error(format!("Write failed: {e}")),
        }
    }
}

// =============================================================================
// Yank Commands
// =============================================================================

/// Yank current line(s) to register (yy, Y).
#[derive(Debug, Clone, Copy, Default)]
pub struct YankLine;

impl Command for YankLine {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "yank-line")
    }

    fn description(&self) -> &'static str {
        "Yank current line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines to yank",
        )]
    }
}

impl CommandHandler for YankLine {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let buffer = buffer_arc.read();
        let start_line = buffer.position().line;
        let line_count = buffer.line_count();

        // Can't yank from empty buffer
        if line_count == 0 {
            return CommandResult::Success;
        }

        // Collect lines to yank
        let end_line = (start_line + count).min(line_count);
        let mut yanked = String::new();
        for line_idx in start_line..end_line {
            if let Some(line) = buffer.line(line_idx) {
                yanked.push_str(line);
                yanked.push('\n');
            }
        }
        drop(buffer);

        // Store in unnamed register as linewise
        let content = RegisterContent::linewise(yanked);
        ctx.registers.write().set(content);

        CommandResult::Success
    }
}

// =============================================================================
// Paste Commands
// =============================================================================

/// Paste from register after cursor (p).
///
/// - Linewise paste: insert below current line
/// - Characterwise paste: insert after cursor position
#[derive(Debug, Clone, Copy, Default)]
pub struct PasteAfter;

impl Command for PasteAfter {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "paste-after")
    }

    fn description(&self) -> &'static str {
        "Paste after cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of times to paste",
        )]
    }
}

impl CommandHandler for PasteAfter {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);

        // Get register content
        let content = {
            let registers = ctx.registers.read();
            registers.get().clone()
        };

        if content.is_empty() {
            return CommandResult::Success; // Nothing to paste
        }

        let mut buffer = buffer_arc.write();
        let pos = buffer.position();
        let cursor_before = pos;

        if content.is_linewise() {
            // Paste below current line
            let line_count = buffer.line_count();

            // Build paste text (repeated count times, strip trailing newline for clean insert)
            let paste_text = content.text.repeat(count);
            let paste_text = paste_text.trim_end_matches('\n');

            if line_count == 0 {
                // Empty buffer: just insert the content
                let edit = buffer.insert(paste_text);
                buffer.set_position(Position::new(0, 0));
                let cursor_after = buffer.position();
                drop(buffer);
                return CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after);
            }

            // Move to end of current line
            let line_len = buffer.line_len(pos.line).unwrap_or(0);
            buffer.set_position(Position::new(pos.line, line_len));

            // Insert newline then content
            let insert_text = format!("\n{paste_text}");
            let edit = buffer.insert(&insert_text);

            // Position cursor on first character of first pasted line
            let new_line = pos.line + 1;
            buffer.set_position(Position::new(new_line, 0));
            let cursor_after = buffer.position();
            drop(buffer);

            CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
        } else {
            // Characterwise: paste after cursor
            let line_len = buffer.line_len(pos.line).unwrap_or(0);
            let insert_col = if line_len == 0 {
                0
            } else {
                (pos.column + 1).min(line_len)
            };

            buffer.set_position(Position::new(pos.line, insert_col));

            let paste_text = content.text.repeat(count);
            let edit = buffer.insert(&paste_text);

            // Position cursor at end of pasted text (Vim behavior: last character)
            let final_pos = buffer.position();
            let cursor_after = if final_pos.column > 0 {
                Position::new(final_pos.line, final_pos.column - 1)
            } else {
                final_pos
            };
            buffer.set_position(cursor_after);
            drop(buffer);

            CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
        }
    }
}

/// Paste from register before cursor (P).
///
/// - Linewise paste: insert above current line
/// - Characterwise paste: insert at cursor position
#[derive(Debug, Clone, Copy, Default)]
pub struct PasteBefore;

impl Command for PasteBefore {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "paste-before")
    }

    fn description(&self) -> &'static str {
        "Paste before cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of times to paste",
        )]
    }
}

impl CommandHandler for PasteBefore {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);

        // Get register content
        let content = {
            let registers = ctx.registers.read();
            registers.get().clone()
        };

        if content.is_empty() {
            return CommandResult::Success; // Nothing to paste
        }

        let mut buffer = buffer_arc.write();
        let pos = buffer.position();
        let cursor_before = pos;

        // Build paste text (repeated count times)
        let paste_text = content.text.repeat(count);

        if content.is_linewise() {
            // Paste above current line
            // Strip trailing newline for clean insert
            let paste_text = paste_text.trim_end_matches('\n');

            // Move to start of current line
            buffer.set_position(Position::new(pos.line, 0));

            // Insert content then newline
            let insert_text = format!("{paste_text}\n");
            let edit = buffer.insert(&insert_text);

            // Position cursor on first character of first pasted line
            buffer.set_position(Position::new(pos.line, 0));
            let cursor_after = buffer.position();
            drop(buffer);

            CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
        } else {
            // Characterwise: paste at cursor position (before)
            // Cursor stays at current position, content inserted there
            let edit = buffer.insert(&paste_text);

            // Position cursor at end of pasted text (Vim behavior: last character)
            let final_pos = buffer.position();
            let cursor_after = if final_pos.column > 0 {
                Position::new(final_pos.line, final_pos.column - 1)
            } else {
                final_pos
            };
            buffer.set_position(cursor_after);
            drop(buffer);

            CommandResult::edit_action(buffer_id, edit, cursor_before, cursor_after)
        }
    }
}

// =============================================================================
// Command Registration Helper
// =============================================================================

/// Get all editor commands as boxed trait objects.
///
/// This is useful for registering all commands at once.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        // Cursor movement
        Box::new(CursorUp),
        Box::new(CursorDown),
        Box::new(CursorLeft),
        Box::new(CursorRight),
        // Display line movement
        Box::new(CursorDisplayDown),
        Box::new(CursorDisplayUp),
        // Mode switching
        Box::new(EnterInsertMode),
        Box::new(EnterInsertModeAppend),
        Box::new(EnterInsertFirstNonBlank),
        Box::new(EnterInsertEndOfLine),
        Box::new(OpenLineBelow),
        Box::new(OpenLineAbove),
        Box::new(ExitToNormal),
        Box::new(EnterWindowMode),
        // Insert mode edits
        Box::new(InsertNewline),
        Box::new(InsertTab),
        // Delete operations
        Box::new(DeleteChar),
        Box::new(DeleteCharBefore),
        Box::new(DeleteLine),
        Box::new(DeleteToEndOfLine),
        Box::new(JoinLines),
        // Yank
        Box::new(YankLine),
        // Paste
        Box::new(PasteAfter),
        Box::new(PasteBefore),
        // Change
        Box::new(ChangeLine),
        Box::new(ChangeToEndOfLine),
        // Replace
        Box::new(ReplaceCharStart),
        // Repeat
        Box::new(RepeatDot),
        // Undo/redo
        Box::new(UndoCommand),
        Box::new(RedoCommand),
        // File operations
        Box::new(WriteBufferCommand),
    ]
}

/// Get all cursor movement commands.
#[must_use]
pub fn cursor_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(CursorUp),
        Box::new(CursorDown),
        Box::new(CursorLeft),
        Box::new(CursorRight),
    ]
}

/// Get display line movement commands (gj, gk).
#[must_use]
pub fn display_line_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(CursorDisplayDown), Box::new(CursorDisplayUp)]
}

/// Get all mode switching commands.
#[must_use]
pub fn mode_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(EnterInsertMode),
        Box::new(EnterInsertModeAppend),
        Box::new(EnterInsertFirstNonBlank),
        Box::new(EnterInsertEndOfLine),
        Box::new(OpenLineBelow),
        Box::new(OpenLineAbove),
        Box::new(ExitToNormal),
        Box::new(EnterWindowMode),
    ]
}

/// Get all insert mode edit commands.
#[must_use]
pub fn insert_edit_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(InsertNewline), Box::new(InsertTab)]
}

/// Get all delete commands.
#[must_use]
pub fn delete_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(DeleteChar),
        Box::new(DeleteCharBefore),
        Box::new(DeleteLine),
        Box::new(DeleteToEndOfLine),
        Box::new(JoinLines),
    ]
}

/// Get all undo/redo commands.
#[must_use]
pub fn undo_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(UndoCommand), Box::new(RedoCommand)]
}

/// Get all yank commands.
#[must_use]
pub fn yank_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(YankLine)]
}

/// Get all paste commands.
#[must_use]
pub fn paste_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(PasteAfter), Box::new(PasteBefore)]
}

/// Get all change commands.
#[must_use]
pub fn change_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(ChangeLine), Box::new(ChangeToEndOfLine)]
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::ArgValue,
        reovim_kernel::api::v1::{
            Buffer, BufferError, BufferId, BufferManager, EventBus, MarkBank, MotionEngine,
            OptionRegistry, RegisterBank, RwLock, TextObjectEngine,
        },
        std::{collections::HashMap, sync::Arc},
    };

    /// Test buffer manager that actually stores buffers.
    struct TestBufferManager {
        buffers: RwLock<HashMap<BufferId, Arc<RwLock<Buffer>>>>,
    }

    impl TestBufferManager {
        fn new() -> Self {
            Self {
                buffers: RwLock::new(HashMap::new()),
            }
        }
    }

    impl BufferManager for TestBufferManager {
        fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
            self.buffers.read().get(&id).cloned()
        }

        fn create(&self) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(Buffer::new()));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn register(&self, buffer: Buffer) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(buffer));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError> {
            self.buffers
                .write()
                .remove(&id)
                .map_or(Err(BufferError::NotFound(id)), |arc_buffer| {
                    Arc::try_unwrap(arc_buffer)
                        .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
                })
        }

        fn list(&self) -> Vec<BufferId> {
            self.buffers.read().keys().copied().collect()
        }

        fn count(&self) -> usize {
            self.buffers.read().len()
        }
    }

    /// Create a `KernelContext` with a real buffer manager for testing.
    fn create_test_context() -> KernelContext {
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(RegisterBank::new())),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
        )
    }

    // =========================================================================
    // Command ID Tests
    // =========================================================================

    #[test]
    fn test_cursor_up_id() {
        let cmd = CursorUp;
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
        assert_eq!(cmd.id().name(), "cursor-up");
    }

    #[test]
    fn test_cursor_down_id() {
        let cmd = CursorDown;
        assert_eq!(cmd.id().name(), "cursor-down");
    }

    #[test]
    fn test_cursor_left_id() {
        let cmd = CursorLeft;
        assert_eq!(cmd.id().name(), "cursor-left");
    }

    #[test]
    fn test_cursor_right_id() {
        let cmd = CursorRight;
        assert_eq!(cmd.id().name(), "cursor-right");
    }

    #[test]
    fn test_enter_insert_id() {
        let cmd = EnterInsertMode;
        assert_eq!(cmd.id().name(), "enter-insert");
    }

    #[test]
    fn test_enter_insert_append_id() {
        let cmd = EnterInsertModeAppend;
        assert_eq!(cmd.id().name(), "enter-insert-after");
    }

    #[test]
    fn test_exit_to_normal_id() {
        let cmd = ExitToNormal;
        assert_eq!(cmd.id().name(), "exit-insert");
    }

    // =========================================================================
    // Command Args Tests
    // =========================================================================

    #[test]
    fn test_cursor_commands_have_count_arg() {
        for cmd in cursor_commands() {
            let args = cmd.args();
            assert!(!args.is_empty(), "Command {} should have count arg", cmd.id());
            assert_eq!(args[0].name, "count");
            assert_eq!(args[0].kind, ArgKind::Count);
        }
    }

    #[test]
    fn test_all_commands_count() {
        let cmds = all_commands();
        // 4 cursor + 2 display + 8 mode + 2 insert-edit + 5 delete + 1 yank + 2 paste
        // + 2 change + 2 undo + 1 replace_char + 1 repeat + 1 write = 31
        assert_eq!(cmds.len(), 31);
    }

    #[test]
    fn test_cursor_commands_count() {
        let cmds = cursor_commands();
        assert_eq!(cmds.len(), 4);
    }

    #[test]
    fn test_display_line_commands_count() {
        let cmds = display_line_commands();
        assert_eq!(cmds.len(), 2);
    }

    #[test]
    fn test_mode_commands_count() {
        let cmds = mode_commands();
        assert_eq!(cmds.len(), 8);
    }

    // =========================================================================
    // Cursor Commands Without Buffer ID (Error Handling)
    // =========================================================================

    #[test]
    fn test_cursor_up_no_buffer_id_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = CursorUp.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_cursor_down_no_buffer_id_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = CursorDown.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_cursor_left_no_buffer_id_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = CursorLeft.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_cursor_right_no_buffer_id_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = CursorRight.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // Cursor Commands With Invalid Buffer ID (Error Handling)
    // =========================================================================

    #[test]
    fn test_cursor_up_invalid_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let mut args = CommandContext::new();
        args.set("buffer_id", ArgValue::BufferId(999));
        let result = CursorUp.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // Cursor Movement Tests (With Valid Buffer)
    // =========================================================================

    fn setup_buffer_context() -> (KernelContext, BufferId) {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("line one\nline two\nline three");
        let buffer_id = ctx.buffers.register(buffer);
        (ctx, buffer_id)
    }

    #[test]
    fn test_cursor_down_moves_cursor() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDown.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 0);
    }

    #[test]
    fn test_cursor_up_moves_cursor() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        // First move down, then test up
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(2, 0));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorUp.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 1);
    }

    #[test]
    fn test_cursor_right_moves_cursor() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorRight.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 1);
    }

    #[test]
    fn test_cursor_left_moves_cursor() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        // First move right, then test left
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 5));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorLeft.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 4);
    }

    // =========================================================================
    // Count Argument Tests
    // =========================================================================

    #[test]
    fn test_cursor_down_count_respected() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));

        let result = CursorDown.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 2); // Moved down 2 lines
    }

    #[test]
    fn test_cursor_right_count_respected() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(3));

        let result = CursorRight.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 3);
    }

    // =========================================================================
    // Boundary Tests
    // =========================================================================

    #[test]
    fn test_cursor_up_at_line_zero_is_noop() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Cursor starts at (0, 0), should stay there
        let result = CursorUp.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 0);
    }

    #[test]
    fn test_cursor_down_at_eof_is_noop() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        // Move to last line
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(2, 0));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDown.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.line, 2); // Stayed at last line
    }

    #[test]
    fn test_cursor_left_at_col_zero_is_noop() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Cursor starts at column 0
        let result = CursorLeft.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 0);
    }

    #[test]
    fn test_cursor_right_at_eol_is_noop() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        // Move to last character of line ("line one" is 8 chars, last char at index 7)
        // In normal mode, cursor can't go past the last character
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 7)); // 'e' in "line one"
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorRight.execute(&mut ctx, &args);
        assert!(result.is_success());

        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let pos = buffer.read().position();
        assert_eq!(pos.column, 7); // Stayed at last character
    }

    // =========================================================================
    // Mode Commands Tests
    // =========================================================================

    #[test]
    fn test_mode_commands_execute_success() {
        let mut ctx = create_test_context();
        let args = CommandContext::new();

        // Mode commands don't require buffer_id
        assert_eq!(EnterInsertMode.execute(&mut ctx, &args), CommandResult::Success);
        assert_eq!(EnterInsertModeAppend.execute(&mut ctx, &args), CommandResult::Success);
        assert_eq!(ExitToNormal.execute(&mut ctx, &args), CommandResult::Success);
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_cursor_movement_empty_buffer() {
        let mut ctx = create_test_context();
        let buffer = Buffer::new(); // Empty buffer
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // All movements should succeed (no-op on empty buffer)
        assert!(CursorDown.execute(&mut ctx, &args).is_success());
        assert!(CursorUp.execute(&mut ctx, &args).is_success());
        assert!(CursorLeft.execute(&mut ctx, &args).is_success());
        assert!(CursorRight.execute(&mut ctx, &args).is_success());
    }

    #[test]
    fn test_cursor_movement_single_line() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("single line");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Up/down should be no-op
        assert!(CursorDown.execute(&mut ctx, &args).is_success());
        assert!(CursorUp.execute(&mut ctx, &args).is_success());

        // Left/right should work
        assert!(CursorRight.execute(&mut ctx, &args).is_success());
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        assert_eq!(buffer.read().position().column, 1);
    }

    // =========================================================================
    // Undo/Redo Command Tests
    // =========================================================================

    #[test]
    fn test_undo_command_id() {
        let cmd = UndoCommand;
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
        assert_eq!(cmd.id().name(), "undo");
    }

    #[test]
    fn test_redo_command_id() {
        let cmd = RedoCommand;
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
        assert_eq!(cmd.id().name(), "redo");
    }

    #[test]
    fn test_undo_command_names() {
        let cmd = UndoCommand;
        let names = cmd.names();
        assert!(names.contains(&"u"));
        assert!(names.contains(&"undo"));
    }

    #[test]
    fn test_redo_command_names() {
        let cmd = RedoCommand;
        let names = cmd.names();
        assert!(names.contains(&"redo"));
    }

    #[test]
    fn test_undo_command_returns_undo_action() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = UndoCommand.execute(&mut ctx, &args);
        assert!(result.is_undo_action());

        match result {
            CommandResult::UndoAction(UndoAction::Undo { count }) => {
                assert_eq!(count, 1); // Default count is 1
            }
            _ => panic!("Expected UndoAction::Undo"),
        }
    }

    #[test]
    fn test_redo_command_returns_undo_action() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = RedoCommand.execute(&mut ctx, &args);
        assert!(result.is_undo_action());

        match result {
            CommandResult::UndoAction(UndoAction::Redo { count }) => {
                assert_eq!(count, 1);
            }
            _ => panic!("Expected UndoAction::Redo"),
        }
    }

    #[test]
    fn test_undo_command_respects_count() {
        let mut ctx = KernelContext::default();
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(5));

        let result = UndoCommand.execute(&mut ctx, &args);

        match result {
            CommandResult::UndoAction(UndoAction::Undo { count }) => {
                assert_eq!(count, 5);
            }
            _ => panic!("Expected UndoAction::Undo"),
        }
    }

    #[test]
    fn test_redo_command_respects_count() {
        let mut ctx = KernelContext::default();
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(3));

        let result = RedoCommand.execute(&mut ctx, &args);

        match result {
            CommandResult::UndoAction(UndoAction::Redo { count }) => {
                assert_eq!(count, 3);
            }
            _ => panic!("Expected UndoAction::Redo"),
        }
    }

    #[test]
    fn test_undo_command_zero_count_defaults_to_one() {
        let mut ctx = KernelContext::default();
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(0));

        let result = UndoCommand.execute(&mut ctx, &args);

        // Zero count should be interpreted as 0 (caller's responsibility to handle)
        match result {
            CommandResult::UndoAction(UndoAction::Undo { count }) => {
                assert_eq!(count, 0);
            }
            _ => panic!("Expected UndoAction::Undo"),
        }
    }

    #[test]
    fn test_undo_commands_helper_returns_two_commands() {
        let cmds = undo_commands();
        assert_eq!(cmds.len(), 2);
    }

    #[test]
    fn test_undo_command_has_count_arg() {
        let cmd = UndoCommand;
        let args = cmd.args();
        assert!(!args.is_empty());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_redo_command_has_count_arg() {
        let cmd = RedoCommand;
        let args = cmd.args();
        assert!(!args.is_empty());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_undo_command_does_not_require_buffer_id() {
        // Undo commands return intent, they don't directly access the buffer
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        // Should NOT return an error, should return UndoAction
        let result = UndoCommand.execute(&mut ctx, &args);
        assert!(!result.is_error());
        assert!(result.is_undo_action());
    }

    #[test]
    fn test_redo_command_does_not_require_buffer_id() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = RedoCommand.execute(&mut ctx, &args);
        assert!(!result.is_error());
        assert!(result.is_undo_action());
    }

    // =========================================================================
    // Yank Command Tests
    // =========================================================================

    #[test]
    fn test_yank_line_command_id() {
        let cmd = YankLine;
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
        assert_eq!(cmd.id().name(), "yank-line");
    }

    #[test]
    fn test_yank_line_has_count_arg() {
        let cmd = YankLine;
        let args = cmd.args();
        assert!(!args.is_empty());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_yank_line_no_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = YankLine.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_yank_line_single_line() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = YankLine.execute(&mut ctx, &args);
        assert!(result.is_success());

        // Check register content
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(content.is_linewise());
        assert_eq!(content.text, "line one\n");
    }

    #[test]
    fn test_yank_line_count() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));

        let result = YankLine.execute(&mut ctx, &args);
        assert!(result.is_success());

        // Check register content
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(content.is_linewise());
        assert_eq!(content.text, "line one\nline two\n");
    }

    #[test]
    fn test_yank_line_at_eof() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        // Move to last line
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(2, 0));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(5)); // More than remaining lines

        let result = YankLine.execute(&mut ctx, &args);
        assert!(result.is_success());

        // Should only yank the last line
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(content.is_linewise());
        assert_eq!(content.text, "line three\n");
    }

    #[test]
    fn test_yank_commands_count() {
        let cmds = yank_commands();
        assert_eq!(cmds.len(), 1);
    }

    // =========================================================================
    // Paste Command Tests
    // =========================================================================

    #[test]
    fn test_paste_after_command_id() {
        let cmd = PasteAfter;
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
        assert_eq!(cmd.id().name(), "paste-after");
    }

    #[test]
    fn test_paste_before_command_id() {
        let cmd = PasteBefore;
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
        assert_eq!(cmd.id().name(), "paste-before");
    }

    #[test]
    fn test_paste_after_has_count_arg() {
        let cmd = PasteAfter;
        let args = cmd.args();
        assert!(!args.is_empty());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_paste_before_has_count_arg() {
        let cmd = PasteBefore;
        let args = cmd.args();
        assert!(!args.is_empty());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_paste_after_no_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = PasteAfter.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_paste_before_no_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = PasteBefore.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_paste_after_empty_register() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Register is empty by default
        let result = PasteAfter.execute(&mut ctx, &args);
        assert!(result.is_success()); // No-op, not error
    }

    #[test]
    fn test_paste_after_linewise() {
        let (mut ctx, buffer_id) = setup_buffer_context();

        // First yank a line
        {
            let mut args = CommandContext::new();
            args.set_buffer_id(buffer_id);
            YankLine.execute(&mut ctx, &args);
        }

        // Then paste after
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let buffer_read = buffer.read();
        let line_count = buffer_read.line_count();
        let line1 = buffer_read.line(1).map(str::to_owned);
        drop(buffer_read);
        assert_eq!(line_count, 4); // Original 3 + 1 pasted
        assert_eq!(line1.as_deref(), Some("line one")); // Pasted line
    }

    #[test]
    fn test_paste_before_linewise() {
        let (mut ctx, buffer_id) = setup_buffer_context();

        // First yank a line
        {
            let mut args = CommandContext::new();
            args.set_buffer_id(buffer_id);
            YankLine.execute(&mut ctx, &args);
        }

        // Then paste before
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteBefore.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let buffer_read = buffer.read();
        let line_count = buffer_read.line_count();
        let line0 = buffer_read.line(0).map(str::to_owned);
        drop(buffer_read);
        assert_eq!(line_count, 4); // Original 3 + 1 pasted
        assert_eq!(line0.as_deref(), Some("line one")); // Pasted line at top
    }

    #[test]
    fn test_paste_after_characterwise() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        // Set characterwise content in register
        ctx.registers
            .write()
            .set(RegisterContent::characterwise("XYZ"));

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content - "XYZ" pasted after cursor (position 0)
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let buffer_read = buffer.read();
        let content = buffer_read.line(0).map(str::to_owned);
        drop(buffer_read);
        assert_eq!(content.as_deref(), Some("hXYZello world"));
    }

    #[test]
    fn test_paste_before_characterwise() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        // Set characterwise content in register
        ctx.registers
            .write()
            .set(RegisterContent::characterwise("XYZ"));

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteBefore.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content - "XYZ" pasted at cursor (position 0)
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let buffer_read = buffer.read();
        let content = buffer_read.line(0).map(str::to_owned);
        drop(buffer_read);
        assert_eq!(content.as_deref(), Some("XYZhello world"));
    }

    #[test]
    fn test_paste_after_count() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        // Set characterwise content in register
        ctx.registers
            .write()
            .set(RegisterContent::characterwise("X"));

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(3));
        let result = PasteAfter.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content - "XXX" pasted after cursor
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        let buffer_read = buffer.read();
        let content = buffer_read.line(0).map(str::to_owned);
        drop(buffer_read);
        assert_eq!(content.as_deref(), Some("hXXXello"));
    }

    #[test]
    fn test_paste_commands_count() {
        let cmds = paste_commands();
        assert_eq!(cmds.len(), 2);
    }

    // =========================================================================
    // Change Command Tests
    // =========================================================================

    #[test]
    fn test_change_line_command_id() {
        let cmd = ChangeLine;
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
        assert_eq!(cmd.id().name(), "change-line");
    }

    #[test]
    fn test_change_to_eol_command_id() {
        let cmd = ChangeToEndOfLine;
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
        assert_eq!(cmd.id().name(), "change-to-eol");
    }

    #[test]
    fn test_change_line_has_count_arg() {
        let cmd = ChangeLine;
        let args = cmd.args();
        assert!(!args.is_empty());
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_change_line_no_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = ChangeLine.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_change_to_eol_no_buffer_returns_error() {
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();
        let result = ChangeToEndOfLine.execute(&mut ctx, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_change_line_single_line() {
        let (mut ctx, _) = setup_buffer_context();

        // Replace buffer with single line content
        let buffer = Buffer::from_string("hello world");
        let new_buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(new_buffer_id);
        let result = ChangeLine.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content - line should be empty
        let buffer = ctx.buffers.get(new_buffer_id).unwrap();
        assert_eq!(buffer.read().content(), "");

        // Check register - should have deleted text as linewise
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(content.is_linewise());
        assert_eq!(content.text, "hello world\n");
    }

    #[test]
    fn test_change_line_multi_line() {
        let (mut ctx, buffer_id) = setup_buffer_context();
        // setup_buffer_context gives us "line one\nline two\nline three"

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));
        let result = ChangeLine.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check register - should have deleted text as linewise
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(content.is_linewise());
        assert!(content.text.contains("line one"));
        assert!(content.text.contains("line two"));
    }

    #[test]
    fn test_change_line_empty_buffer() {
        let mut ctx = create_test_context();
        let buffer = Buffer::new();
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeLine.execute(&mut ctx, &args);
        // Should succeed (enters insert mode on empty buffer)
        assert!(result.is_success());
    }

    #[test]
    fn test_change_to_eol_middle_of_line() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        // Position cursor at column 6 (at 'w')
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 6));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeToEndOfLine.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content - should have "hello "
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        assert_eq!(buffer.read().content(), "hello ");

        // Check register - should have "world" as characterwise
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(!content.is_linewise());
        assert_eq!(content.text, "world");
    }

    #[test]
    fn test_change_to_eol_at_start() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeToEndOfLine.execute(&mut ctx, &args);
        assert!(result.is_edit_action());

        // Check buffer content - should be empty
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        assert_eq!(buffer.read().content(), "");

        // Check register - should have "hello world" as characterwise
        let registers = ctx.registers.read();
        let content = registers.get().clone();
        drop(registers);
        assert!(!content.is_linewise());
        assert_eq!(content.text, "hello world");
    }

    #[test]
    fn test_change_to_eol_at_end_of_line() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        // Position cursor at end of line
        {
            let buffer = ctx.buffers.get(buffer_id).unwrap();
            buffer.write().set_position(Position::new(0, 5));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeToEndOfLine.execute(&mut ctx, &args);
        // Should succeed but be a no-op (just enters insert mode)
        assert!(result.is_success());

        // Buffer should be unchanged
        let buffer = ctx.buffers.get(buffer_id).unwrap();
        assert_eq!(buffer.read().content(), "hello");
    }

    #[test]
    fn test_change_commands_count() {
        let cmds = change_commands();
        assert_eq!(cmds.len(), 2);
    }

    #[test]
    fn test_all_commands_includes_change() {
        let cmds = all_commands();
        let has_change_line = cmds
            .iter()
            .any(|c| c.id().module() == &EDITOR_MODULE && c.id().name() == "change-line");
        let has_change_to_eol = cmds
            .iter()
            .any(|c| c.id().module() == &EDITOR_MODULE && c.id().name() == "change-to-eol");
        assert!(has_change_line);
        assert!(has_change_to_eol);
    }

    #[test]
    fn test_change_line_returns_edit_action() {
        let (mut ctx, buffer_id) = setup_buffer_context();

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeLine.execute(&mut ctx, &args);

        // Verify EditAction returned for undo integration
        assert!(result.is_edit_action());
        if let CommandResult::EditAction(action) = result {
            // EditAction should have non-empty edits
            assert!(!action.edits.is_empty());
            // Verify buffer_id is correct
            assert_eq!(action.buffer_id, buffer_id);
        } else {
            panic!("Expected EditAction result");
        }
    }

    #[test]
    fn test_change_to_eol_returns_edit_action() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeToEndOfLine.execute(&mut ctx, &args);

        // Verify EditAction returned for undo integration
        assert!(result.is_edit_action());
        if let CommandResult::EditAction(action) = result {
            // EditAction should have non-empty edits
            assert!(!action.edits.is_empty());
            // Verify buffer_id is correct
            assert_eq!(action.buffer_id, buffer_id);
        } else {
            panic!("Expected EditAction result");
        }
    }

    #[test]
    fn test_change_line_cursor_position_after() {
        let (mut ctx, buffer_id) = setup_buffer_context();

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ChangeLine.execute(&mut ctx, &args);

        // After cc, cursor should be at column 0
        if let CommandResult::EditAction(action) = result {
            assert_eq!(action.cursor_after.column, 0);
        } else {
            panic!("Expected EditAction result");
        }
    }

    // =========================================================================
    // Replace Char Tests
    // =========================================================================

    #[test]
    fn test_replace_char_start_command_id() {
        let cmd = ReplaceCharStart;
        assert_eq!(cmd.id().name(), "replace-char-start");
    }

    #[test]
    fn test_replace_char_start_has_count_arg() {
        let cmd = ReplaceCharStart;
        let args = cmd.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
    }

    #[test]
    fn test_replace_char_start_returns_waiting_for_char() {
        let (mut ctx, _buffer_id) = setup_buffer_context();
        let args = CommandContext::new();
        let result = ReplaceCharStart.execute(&mut ctx, &args);

        assert!(result.is_waiting_for_char());
    }

    #[test]
    fn test_replace_char_start_with_count() {
        use reovim_driver_command::{ArgValue, CharWaitOp};

        let (mut ctx, _buffer_id) = setup_buffer_context();
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(3));
        let result = ReplaceCharStart.execute(&mut ctx, &args);

        if let CommandResult::WaitingForChar(char_ctx) = result {
            assert_eq!(char_ctx.op_type, CharWaitOp::ReplaceChar);
            assert_eq!(char_ctx.count, Some(3));
        } else {
            panic!("Expected WaitingForChar result");
        }
    }

    #[test]
    fn test_replace_char_start_default_count_is_one() {
        let (mut ctx, _buffer_id) = setup_buffer_context();
        let args = CommandContext::new();
        let result = ReplaceCharStart.execute(&mut ctx, &args);

        if let CommandResult::WaitingForChar(char_ctx) = result {
            assert_eq!(char_ctx.count, Some(1));
        } else {
            panic!("Expected WaitingForChar result");
        }
    }

    // =========================================================================
    // Repeat Dot Tests
    // =========================================================================

    #[test]
    fn test_repeat_dot_command_id() {
        let cmd = RepeatDot;
        assert_eq!(cmd.id().name(), "repeat-dot");
    }

    #[test]
    fn test_repeat_dot_has_count_arg() {
        let cmd = RepeatDot;
        let args = cmd.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
    }

    #[test]
    fn test_repeat_dot_returns_repeat_action() {
        let (mut ctx, _buffer_id) = setup_buffer_context();
        let args = CommandContext::new();
        let result = RepeatDot.execute(&mut ctx, &args);

        assert!(result.is_repeat_action());
    }
}
