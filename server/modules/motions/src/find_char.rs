//! Find-char motion commands.
//!
//! Implements vim find-char motions: `f`, `F`, `t`, `T`, `;`, `,`.
//!
//! # Architecture (#563, Epic #385)
//!
//! ## Coordinator + Execution Split
//!
//! `DISPATCH_FIND_CHAR` (coordinator) records `FindCharState` for repeat,
//! then delegates to `EXECUTE_FIND_CHAR` (execution). Providers override
//! `EXECUTE_FIND_CHAR` only — they never touch repeat state.
//!
//! ## Stub Commands (f/F/t/T)
//!
//! These are **intercepted by the vim resolver** before execution.
//! They exist only for keybinding registration and command metadata.
//!
//! ## Repeat Handlers (;/,)
//!
//! `RepeatFindSame` and `RepeatFindReverse` read `FindCharState` and
//! delegate to `EXECUTE_FIND_CHAR` with stored parameters.

use {
    reovim_driver_command::{ArgValue, Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
            FindCharState, SessionRuntime,
        api::{CommandApi, ExtensionApi},
    },
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

use crate::ids;

/// The execution command ID for find-char (defined by vim module).
/// Constructed locally to avoid compile-time dependency on vim module.
const EXECUTE_FIND_CHAR: CommandId = CommandId::new(ModuleId::new("vim"), "execute-find-char");

// =============================================================================
// Dispatch Find Char (coordinator) (#563)
// =============================================================================

/// Coordinator command for find-char motions.
///
/// Records `FindCharState` for `;`/`,` repeat, then delegates to
/// `EXECUTE_FIND_CHAR`. This ensures repeat state is always recorded
/// regardless of which provider handles execution.
///
/// WARNING: Do NOT override this command. Overriding would break repeat
/// recording for all providers. Override `EXECUTE_FIND_CHAR` instead.
#[derive(Debug, Clone, Copy, Default)]
pub struct DispatchFindChar;

impl Command for DispatchFindChar {
    fn id(&self) -> CommandId {
        ids::DISPATCH_FIND_CHAR
    }

    fn description(&self) -> &'static str {
        "Record find-char state and delegate to execute-find-char"
    }
}

impl CommandHandler for DispatchFindChar {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(target_char) = args.char("find_char") else {
            return CommandResult::error("find_char argument required");
        };
        let forward = args.string("find_direction") != Some("backward");
        let inclusive = match args.get("find_inclusive") {
            Some(ArgValue::Bang(b)) => *b,
            _ => true,
        };

        // Record state for ;/, repeat
        runtime
            .ext_mut::<FindCharState>()
            .record(target_char, forward, inclusive);

        // Delegate to EXECUTE_FIND_CHAR (same context)
        runtime.execute_command(EXECUTE_FIND_CHAR, args.clone())
    }
}

// =============================================================================
// Find Char Forward (f)
// =============================================================================

/// Find character forward - cursor lands on the character.
///
/// Press `f` followed by a character to move cursor to next occurrence
/// of that character on the current line.
///
/// Note: This command is intercepted by the vim resolver. The `execute()`
/// method should never be called in normal operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct FindCharForward;

impl Command for FindCharForward {
    fn id(&self) -> CommandId {
        ids::FIND_CHAR_FORWARD
    }

    fn description(&self) -> &'static str {
        "Find character forward (f)"
    }
}

impl CommandHandler for FindCharForward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Intercepted by vim resolver - should not reach here
        CommandResult::Success
    }
}

// =============================================================================
// Find Char Backward (F)
// =============================================================================

/// Find character backward - cursor lands on the character.
///
/// Press `F` followed by a character to move cursor to previous occurrence
/// of that character on the current line.
#[derive(Debug, Clone, Copy, Default)]
pub struct FindCharBackward;

impl Command for FindCharBackward {
    fn id(&self) -> CommandId {
        ids::FIND_CHAR_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Find character backward (F)"
    }
}

impl CommandHandler for FindCharBackward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Intercepted by vim resolver - should not reach here
        CommandResult::Success
    }
}

// =============================================================================
// Till Char Forward (t)
// =============================================================================

/// Till character forward - cursor stops before the character.
///
/// Press `t` followed by a character to move cursor to position just
/// before the next occurrence of that character on the current line.
#[derive(Debug, Clone, Copy, Default)]
pub struct TillCharForward;

impl Command for TillCharForward {
    fn id(&self) -> CommandId {
        ids::TILL_CHAR_FORWARD
    }

    fn description(&self) -> &'static str {
        "Till character forward (t)"
    }
}

impl CommandHandler for TillCharForward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Intercepted by vim resolver - should not reach here
        CommandResult::Success
    }
}

// =============================================================================
// Till Char Backward (T)
// =============================================================================

/// Till character backward - cursor stops after the character.
///
/// Press `T` followed by a character to move cursor to position just
/// after the previous occurrence of that character on the current line.
#[derive(Debug, Clone, Copy, Default)]
pub struct TillCharBackward;

impl Command for TillCharBackward {
    fn id(&self) -> CommandId {
        ids::TILL_CHAR_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Till character backward (T)"
    }
}

impl CommandHandler for TillCharBackward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Intercepted by vim resolver - should not reach here
        CommandResult::Success
    }
}

// =============================================================================
// Repeat Find Same (;)
// =============================================================================

/// Repeat last find-char in the same direction.
///
/// After using `f`, `F`, `t`, or `T`, press `;` to repeat that motion
/// in the same direction. Reads `FindCharState` and delegates to
/// `EXECUTE_FIND_CHAR` with stored parameters.
#[derive(Debug, Clone, Copy, Default)]
pub struct RepeatFindSame;

impl Command for RepeatFindSame {
    fn id(&self) -> CommandId {
        ids::REPEAT_FIND_SAME
    }

    fn description(&self) -> &'static str {
        "Repeat last find in same direction (;)"
    }
}

impl CommandHandler for RepeatFindSame {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(record) = runtime.ext_mut::<FindCharState>().last().copied() else {
            return CommandResult::error("No previous find-char");
        };

        let mut ctx = args.clone();
        ctx.set("find_char", ArgValue::Char(record.char()));
        ctx.set(
            "find_direction",
            ArgValue::String(
                if record.forward() {
                    "forward"
                } else {
                    "backward"
                }
                .to_string(),
            ),
        );
        ctx.set("find_inclusive", ArgValue::Bang(record.inclusive()));

        runtime.execute_command(EXECUTE_FIND_CHAR, ctx)
    }
}

// =============================================================================
// Repeat Find Reverse (,)
// =============================================================================

/// Repeat last find-char in the opposite direction.
///
/// After using `f`, `F`, `t`, or `T`, press `,` to repeat that motion
/// in the opposite direction. Reads `FindCharState`, reverses direction,
/// and delegates to `EXECUTE_FIND_CHAR`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RepeatFindReverse;

impl Command for RepeatFindReverse {
    fn id(&self) -> CommandId {
        ids::REPEAT_FIND_REVERSE
    }

    fn description(&self) -> &'static str {
        "Repeat last find in opposite direction (,)"
    }
}

impl CommandHandler for RepeatFindReverse {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(record) = runtime.ext_mut::<FindCharState>().last().copied() else {
            return CommandResult::error("No previous find-char");
        };
        let reversed = record.reversed();

        let mut ctx = args.clone();
        ctx.set("find_char", ArgValue::Char(reversed.char()));
        ctx.set(
            "find_direction",
            ArgValue::String(
                if reversed.forward() {
                    "forward"
                } else {
                    "backward"
                }
                .to_string(),
            ),
        );
        ctx.set("find_inclusive", ArgValue::Bang(reversed.inclusive()));

        runtime.execute_command(EXECUTE_FIND_CHAR, ctx)
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Get all find-char motion commands.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(DispatchFindChar),
        Box::new(FindCharForward),
        Box::new(FindCharBackward),
        Box::new(TillCharForward),
        Box::new(TillCharBackward),
        Box::new(RepeatFindSame),
        Box::new(RepeatFindReverse),
    ]
}

// =============================================================================
// Tests
// =============================================================================

