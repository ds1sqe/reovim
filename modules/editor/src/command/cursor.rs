//! Cursor movement commands.
//!
//! Provides basic cursor movement: up, down, left, right.
//!
//! # Cursor Movement Philosophy
//!
//! Cursor movement commands follow Vim semantics:
//! - j/k (down/up) preserve the "preferred column" - the column the user
//!   intended, even if shorter lines force temporary repositioning
//! - h/l (left/right) clear the preferred column
//! - Movements clamp to valid positions (no-op at boundaries)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_kernel::api::v1::{CommandId, KernelContext, Position, events::CursorMoved},
};

use super::super::mode::EDITOR_MODULE;

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
        let buffer = buffer_arc.read();
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
        drop(buffer);

        // In operator-pending mode, return range for the operator
        // j/k motions are linewise
        if args.is_operator_pending() {
            // k moves up, so new_pos.line < old_pos.line
            return CommandResult::operator_range(new_pos, old_pos, true);
        }

        // Normal mode: move cursor
        {
            let mut buffer = buffer_arc.write();
            buffer.set_position(new_pos);
        }

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
        let buffer = buffer_arc.read();
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
        drop(buffer);

        // In operator-pending mode, return range for the operator
        // j/k motions are linewise
        if args.is_operator_pending() {
            // j moves down, so old_pos.line < new_pos.line
            return CommandResult::operator_range(old_pos, new_pos, true);
        }

        // Normal mode: move cursor
        {
            let mut buffer = buffer_arc.write();
            buffer.set_position(new_pos);
        }

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
        let buffer = buffer_arc.read();
        let old_pos = buffer.position();

        // Calculate new column (saturating sub to handle boundary)
        let new_col = old_pos.column.saturating_sub(count);

        // If already at left edge, no-op
        if new_col == old_pos.column && old_pos.column == 0 {
            return CommandResult::Success;
        }

        let new_pos = Position::new(old_pos.line, new_col);
        drop(buffer);

        // In operator-pending mode, return range for the operator
        // h/l motions are characterwise
        if args.is_operator_pending() {
            // h moves left, so new_pos.column < old_pos.column
            return CommandResult::operator_range(new_pos, old_pos, false);
        }

        // Normal mode: move cursor
        {
            let mut buffer = buffer_arc.write();
            buffer.set_position(new_pos);
        }

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
        let buffer = buffer_arc.read();
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
        drop(buffer);

        // In operator-pending mode, return range for the operator
        // h/l motions are characterwise
        if args.is_operator_pending() {
            // l moves right, so old_pos.column < new_pos.column
            return CommandResult::operator_range(old_pos, new_pos, false);
        }

        // Normal mode: move cursor
        {
            let mut buffer = buffer_arc.write();
            buffer.set_position(new_pos);
        }

        // Emit CursorMoved event (buffer lock released)
        ctx.event_bus.emit(CursorMoved {
            buffer_id: buffer_id.as_usize() as u64,
            from: (old_pos.line as u32, old_pos.column as u32),
            to: (new_pos.line as u32, new_pos.column as u32),
        });

        CommandResult::Success
    }
}
