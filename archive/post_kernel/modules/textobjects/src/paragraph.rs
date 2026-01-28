//! Paragraph text object commands.
//!
//! Implements text objects: `ip`, `ap`.
//!
//! These commands select paragraph regions (contiguous non-blank lines).

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

/// Execute a paragraph text object and return the range.
fn execute_paragraph_textobj(
    runtime: &SessionRuntime<'_>,
    args: &CommandContext,
    text_object: TextObject,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    let count = args.count().unwrap_or(1);

    // Calculate text object range using with_buffer_read callback
    let range_result = runtime.with_buffer_read(buffer_id, |buffer| {
        let pos = buffer.position();
        TextObjectEngine::range(buffer, pos, text_object, count)
    });

    let Some(range_result) = range_result else {
        return CommandResult::error("Buffer not found");
    };

    let Some((start, end)) = range_result else {
        return CommandResult::Success; // No-op if no paragraph found
    };

    // Kernel returns inclusive end, convert to exclusive
    let end_exclusive = Position::new(end.line, end.column + 1);

    // TODO(#394): Return operator range via different mechanism (escape hatch until API supports this)
    // Paragraph operations are linewise
    let _ = (start, end_exclusive); // Suppress unused warnings
    CommandResult::Success
}

// =============================================================================
// Inner Paragraph (ip)
// =============================================================================

/// Inner paragraph text object.
///
/// Selects contiguous non-blank lines.
#[derive(Debug, Clone, Copy, Default)]
pub struct InnerParagraph;

impl Command for InnerParagraph {
    fn id(&self) -> CommandId {
        ids::INNER_PARAGRAPH
    }

    fn description(&self) -> &'static str {
        "Inner paragraph text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of paragraphs",
        )]
    }
}

impl CommandHandler for InnerParagraph {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_paragraph_textobj(runtime, args, TextObject::InnerParagraph)
    }
}

// =============================================================================
// Around Paragraph (ap)
// =============================================================================

/// Around paragraph text object.
///
/// Selects contiguous non-blank lines plus trailing blank lines.
#[derive(Debug, Clone, Copy, Default)]
pub struct AroundParagraph;

impl Command for AroundParagraph {
    fn id(&self) -> CommandId {
        ids::AROUND_PARAGRAPH
    }

    fn description(&self) -> &'static str {
        "Around paragraph text object"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of paragraphs",
        )]
    }
}

impl CommandHandler for AroundParagraph {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_paragraph_textobj(runtime, args, TextObject::AParagraph)
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all paragraph text object commands as boxed trait objects.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(InnerParagraph), Box::new(AroundParagraph)]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use {super::*, crate::TEXTOBJECTS_MODULE};

    #[test]
    fn test_inner_paragraph_id() {
        let cmd = InnerParagraph;
        assert_eq!(cmd.id().module(), &TEXTOBJECTS_MODULE);
        assert_eq!(cmd.id().name(), "inner-paragraph");
    }

    #[test]
    fn test_around_paragraph_id() {
        let cmd = AroundParagraph;
        assert_eq!(cmd.id().name(), "around-paragraph");
    }

    #[test]
    fn test_all_commands_count() {
        let cmds = all_commands();
        assert_eq!(cmds.len(), 2);
    }
}
