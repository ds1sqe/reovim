//! Quote text object commands.
//!
//! Implements text objects: `i"`, `a"`, `i'`, `a'`, `` i` ``, `` a` ``.
//!
//! These commands select text within or around quote characters.
//!
//! # Mode-Aware Behavior (Epic #465)
//!
//! Text objects behave differently based on the current mode:
//! - **Operator-pending mode** (d, y, c): Store range for operator consumption
//! - **Visual mode** (v, V, Ctrl-V): Update selection to cover the text object

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{
            OperatorPendingState, SessionRuntime, TextObjRange,
        api::{ExtensionApi, ModeApi, Selection, SelectionMode},
    },
    reovim_kernel::api::v1::{CommandId, Position, TextObject, TextObjectEngine},
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

/// Execute a quote text object and store the range for operator consumption,
/// or update the selection if in visual mode.
#[cfg_attr(coverage_nightly, coverage(off))]
fn execute_quote_textobj(
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

    // Calculate text object range using with_buffer_read callback
    let range_result = runtime.with_buffer_read(buffer_id, |buffer| {
        TextObjectEngine::range(buffer, pos, text_object, count)
    });

    let Some(range_result) = range_result else {
        return CommandResult::error("Buffer not found");
    };

    let Some((start, end)) = range_result else {
        return CommandResult::Success; // No-op if no matching quotes
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

