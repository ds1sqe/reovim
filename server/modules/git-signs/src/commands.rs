//! Command handlers for git hunk operations.

use {
    reovim_driver_command::{Command, CommandHandler, CommandResult},
    reovim_driver_git::GitProviderStore,
    reovim_driver_text_session::{BufferApi, ChangeTracker, SessionRuntime, WindowApi},
    reovim_kernel::api::v1::CommandId,
    reovim_subsys_command_types::CommandContext,
};

use crate::{ids, navigation};

// ============================================================================
// Navigation
// ============================================================================

/// Jump to the next diff hunk.
#[derive(Debug, Clone, Copy, Default)]
pub struct NextHunk;

impl Command for NextHunk {
    fn id(&self) -> CommandId {
        ids::NEXT_HUNK
    }

    fn description(&self) -> &'static str {
        "Next git hunk"
    }
}

impl CommandHandler for NextHunk {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        navigate_hunk(runtime, navigation::next_hunk_line)
    }
}

/// Jump to the previous diff hunk.
#[derive(Debug, Clone, Copy, Default)]
pub struct PrevHunk;

impl Command for PrevHunk {
    fn id(&self) -> CommandId {
        ids::PREV_HUNK
    }

    fn description(&self) -> &'static str {
        "Previous git hunk"
    }
}

impl CommandHandler for PrevHunk {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        navigate_hunk(runtime, navigation::prev_hunk_line)
    }
}

// ============================================================================
// Stage/Reset operations
// ============================================================================

/// Stage the hunk under cursor.
#[derive(Debug, Clone, Copy, Default)]
pub struct StageHunk;

impl Command for StageHunk {
    fn id(&self) -> CommandId {
        ids::STAGE_HUNK
    }

    fn description(&self) -> &'static str {
        "Stage git hunk"
    }
}

impl CommandHandler for StageHunk {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Stage hunk requires generating a patch — deferred to integration
        let _ = runtime;
        CommandResult::Success
    }
}

/// Reset the hunk under cursor.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResetHunk;

impl Command for ResetHunk {
    fn id(&self) -> CommandId {
        ids::RESET_HUNK
    }

    fn description(&self) -> &'static str {
        "Reset git hunk"
    }
}

impl CommandHandler for ResetHunk {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let _ = runtime;
        CommandResult::Success
    }
}

/// Stage all hunks in the current buffer.
#[derive(Debug, Clone, Copy, Default)]
pub struct StageBuffer;

impl Command for StageBuffer {
    fn id(&self) -> CommandId {
        ids::STAGE_BUFFER
    }

    fn description(&self) -> &'static str {
        "Stage entire buffer"
    }
}

impl CommandHandler for StageBuffer {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(buf_id) = runtime.active_buffer() else {
            return CommandResult::Success;
        };
        let Some(path) = runtime.buffer_file_path(buf_id) else {
            return CommandResult::Success;
        };
        let Some(store) = runtime.kernel().services.get::<GitProviderStore>() else {
            return CommandResult::Success;
        };
        let Some(git) = store.get() else {
            return CommandResult::Success;
        };
        git.stage_file(std::path::Path::new(&path));
        CommandResult::Success
    }
}

/// Reset all changes in the current buffer.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResetBuffer;

impl Command for ResetBuffer {
    fn id(&self) -> CommandId {
        ids::RESET_BUFFER
    }

    fn description(&self) -> &'static str {
        "Reset entire buffer"
    }
}

impl CommandHandler for ResetBuffer {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(buf_id) = runtime.active_buffer() else {
            return CommandResult::Success;
        };
        let Some(path) = runtime.buffer_file_path(buf_id) else {
            return CommandResult::Success;
        };
        let Some(store) = runtime.kernel().services.get::<GitProviderStore>() else {
            return CommandResult::Success;
        };
        let Some(git) = store.get() else {
            return CommandResult::Success;
        };
        git.reset_file(std::path::Path::new(&path));
        CommandResult::Success
    }
}

/// Unstage the current file.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnstageFile;

impl Command for UnstageFile {
    fn id(&self) -> CommandId {
        ids::UNSTAGE_FILE
    }

    fn description(&self) -> &'static str {
        "Unstage file"
    }
}

impl CommandHandler for UnstageFile {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(buf_id) = runtime.active_buffer() else {
            return CommandResult::Success;
        };
        let Some(path) = runtime.buffer_file_path(buf_id) else {
            return CommandResult::Success;
        };
        let Some(store) = runtime.kernel().services.get::<GitProviderStore>() else {
            return CommandResult::Success;
        };
        let Some(git) = store.get() else {
            return CommandResult::Success;
        };
        git.unstage_file(std::path::Path::new(&path));
        CommandResult::Success
    }
}

// ============================================================================
// Preview
// ============================================================================

/// Preview the hunk diff.
#[derive(Debug, Clone, Copy, Default)]
pub struct PreviewHunk;

impl Command for PreviewHunk {
    fn id(&self) -> CommandId {
        ids::PREVIEW_HUNK
    }

    fn description(&self) -> &'static str {
        "Preview hunk diff"
    }
}

impl CommandHandler for PreviewHunk {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let _ = runtime;
        CommandResult::Success
    }
}

/// Show full diff for the current file.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiffThis;

impl Command for DiffThis {
    fn id(&self) -> CommandId {
        ids::DIFF_THIS
    }

    fn description(&self) -> &'static str {
        "Show file diff"
    }
}

impl CommandHandler for DiffThis {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let _ = runtime;
        CommandResult::Success
    }
}

// ============================================================================
// Factory
// ============================================================================

/// Create all command handlers for the git-signs module.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(NextHunk),
        Box::new(PrevHunk),
        Box::new(StageHunk),
        Box::new(ResetHunk),
        Box::new(StageBuffer),
        Box::new(ResetBuffer),
        Box::new(UnstageFile),
        Box::new(PreviewHunk),
        Box::new(DiffThis),
    ]
}

// ============================================================================
// Helpers
// ============================================================================

/// Navigate to a hunk using the given direction function.
#[cfg_attr(coverage_nightly, coverage(off))]
fn navigate_hunk(
    runtime: &mut SessionRuntime<'_>,
    direction: fn(&[reovim_driver_git::types::DiffHunk], usize) -> Option<usize>,
) -> CommandResult {
    let Some(buf_id) = runtime.active_buffer() else {
        return CommandResult::Success;
    };
    let Some(path) = runtime.buffer_file_path(buf_id) else {
        return CommandResult::Success;
    };
    let Some(cursor) = runtime.cursor_position() else {
        return CommandResult::Success;
    };
    let Some(store) = runtime.kernel().services.get::<GitProviderStore>() else {
        return CommandResult::Success;
    };
    let Some(git) = store.get() else {
        return CommandResult::Success;
    };

    let hunks = git.diff_hunks(std::path::Path::new(&path));
    if let Some(target_line) = direction(&hunks, cursor.line) {
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor.line = target_line;
            window.cursor.column = 0;
        }
        runtime.record_cursor_move(buf_id);
    }

    CommandResult::Success
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
