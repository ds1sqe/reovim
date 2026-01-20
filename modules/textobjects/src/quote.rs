//! Quote text object commands.
//!
//! Implements text objects: `i"`, `a"`, `i'`, `a'`, `` i` ``, `` a` ``.
//!
//! These commands select text within or around quote characters.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::{CommandId, Position, TextObject, TextObjectEngine},
};

use crate::ids;

// =============================================================================
// Helper function
// =============================================================================

/// Execute a quote text object and return the range.
#[allow(clippy::significant_drop_tightening)]
fn execute_quote_textobj(
    runtime: &SessionRuntime<'_>,
    args: &CommandContext,
    text_object: TextObject,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    let Some(buffer_arc) = runtime.kernel().buffers.get(buffer_id) else {
        return CommandResult::error("Buffer not found");
    };

    let count = args.count().unwrap_or(1);

    // Hold the buffer lock only for the duration needed
    let range_result = {
        let buffer = buffer_arc.read();
        let pos = buffer.position();
        TextObjectEngine::range(&buffer, pos, text_object, count)
    };

    let Some((start, end)) = range_result else {
        return CommandResult::Success; // No-op if no matching quotes
    };

    // Kernel returns inclusive end, convert to exclusive
    let end_exclusive = Position::new(end.line, end.column + 1);

    // TODO(#394): Return operator range via different mechanism (escape hatch until API supports this)
    let _ = (start, end_exclusive); // Suppress unused warnings
    CommandResult::Success
}

// =============================================================================
// Inner Double Quote (i")
// =============================================================================

/// Inner double quote text object.
///
/// Selects text inside double quotes, excluding the quotes.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerDoubleQuote;

impl Command for InnerDoubleQuote {
    fn id(&self) -> CommandId {
        ids::INNER_DOUBLE_QUOTE
    }

    fn description(&self) -> &'static str {
        "Inner double quote text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Quote nesting level",
        )]
    }
}

impl CommandHandler for InnerDoubleQuote {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_quote_textobj(runtime, args, TextObject::InnerQuote('"'))
    }
}

// =============================================================================
// Around Double Quote (a")
// =============================================================================

/// Around double quote text object.
///
/// Selects text including the double quotes.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundDoubleQuote;

impl Command for AroundDoubleQuote {
    fn id(&self) -> CommandId {
        ids::AROUND_DOUBLE_QUOTE
    }

    fn description(&self) -> &'static str {
        "Around double quote text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Quote nesting level",
        )]
    }
}

impl CommandHandler for AroundDoubleQuote {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_quote_textobj(runtime, args, TextObject::AQuote('"'))
    }
}

// =============================================================================
// Inner Single Quote (i')
// =============================================================================

/// Inner single quote text object.
///
/// Selects text inside single quotes, excluding the quotes.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerSingleQuote;

impl Command for InnerSingleQuote {
    fn id(&self) -> CommandId {
        ids::INNER_SINGLE_QUOTE
    }

    fn description(&self) -> &'static str {
        "Inner single quote text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Quote nesting level",
        )]
    }
}

impl CommandHandler for InnerSingleQuote {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_quote_textobj(runtime, args, TextObject::InnerQuote('\''))
    }
}

// =============================================================================
// Around Single Quote (a')
// =============================================================================

/// Around single quote text object.
///
/// Selects text including the single quotes.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundSingleQuote;

impl Command for AroundSingleQuote {
    fn id(&self) -> CommandId {
        ids::AROUND_SINGLE_QUOTE
    }

    fn description(&self) -> &'static str {
        "Around single quote text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Quote nesting level",
        )]
    }
}

impl CommandHandler for AroundSingleQuote {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_quote_textobj(runtime, args, TextObject::AQuote('\''))
    }
}

// =============================================================================
// Inner Backtick (i`)
// =============================================================================

/// Inner backtick text object.
///
/// Selects text inside backticks, excluding the backticks.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerBacktick;

impl Command for InnerBacktick {
    fn id(&self) -> CommandId {
        ids::INNER_BACKTICK
    }

    fn description(&self) -> &'static str {
        "Inner backtick text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Quote nesting level",
        )]
    }
}

impl CommandHandler for InnerBacktick {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_quote_textobj(runtime, args, TextObject::InnerQuote('`'))
    }
}

// =============================================================================
// Around Backtick (a`)
// =============================================================================

/// Around backtick text object.
///
/// Selects text including the backticks.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundBacktick;

impl Command for AroundBacktick {
    fn id(&self) -> CommandId {
        ids::AROUND_BACKTICK
    }

    fn description(&self) -> &'static str {
        "Around backtick text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Quote nesting level",
        )]
    }
}

impl CommandHandler for AroundBacktick {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_quote_textobj(runtime, args, TextObject::AQuote('`'))
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all quote text object commands as boxed trait objects.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(InnerDoubleQuote),
        Box::new(AroundDoubleQuote),
        Box::new(InnerSingleQuote),
        Box::new(AroundSingleQuote),
        Box::new(InnerBacktick),
        Box::new(AroundBacktick),
    ]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use {super::*, crate::TEXTOBJECTS_MODULE};

    #[test]
    fn test_inner_double_quote_id() {
        let cmd = InnerDoubleQuote;
        assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
        assert_eq!(cmd.id().name(), "inner-double-quote");
    }

    #[test]
    fn test_around_double_quote_id() {
        let cmd = AroundDoubleQuote;
        assert_eq!(cmd.id().name(), "around-double-quote");
    }

    #[test]
    fn test_inner_single_quote_id() {
        let cmd = InnerSingleQuote;
        assert_eq!(cmd.id().name(), "inner-single-quote");
    }

    #[test]
    fn test_around_single_quote_id() {
        let cmd = AroundSingleQuote;
        assert_eq!(cmd.id().name(), "around-single-quote");
    }

    #[test]
    fn test_inner_backtick_id() {
        let cmd = InnerBacktick;
        assert_eq!(cmd.id().name(), "inner-backtick");
    }

    #[test]
    fn test_around_backtick_id() {
        let cmd = AroundBacktick;
        assert_eq!(cmd.id().name(), "around-backtick");
    }

    #[test]
    fn test_all_commands_count() {
        let cmds = all_commands();
        assert_eq!(cmds.len(), 6);
    }
}
