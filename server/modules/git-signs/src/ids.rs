//! Command and module IDs for the git-signs module.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Module identifier for git-signs.
pub const MODULE: ModuleId = ModuleId::new("git-signs");

/// Navigate to the next hunk.
pub const NEXT_HUNK: CommandId = CommandId::new(MODULE, "next-hunk");

/// Navigate to the previous hunk.
pub const PREV_HUNK: CommandId = CommandId::new(MODULE, "prev-hunk");

/// Stage the hunk under cursor.
pub const STAGE_HUNK: CommandId = CommandId::new(MODULE, "stage-hunk");

/// Reset the hunk under cursor.
pub const RESET_HUNK: CommandId = CommandId::new(MODULE, "reset-hunk");

/// Stage all hunks in the current buffer.
pub const STAGE_BUFFER: CommandId = CommandId::new(MODULE, "stage-buffer");

/// Reset all changes in the current buffer.
pub const RESET_BUFFER: CommandId = CommandId::new(MODULE, "reset-buffer");

/// Unstage the current file.
pub const UNSTAGE_FILE: CommandId = CommandId::new(MODULE, "unstage-file");

/// Preview hunk diff.
pub const PREVIEW_HUNK: CommandId = CommandId::new(MODULE, "preview-hunk");

/// Show diff for current file.
pub const DIFF_THIS: CommandId = CommandId::new(MODULE, "diff-this");

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
