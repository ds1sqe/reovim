//! Enhanced find-char command with multi-match jump labels.
//!
//! Overrides vim's `EXECUTE_FIND_CHAR` command when this adapter is loaded.
//! When multiple matches exist for f/F/t/T on a line (and count == 1),
//! activates jump labels via `JumpSessionState` instead of jumping to the
//! first match.
//!
//! # Architecture
//!
//! This command lives in the vim-range-finder adapter because it couples
//! vim's motion semantics with range-finder's jump state machine.
//! Neither module knows about the other — this adapter is the sole
//! coupling point.
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
    reovim_driver_command::{
        ArgValue, Command, CommandContext, CommandHandler, CommandPriority, CommandResult,
    },
    reovim_driver_session::{
        ExtensionApi, ModeApi, SessionRuntime, TransitionContext, api::ChangeTracker,
    },
    reovim_kernel::api::v1::{CommandId, Cursor, ModuleId, Motion, MotionEngine, Position},
    reovim_module_range_finder::jump::{
        ids as jump_ids,
        search::{JumpMatch, generate_labels},
        state::JumpSessionState,
    },
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
            Some(ArgValue::Bang(b)) => *b,
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
mod tests {
    use super::*;

    // ========================================================================
    // find_all_char_matches tests
    // ========================================================================

    #[test]
    fn test_find_all_forward_multiple() {
        let matches = find_all_char_matches("ababa", 0, 0, 'a', true);
        assert_eq!(matches, vec![(0, 2), (0, 4)]);
    }

    #[test]
    fn test_find_all_forward_single() {
        let matches = find_all_char_matches("abcde", 0, 0, 'c', true);
        assert_eq!(matches, vec![(0, 2)]);
    }

    #[test]
    fn test_find_all_forward_none() {
        let matches = find_all_char_matches("abcde", 0, 0, 'z', true);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_find_all_backward_multiple() {
        let matches = find_all_char_matches("ababa", 4, 0, 'a', false);
        assert_eq!(matches, vec![(0, 2), (0, 0)]);
    }

    #[test]
    fn test_find_all_backward_none() {
        let matches = find_all_char_matches("abcde", 0, 0, 'a', false);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_find_all_preserves_line() {
        let matches = find_all_char_matches("aa", 0, 5, 'a', true);
        assert_eq!(matches, vec![(5, 1)]);
    }

    #[test]
    fn test_find_all_empty_line() {
        let matches = find_all_char_matches("", 0, 0, 'a', true);
        assert!(matches.is_empty());
    }

    // ========================================================================
    // Command metadata tests
    // ========================================================================

    #[test]
    fn test_command_id() {
        let cmd = EnhancedFindCharCommand;
        assert_eq!(cmd.id().module().as_str(), "vim");
        assert_eq!(cmd.id().name(), "execute-find-char");
    }

    #[test]
    fn test_command_description() {
        let cmd = EnhancedFindCharCommand;
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn test_command_priority_is_override() {
        let cmd = EnhancedFindCharCommand;
        assert_eq!(cmd.priority(), CommandPriority::Override);
    }

    // ========================================================================
    // Execute tests using TestSessionRuntime
    // ========================================================================

    use reovim_driver_session::{TextInputSink, testing::TestSessionRuntime};

    #[test]
    fn test_execute_no_find_char_arg() {
        let cmd = EnhancedFindCharCommand;
        let mut harness = TestSessionRuntime::new();
        let args = CommandContext::new();
        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[test]
    fn test_execute_no_buffer() {
        let cmd = EnhancedFindCharCommand;
        let mut harness = TestSessionRuntime::new();
        let mut args = CommandContext::new();
        args.set("find_char", ArgValue::Char('x'));
        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[test]
    fn test_execute_forward_single_match() {
        let cmd = EnhancedFindCharCommand;
        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let buffer_id = harness.active_buffer().unwrap();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('w'));

        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert_eq!(result, CommandResult::Success);

        harness.with_runtime(|rt| {
            let window = rt.windows().active().unwrap();
            assert_eq!(window.cursor.column, 6);
        });
    }

    #[test]
    fn test_execute_forward_multi_match_activates_jump() {
        let cmd = EnhancedFindCharCommand;
        let mut harness = TestSessionRuntime::with_buffer("ababa");
        let buffer_id = harness.active_buffer().unwrap();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('a'));

        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert_eq!(result, CommandResult::Success);

        // Jump state should be active with 2 matches
        harness.with_runtime(|rt| {
            let jump = rt.ext_mut::<JumpSessionState>();
            assert!(jump.is_active());
            let m = jump.get_matches().unwrap();
            assert_eq!(m.len(), 2);
        });

        // Mode should have been pushed
        assert!(harness.changes().mode_changed);
    }

    #[test]
    fn test_execute_multi_match_with_count_uses_motion() {
        let cmd = EnhancedFindCharCommand;
        let mut harness = TestSessionRuntime::with_buffer("ababa");
        let buffer_id = harness.active_buffer().unwrap();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('a'));
        args.set("count", ArgValue::Count(2));

        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert_eq!(result, CommandResult::Success);

        // With count=2, should use direct motion to second 'a' at col 4
        harness.with_runtime(|rt| {
            let window = rt.windows().active().unwrap();
            assert_eq!(window.cursor.column, 4);
        });
    }

    #[test]
    fn test_execute_not_found() {
        let cmd = EnhancedFindCharCommand;
        let mut harness = TestSessionRuntime::with_buffer("hello");
        let buffer_id = harness.active_buffer().unwrap();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('z'));

        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert_eq!(result, CommandResult::Success);

        // Cursor doesn't move
        harness.with_runtime(|rt| {
            let window = rt.windows().active().unwrap();
            assert_eq!(window.cursor.column, 0);
        });
    }

    #[test]
    fn test_execute_backward() {
        let cmd = EnhancedFindCharCommand;
        // Use string with single 'h' to test basic backward motion
        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let buffer_id = harness.active_buffer().unwrap();

        harness.with_runtime(|rt| {
            let window = rt.windows_mut().active_mut().unwrap();
            window.cursor.column = 5;
        });

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('h'));
        args.set("find_direction", ArgValue::String("backward".to_string()));

        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert_eq!(result, CommandResult::Success);

        harness.with_runtime(|rt| {
            let window = rt.windows().active().unwrap();
            assert_eq!(window.cursor.column, 0); // 'h' at col 0
        });
    }

    #[test]
    fn test_execute_till_mode() {
        let cmd = EnhancedFindCharCommand;
        // Use 'w' (single match) to test till mode
        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let buffer_id = harness.active_buffer().unwrap();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('w'));
        args.set("find_inclusive", ArgValue::Bang(false));

        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert_eq!(result, CommandResult::Success);

        harness.with_runtime(|rt| {
            let window = rt.windows().active().unwrap();
            assert_eq!(window.cursor.column, 5); // One before 'w' at col 6
        });
    }

    #[test]
    fn test_execute_empty_buffer() {
        let cmd = EnhancedFindCharCommand;
        let mut harness = TestSessionRuntime::with_buffer("");
        let buffer_id = harness.active_buffer().unwrap();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('x'));

        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert_eq!(result, CommandResult::Success);

        harness.with_runtime(|rt| {
            let window = rt.windows().active().unwrap();
            assert_eq!(window.cursor.column, 0);
        });
    }

    #[test]
    fn test_execute_multi_match_label_selection_moves_cursor() {
        let cmd = EnhancedFindCharCommand;
        let mut harness = TestSessionRuntime::with_buffer("ababa");
        let buffer_id = harness.active_buffer().unwrap();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('a'));

        // Start enhanced find-char, should activate jump labels
        harness.with_runtime(|rt| cmd.execute(rt, &args));

        // Select second label ('f') to jump to col 4
        harness.with_runtime(|rt| {
            let jump = rt.ext_mut::<JumpSessionState>();
            jump.insert_char('f');
            assert!(jump.has_target());
        });

        // Execute jump
        let exec_cmd = reovim_module_range_finder::jump::command::JumpExecuteCommand;
        let exec_args = CommandContext::new();
        harness.with_runtime(|rt| exec_cmd.execute(rt, &exec_args));

        harness.with_runtime(|rt| {
            let window = rt.windows().active().unwrap();
            assert_eq!(window.cursor.column, 4);
        });
    }
}
