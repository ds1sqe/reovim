//! Screen position motion commands.
//!
//! Implements vim screen-relative motions: `H`, `M`, `L`.
//!
//! These commands move the cursor to visible lines relative to the viewport.
//! They are linewise jump motions that push to the jump list.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{ChangeTracker, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, JumpEntry},
    reovim_types_text::Position,
};

use crate::ids;

// =============================================================================
// Helper
// =============================================================================

/// Execute a screen position jump: push to jump list, move cursor, record change.
#[cfg_attr(coverage_nightly, coverage(off))]
fn execute_screen_jump(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    target_line: usize,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    let Some(window) = runtime.windows().active() else {
        return CommandResult::error("No active window");
    };
    let old_pos = Position::new(window.cursor.line, window.cursor.column);

    // Clamp target to buffer bounds
    let line_count = runtime
        .with_text_geometry(buffer_id, |buf| buf.line_count())
        .unwrap_or(1);
    let clamped = target_line.min(line_count.saturating_sub(1));

    if clamped == old_pos.line {
        return CommandResult::Success;
    }

    // Push to jump list before major jump (#654)
    runtime
        .jumplist_mut()
        .push(JumpEntry::new(buffer_id, old_pos));

    // Move cursor to column 0 of target line
    if let Some(window) = runtime.windows_mut().active_mut() {
        window.cursor.line = clamped;
        window.cursor.column = 0;
    }

    runtime.record_cursor_move(buffer_id);

    CommandResult::Success
}

// =============================================================================
// Screen High (H)
// =============================================================================

/// Move cursor to top of screen.
///
/// With count N, moves to the Nth line from the top of the viewport.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScreenHigh;

impl Command for ScreenHigh {
    fn id(&self) -> CommandId {
        ids::SCREEN_HIGH
    }

    fn description(&self) -> &'static str {
        "Move to top of screen"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Lines from top")]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for ScreenHigh {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let viewport = &window.viewport;
        if viewport.height == 0 {
            return CommandResult::Success;
        }

        let count = args.count().unwrap_or(1).max(1);
        let target = viewport.scroll_top + count - 1;

        execute_screen_jump(runtime, args, target)
    }
}

// =============================================================================
// Screen Middle (M)
// =============================================================================

/// Move cursor to middle of screen.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScreenMiddle;

impl Command for ScreenMiddle {
    fn id(&self) -> CommandId {
        ids::SCREEN_MIDDLE
    }

    fn description(&self) -> &'static str {
        "Move to middle of screen"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for ScreenMiddle {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let viewport = &window.viewport;
        if viewport.height == 0 {
            return CommandResult::Success;
        }

        let target = viewport.scroll_top + usize::from(viewport.height) / 2;

        execute_screen_jump(runtime, args, target)
    }
}

// =============================================================================
// Screen Low (L)
// =============================================================================

/// Move cursor to bottom of screen.
///
/// With count N, moves to the Nth line from the bottom of the viewport.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScreenLow;

impl Command for ScreenLow {
    fn id(&self) -> CommandId {
        ids::SCREEN_LOW
    }

    fn description(&self) -> &'static str {
        "Move to bottom of screen"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Lines from bottom",
        )]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for ScreenLow {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let viewport = &window.viewport;
        if viewport.height == 0 {
            return CommandResult::Success;
        }

        let count = args.count().unwrap_or(1).max(1);
        let raw_target = viewport.scroll_top + usize::from(viewport.height).saturating_sub(count);
        // Can't go above scroll_top (when count > height)
        let target = raw_target.max(viewport.scroll_top);

        execute_screen_jump(runtime, args, target)
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all screen position motion commands.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(ScreenHigh),
        Box::new(ScreenMiddle),
        Box::new(ScreenLow),
    ]
}
