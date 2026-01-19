//! Bracket text object commands.
//!
//! Implements text objects: `i(`, `a(`, `i[`, `a[`, `i{`, `a{`, `i<`, `a<`.
//!
//! These commands select text within or around bracket pairs.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_kernel::api::v1::{CommandId, KernelContext, Position, TextObject, TextObjectEngine},
};

use crate::ids;

// =============================================================================
// Helper function
// =============================================================================

/// Execute a bracket text object and return the range.
#[allow(clippy::significant_drop_tightening)]
fn execute_bracket_textobj(
    ctx: &KernelContext,
    args: &CommandContext,
    text_object: TextObject,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
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
        return CommandResult::Success; // No-op if no matching brackets
    };

    // Kernel returns inclusive end, convert to exclusive
    let end_exclusive = Position::new(end.line, end.column + 1);

    // TODO: Return operator range via different mechanism when SessionContext is available
    // For now, store range somewhere the operator can access
    let _ = (start, end_exclusive); // Suppress unused warnings
    CommandResult::Success
}

// =============================================================================
// Inner Parenthesis (i(, i), ib)
// =============================================================================

/// Inner parenthesis text object.
///
/// Selects text inside parentheses, excluding the parens.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerParen;

impl Command for InnerParen {
    fn id(&self) -> CommandId {
        ids::INNER_PAREN
    }

    fn description(&self) -> &'static str {
        "Inner parenthesis text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for InnerParen {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(ctx, args, TextObject::InnerBracket('('))
    }
}

// =============================================================================
// Around Parenthesis (a(, a), ab)
// =============================================================================

/// Around parenthesis text object.
///
/// Selects text including the parentheses.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundParen;

impl Command for AroundParen {
    fn id(&self) -> CommandId {
        ids::AROUND_PAREN
    }

    fn description(&self) -> &'static str {
        "Around parenthesis text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for AroundParen {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(ctx, args, TextObject::ABracket('('))
    }
}

// =============================================================================
// Inner Square Bracket (i[, i])
// =============================================================================

/// Inner square bracket text object.
///
/// Selects text inside square brackets, excluding the brackets.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerSquareBracket;

impl Command for InnerSquareBracket {
    fn id(&self) -> CommandId {
        ids::INNER_BRACKET
    }

    fn description(&self) -> &'static str {
        "Inner square bracket text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for InnerSquareBracket {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(ctx, args, TextObject::InnerBracket('['))
    }
}

// =============================================================================
// Around Square Bracket (a[, a])
// =============================================================================

/// Around square bracket text object.
///
/// Selects text including the square brackets.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundSquareBracket;

impl Command for AroundSquareBracket {
    fn id(&self) -> CommandId {
        ids::AROUND_BRACKET
    }

    fn description(&self) -> &'static str {
        "Around square bracket text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for AroundSquareBracket {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(ctx, args, TextObject::ABracket('['))
    }
}

// =============================================================================
// Inner Brace (i{, i}, iB)
// =============================================================================

/// Inner brace text object.
///
/// Selects text inside braces, excluding the braces.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerBrace;

impl Command for InnerBrace {
    fn id(&self) -> CommandId {
        ids::INNER_BRACE
    }

    fn description(&self) -> &'static str {
        "Inner brace text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for InnerBrace {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(ctx, args, TextObject::InnerBracket('{'))
    }
}

// =============================================================================
// Around Brace (a{, a}, aB)
// =============================================================================

/// Around brace text object.
///
/// Selects text including the braces.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundBrace;

impl Command for AroundBrace {
    fn id(&self) -> CommandId {
        ids::AROUND_BRACE
    }

    fn description(&self) -> &'static str {
        "Around brace text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for AroundBrace {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(ctx, args, TextObject::ABracket('{'))
    }
}

// =============================================================================
// Inner Angle Bracket (i<, i>)
// =============================================================================

/// Inner angle bracket text object.
///
/// Selects text inside angle brackets, excluding the brackets.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerAngle;

impl Command for InnerAngle {
    fn id(&self) -> CommandId {
        ids::INNER_ANGLE
    }

    fn description(&self) -> &'static str {
        "Inner angle bracket text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for InnerAngle {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(ctx, args, TextObject::InnerBracket('<'))
    }
}

// =============================================================================
// Around Angle Bracket (a<, a>)
// =============================================================================

/// Around angle bracket text object.
///
/// Selects text including the angle brackets.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundAngle;

impl Command for AroundAngle {
    fn id(&self) -> CommandId {
        ids::AROUND_ANGLE
    }

    fn description(&self) -> &'static str {
        "Around angle bracket text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("count", ArgKind::Count, "Nesting level")]
    }
}

impl CommandHandler for AroundAngle {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(ctx, args, TextObject::ABracket('<'))
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all bracket text object commands as boxed trait objects.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(InnerParen),
        Box::new(AroundParen),
        Box::new(InnerSquareBracket),
        Box::new(AroundSquareBracket),
        Box::new(InnerBrace),
        Box::new(AroundBrace),
        Box::new(InnerAngle),
        Box::new(AroundAngle),
    ]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use {super::*, crate::TEXTOBJECTS_MODULE};

    #[test]
    fn test_inner_paren_id() {
        let cmd = InnerParen;
        assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
        assert_eq!(cmd.id().name(), "inner-paren");
    }

    #[test]
    fn test_around_paren_id() {
        let cmd = AroundParen;
        assert_eq!(cmd.id().name(), "around-paren");
    }

    #[test]
    fn test_inner_bracket_id() {
        let cmd = InnerSquareBracket;
        assert_eq!(cmd.id().name(), "inner-bracket");
    }

    #[test]
    fn test_around_bracket_id() {
        let cmd = AroundSquareBracket;
        assert_eq!(cmd.id().name(), "around-bracket");
    }

    #[test]
    fn test_inner_brace_id() {
        let cmd = InnerBrace;
        assert_eq!(cmd.id().name(), "inner-brace");
    }

    #[test]
    fn test_around_brace_id() {
        let cmd = AroundBrace;
        assert_eq!(cmd.id().name(), "around-brace");
    }

    #[test]
    fn test_inner_angle_id() {
        let cmd = InnerAngle;
        assert_eq!(cmd.id().name(), "inner-angle");
    }

    #[test]
    fn test_around_angle_id() {
        let cmd = AroundAngle;
        assert_eq!(cmd.id().name(), "around-angle");
    }

    #[test]
    fn test_all_commands_count() {
        let cmds = all_commands();
        assert_eq!(cmds.len(), 8);
    }
}
