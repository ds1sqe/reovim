//! Enhanced find-char command with multi-match jump labels.
//!
//! Overrides vim's `EXECUTE_FIND_CHAR` command when loaded.
//! When multiple matches exist for f/F/t/T on a line (and count == 1),
//! activates jump labels via `JumpSessionState` instead of jumping to the
//! first match.
//!
//! # Architecture
//!
//! This command lives in range-finder because it couples vim's motion
//! semantics with range-finder's jump state machine (`JumpSessionState`).
//! It is registered during `RangeFinderModule::init()` when a personality
//! manifest provides mode bridges.
//!
//! ```text
//! VimNormalResolver → Execute(EXECUTE_FIND_CHAR, ctx)
//!                          ↓
//!         ┌────────────────────────────────────┐
//!         │  EnhancedFindCharCommand (adapter)  │
//!         │  replaces vim's basic handler       │
//!         └────────────────────────────────────┘
//!              ↓ single match        ↓ multi match
//!         MotionEngine          JumpSessionState
//!         (kernel)              (range-finder)
//! ```

use {
    crate::jump::{
        ids as jump_ids,
        search::{JumpMatch, generate_labels},
        state::JumpSessionState,
    },
    reovim_driver_command::{
        ArgValue, Command, CommandContext, CommandHandler, CommandPriority, CommandResult,
    },
    reovim_driver_session::{
        ExtensionApi, ModeApi, SessionRuntime, TransitionContext, api::ChangeTracker,
    },
    reovim_kernel::api::v1::{CommandId, Cursor, ModuleId, Motion, MotionEngine, Position},
};

/// Vim module's command ID for find-char execution.
///
/// Must match `vim::ids::EXECUTE_FIND_CHAR`. Constructed locally to avoid
/// the adapter depending on the vim module (reverse coupling).
pub const EXECUTE_FIND_CHAR: CommandId = CommandId::new(ModuleId::new("vim"), "execute-find-char");

/// Enhanced find-char command that shows jump labels for multi-match cases.
///
/// When the target character appears multiple times on the line (and count == 1),
/// this command activates `JumpSessionState` with labeled positions and pushes
/// jump-input mode for label selection.
///
/// For single-match or counted motions, it delegates to the standard
/// `MotionEngine::calculate` path (same behavior as vim's basic handler).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnhancedFindCharCommand;

impl Command for EnhancedFindCharCommand {
    fn id(&self) -> CommandId {
        EXECUTE_FIND_CHAR
    }

    fn description(&self) -> &'static str {
        "Enhanced find-char motion with multi-match jump labels"
    }

    fn priority(&self) -> CommandPriority {
        CommandPriority::Override
    }
}

/// Count all occurrences of `target` on the current line in the given direction.
///
/// Returns positions as `(line, col)` pairs sorted by distance from cursor.
fn find_all_char_matches(
    line_text: &str,
    cursor_col: usize,
    cursor_line: usize,
    target: char,
    forward: bool,
) -> Vec<(u32, u32)> {
    let chars: Vec<char> = line_text.chars().collect();
    let mut positions = Vec::new();

    if forward {
        for (i, &c) in chars.iter().enumerate().skip(cursor_col + 1) {
            if c == target {
                #[allow(clippy::cast_possible_truncation)]
                positions.push((cursor_line as u32, i as u32));
            }
        }
    } else {
        for i in (0..cursor_col).rev() {
            if chars.get(i) == Some(&target) {
                #[allow(clippy::cast_possible_truncation)]
                positions.push((cursor_line as u32, i as u32));
            }
        }
    }

    positions
}

impl CommandHandler for EnhancedFindCharCommand {
    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(target_char) = args.char("find_char") else {
            return CommandResult::error("find_char argument required");
        };

        let forward = args.string("find_direction") != Some("backward");

        let inclusive = match args.get("find_inclusive") {
            Some(ArgValue::Bool(b)) => *b,
            _ => true,
        };

        let count = args.count().unwrap_or(1);

        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let cursor_line = window.cursor.line;
        let cursor_col = window.cursor.column;

        // Scan all matches to detect multi-match case
        let matches = runtime.with_buffer_read(buffer_id, |buffer| {
            buffer.line(cursor_line).map_or_else(Vec::new, |line_text| {
                find_all_char_matches(line_text, cursor_col, cursor_line, target_char, forward)
            })
        });

        let Some(matches) = matches else {
            return CommandResult::error("Buffer not found");
        };

        // Multi-match with count=1: show jump labels
        if matches.len() > 1 && count == 1 {
            let labels = generate_labels(matches.len());
            let jump_matches: Vec<JumpMatch> = matches
                .iter()
                .zip(labels)
                .enumerate()
                .map(|(i, (&(line, col), label))| JumpMatch::new(line, col, label, i as u32))
                .collect();

            let jump = runtime.ext_mut::<JumpSessionState>();
            jump.start_with_matches(jump_matches);

            runtime.push_mode(jump_ids::JUMP_INPUT_MODE, TransitionContext::new());
            return CommandResult::Success;
        }

        // Single match or counted: use standard motion engine
        let cursor = Cursor::new(Position::new(cursor_line, cursor_col));
        let motion = Motion::FindChar {
            char: target_char,
            direction: if forward {
                reovim_kernel::api::v1::Direction::Forward
            } else {
                reovim_kernel::api::v1::Direction::Backward
            },
            till: !inclusive,
        };

        let target = runtime.with_buffer_read(buffer_id, |buffer| {
            MotionEngine::calculate(buffer, &cursor, motion, count)
        });

        let Some(target) = target else {
            return CommandResult::error("Buffer not found");
        };

        if let Some(pos) = target {
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = pos.into();
            }
            runtime.record_cursor_move(buffer_id);
        }

        CommandResult::Success
    }
}

#[cfg(test)]
#[path = "find_char_tests.rs"]
mod tests;
