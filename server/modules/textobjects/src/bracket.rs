//! Bracket text object commands.
//!
//! Implements text objects: `i(`, `a(`, `i[`, `a[`, `i{`, `a{`, `i<`, `a<`.
//!
//! These commands select text within or around bracket pairs.
//!
//! # Mode-Aware Behavior (Epic #465)
//!
//! Text objects behave differently based on the current mode:
//! - **Operator-pending mode** (d, y, c): Store range for operator consumption
//! - **Visual mode** (v, V, Ctrl-V): Update selection to cover the text object

use {
    reovim_domain_text::{Position, TextObject, TextObjectEngine},
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{
        OperatorPendingState, SessionRuntime, TextObjRange,
        api::{ExtensionApi, ModeApi, Selection, SelectionMode},
    },
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

// =============================================================================
// Helper functions
// =============================================================================

/// Check if the current mode is a visual mode.
fn is_visual_mode(runtime: &SessionRuntime<'_>) -> bool {
    let mode = runtime.current_mode();
    let mode_name = mode.name();
    mode_name == "visual" || mode_name == "visual-line" || mode_name == "visual-block"
}

/// Get the selection mode for the current visual mode.
fn visual_selection_mode(runtime: &SessionRuntime<'_>) -> SelectionMode {
    let mode = runtime.current_mode();
    match mode.name() {
        "visual-line" => SelectionMode::Line,
        "visual-block" => SelectionMode::Block,
        _ => SelectionMode::Character,
    }
}

/// Execute a bracket text object and store the range for operator consumption,
/// or update the selection if in visual mode.
#[cfg_attr(coverage_nightly, coverage(off))]
fn execute_bracket_textobj(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    text_object: TextObject,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    let count = args.count().unwrap_or(1);

    // Get cursor from per-client Window (#471)
    let Some(window) = runtime.windows().active() else {
        return CommandResult::error("No active window");
    };
    let pos = Position::new(window.cursor.line, window.cursor.column);

    // Calculate text object range using with_text_geometry callback
    let range_result = runtime.with_text_geometry(buffer_id, |buffer| {
        TextObjectEngine::range(buffer, pos, text_object, count)
    });

    let Some(range_result) = range_result else {
        return CommandResult::error("Buffer not found");
    };

    let Some((start, end)) = range_result else {
        return CommandResult::Success; // No-op if no matching brackets
    };

    // Kernel returns inclusive end, convert to exclusive
    let end_exclusive = Position::new(end.line, end.column + 1);

    // Check if we're in visual mode
    if is_visual_mode(runtime) {
        // Visual mode: update the selection to cover the text object
        let sel_mode = visual_selection_mode(runtime);
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = Some(Selection::new(start, end_exclusive, sel_mode));
            // Move cursor to end of selection
            window.cursor = end.into();
        }
    } else {
        // Operator-pending mode: store range for operator consumption (Epic #465)
        // The operator resolver's on_command_complete will take() this range
        let state = runtime.ext_mut::<OperatorPendingState>();
        state.set_textobj_range(TextObjRange::characterwise(start, end_exclusive));
    }

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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::InnerBracket('('))
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::ABracket('('))
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::InnerBracket('['))
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::ABracket('['))
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::InnerBracket('{'))
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::ABracket('{'))
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::InnerBracket('<'))
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_bracket_textobj(runtime, args, TextObject::ABracket('<'))
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
