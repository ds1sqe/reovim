//! Visual mode entry commands.
//!
//! Provides commands to enter the various visual selection modes:
//! - `v` - Character-wise selection
//! - `V` - Line-wise selection
//! - `Ctrl-V` - Block (rectangular) selection

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
            BufferApi, SessionRuntime, TransitionContext,
        api::{ChangeTracker, ModeApi, Selection},
    },
    reovim_kernel::api::v1::{CommandId, Position},
};

use crate::{ids, modes::VimMode};

/// Enter visual mode (character-wise selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualMode;

impl Command for EnterVisualMode {
    fn id(&self) -> CommandId {
        ids::ENTER_VISUAL
    }

    fn description(&self) -> &'static str {
        "Enter visual mode (character-wise)"
    }
}

impl CommandHandler for EnterVisualMode {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Get current cursor position from per-client window
        let Some(window) = runtime.windows().active() else {
            tracing::warn!("EnterVisualMode: No active window");
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Start character-wise selection at current cursor position.
        // Phase 8 (#465): Use exclusive end semantics - to include the character
        // at the cursor, end must be cursor + 1.
        let selection = Selection::character(pos, Position::new(pos.line, pos.column + 1));
        tracing::debug!(?pos, ?selection, "EnterVisualMode: Setting selection");

        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = Some(selection);
        }

        // #474: Notify other clients about new selection
        if let Some(buffer_id) = runtime.active_buffer() {
            runtime.record_selection_change(buffer_id);
        }

        // Verify selection was set
        let sel_check = runtime.windows().active().and_then(|w| w.selection.clone());
        tracing::debug!(?sel_check, "EnterVisualMode: Selection after set");

        // Change to visual mode
        runtime.set_mode(VimMode::VISUAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Enter visual line mode (line-wise selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualLineMode;

impl Command for EnterVisualLineMode {
    fn id(&self) -> CommandId {
        ids::ENTER_VISUAL_LINE
    }

    fn description(&self) -> &'static str {
        "Enter visual line mode"
    }
}

impl CommandHandler for EnterVisualLineMode {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Get current cursor position from per-client window
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Start line-wise selection at current cursor position.
        // Phase 8 (#465): Use exclusive end semantics - to select the current line,
        // end.line must be pos.line + 1.
        let selection = Selection::line(Position::new(pos.line, 0), Position::new(pos.line + 1, 0));

        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = Some(selection);
        }

        // #474: Notify other clients about new selection
        if let Some(buffer_id) = runtime.active_buffer() {
            runtime.record_selection_change(buffer_id);
        }

        // Change to visual line mode
        runtime.set_mode(VimMode::VISUAL_LINE_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Enter visual block mode (rectangular selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualBlockMode;

impl Command for EnterVisualBlockMode {
    fn id(&self) -> CommandId {
        ids::ENTER_VISUAL_BLOCK
    }

    fn description(&self) -> &'static str {
        "Enter visual block mode"
    }
}

impl CommandHandler for EnterVisualBlockMode {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Get current cursor position from per-client window
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Start block selection at current cursor position.
        // Phase 8 (#465): Use exclusive end semantics - to include the character
        // at the cursor, end must be cursor + 1.
        let selection = Selection::block(pos, Position::new(pos.line, pos.column + 1));

        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = Some(selection);
        }

        // #474: Notify other clients about new selection
        if let Some(buffer_id) = runtime.active_buffer() {
            runtime.record_selection_change(buffer_id);
        }

        // Change to visual block mode
        runtime.set_mode(VimMode::VISUAL_BLOCK_ID, TransitionContext::new());

        CommandResult::Success
    }
}
