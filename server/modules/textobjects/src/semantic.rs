//! Treesitter-based semantic text object commands.
//!
//! Provides 14 text objects (7 kinds x inner/around) that use the syntax
//! driver's `textobject_range()` to resolve language-aware constructs:
//!
//! | Kind        | Inner | Around | Keybinding |
//! |-------------|-------|--------|------------|
//! | Function    | if    | af     | if / af    |
//! | Class       | ic    | ac     | ic / ac    |
//! | Argument    | ia    | aa     | ia / aa    |
//! | Conditional | io    | ao     | io / ao    |
//! | Loop        | il    | al     | il / al    |
//! | Comment     | i/    | a/     | i/ / a/    |
//! | Block (TS)  | --    | --     | deferred   |

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        OperatorPendingState, SessionRuntime, TextObjRange,
        api::{ExtensionApi, ModeApi, Selection, SelectionMode},
    },
    reovim_driver_syntax::{SyntaxSessionState, TextObjectKind, TextObjectScope},
    reovim_kernel::api::v1::CommandId,
    reovim_types_text::Position,
};

use crate::ids;

/// Execute a semantic text object query using the syntax driver.
///
/// 1. Gets cursor position from active window
/// 2. Looks up `SyntaxSessionState` for the current buffer
/// 3. Calls `textobject_range()` on the driver
/// 4. Stores the result in `OperatorPendingState` (operator-pending mode)
///    or updates the selection (visual mode)
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::cast_possible_truncation)]
fn execute_semantic_textobj(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    kind: TextObjectKind,
    scope: TextObjectScope,
    linewise: bool,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::Success;
    };
    let Some(window) = runtime.windows().active() else {
        return CommandResult::Success;
    };
    let line = window.cursor.line as u32;
    let col = window.cursor.column as u32;

    // Query syntax driver for textobject range (immutable borrow)
    let range = runtime
        .ext::<SyntaxSessionState>()
        .and_then(|s| s.get(buffer_id))
        .and_then(|driver| driver.textobject_range(kind, scope, line, col));

    let Some(range) = range else {
        return CommandResult::Success;
    };

    let start = Position::new(range.start_row as usize, range.start_col as usize);
    let end_exclusive = Position::new(range.end_row as usize, range.end_col as usize);

    if is_visual_mode(runtime) {
        let sel_mode = visual_selection_mode(runtime);
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = Some(Selection::new(start, end_exclusive, sel_mode));
            // Move cursor to last character (inclusive end)
            window.cursor.line = range.end_row as usize;
            window.cursor.column = range.end_col.saturating_sub(1) as usize;
        }
    } else {
        let state = runtime.ext_mut::<OperatorPendingState>();
        if linewise {
            state.set_textobj_range(TextObjRange::linewise(start, end_exclusive));
        } else {
            state.set_textobj_range(TextObjRange::characterwise(start, end_exclusive));
        }
    }

    CommandResult::Success
}

fn is_visual_mode(runtime: &SessionRuntime<'_>) -> bool {
    let mode = runtime.current_mode();
    let name = mode.name();
    name == "visual" || name == "visual-line" || name == "visual-block"
}

fn visual_selection_mode(runtime: &SessionRuntime<'_>) -> SelectionMode {
    let mode = runtime.current_mode();
    match mode.name() {
        "visual-line" => SelectionMode::Line,
        "visual-block" => SelectionMode::Block,
        _ => SelectionMode::Character,
    }
}

// =============================================================================
// Command handler definitions (7 kinds x 2 scopes = 14 commands)
// =============================================================================

macro_rules! semantic_command {
    ($name:ident, $id:expr, $desc:expr, $kind:expr, $scope:expr, $linewise:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl Command for $name {
            fn id(&self) -> CommandId {
                $id
            }

            fn description(&self) -> &'static str {
                $desc
            }
        }

        impl CommandHandler for $name {
            fn execute(
                &self,
                runtime: &mut SessionRuntime<'_>,
                args: &CommandContext,
            ) -> CommandResult {
                execute_semantic_textobj(runtime, args, $kind, $scope, $linewise)
            }
        }
    };
}

// Function (linewise)
semantic_command!(
    InnerFunction,
    ids::INNER_FUNCTION,
    "Inner function text object (if)",
    TextObjectKind::Function,
    TextObjectScope::Inner,
    true
);
semantic_command!(
    AroundFunction,
    ids::AROUND_FUNCTION,
    "Around function text object (af)",
    TextObjectKind::Function,
    TextObjectScope::Outer,
    true
);

// Class (linewise)
semantic_command!(
    InnerClass,
    ids::INNER_CLASS,
    "Inner class text object (ic)",
    TextObjectKind::Class,
    TextObjectScope::Inner,
    true
);
semantic_command!(
    AroundClass,
    ids::AROUND_CLASS,
    "Around class text object (ac)",
    TextObjectKind::Class,
    TextObjectScope::Outer,
    true
);

// Argument (characterwise)
semantic_command!(
    InnerArgument,
    ids::INNER_ARGUMENT,
    "Inner argument text object (ia)",
    TextObjectKind::Argument,
    TextObjectScope::Inner,
    false
);
semantic_command!(
    AroundArgument,
    ids::AROUND_ARGUMENT,
    "Around argument text object (aa)",
    TextObjectKind::Argument,
    TextObjectScope::Outer,
    false
);

// Conditional (linewise)
semantic_command!(
    InnerConditional,
    ids::INNER_CONDITIONAL,
    "Inner conditional text object (io)",
    TextObjectKind::Conditional,
    TextObjectScope::Inner,
    true
);
semantic_command!(
    AroundConditional,
    ids::AROUND_CONDITIONAL,
    "Around conditional text object (ao)",
    TextObjectKind::Conditional,
    TextObjectScope::Outer,
    true
);

// Loop (linewise)
semantic_command!(
    InnerLoop,
    ids::INNER_LOOP,
    "Inner loop text object (il)",
    TextObjectKind::Loop,
    TextObjectScope::Inner,
    true
);
semantic_command!(
    AroundLoop,
    ids::AROUND_LOOP,
    "Around loop text object (al)",
    TextObjectKind::Loop,
    TextObjectScope::Outer,
    true
);

// Comment (characterwise)
semantic_command!(
    InnerComment,
    ids::INNER_COMMENT,
    "Inner comment text object (i/)",
    TextObjectKind::Comment,
    TextObjectScope::Inner,
    false
);
semantic_command!(
    AroundComment,
    ids::AROUND_COMMENT,
    "Around comment text object (a/)",
    TextObjectKind::Comment,
    TextObjectScope::Outer,
    false
);

// Block (linewise, treesitter-based)
semantic_command!(
    InnerBlockTs,
    ids::INNER_BLOCK_TS,
    "Inner block text object (treesitter)",
    TextObjectKind::Block,
    TextObjectScope::Inner,
    true
);
semantic_command!(
    AroundBlockTs,
    ids::AROUND_BLOCK_TS,
    "Around block text object (treesitter)",
    TextObjectKind::Block,
    TextObjectScope::Outer,
    true
);

/// Return all semantic text object command handlers.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(InnerFunction),
        Box::new(AroundFunction),
        Box::new(InnerClass),
        Box::new(AroundClass),
        Box::new(InnerArgument),
        Box::new(AroundArgument),
        Box::new(InnerConditional),
        Box::new(AroundConditional),
        Box::new(InnerLoop),
        Box::new(AroundLoop),
        Box::new(InnerComment),
        Box::new(AroundComment),
        Box::new(InnerBlockTs),
        Box::new(AroundBlockTs),
    ]
}
