//! LSP commands (with cursor context for navigation/hover)

use reovim_core::{
    command::{CommandResult, CommandTrait, ExecutionContext},
    event_bus::{DynEvent, Event},
};

// =============================================================================
// Events (carry context for handlers)
// =============================================================================

/// Event emitted when user triggers goto definition
#[derive(Debug, Clone)]
pub struct LspGotoDefinition {
    /// Buffer ID to navigate from
    pub buffer_id: usize,
    /// Cursor line (0-indexed)
    pub line: usize,
    /// Cursor column (0-indexed)
    pub column: usize,
}

impl Event for LspGotoDefinition {
    fn priority(&self) -> u32 {
        100
    }
}

/// Event emitted when user triggers goto references
#[derive(Debug, Clone)]
pub struct LspGotoReferences {
    /// Buffer ID to find references from
    pub buffer_id: usize,
    /// Cursor line (0-indexed)
    pub line: usize,
    /// Cursor column (0-indexed)
    pub column: usize,
}

impl Event for LspGotoReferences {
    fn priority(&self) -> u32 {
        100
    }
}

/// Event emitted when user triggers hover
#[derive(Debug, Clone)]
pub struct LspShowHover {
    /// Buffer ID to show hover for
    pub buffer_id: usize,
    /// Cursor line (0-indexed)
    pub line: usize,
    /// Cursor column (0-indexed)
    pub column: usize,
}

impl Event for LspShowHover {
    fn priority(&self) -> u32 {
        100
    }
}

// =============================================================================
// Commands (extract context from ExecutionContext, emit events)
// =============================================================================

/// Command: Go to definition of symbol under cursor
#[derive(Debug, Clone, Copy)]
pub struct LspGotoDefinitionCommand;

impl CommandTrait for LspGotoDefinitionCommand {
    fn name(&self) -> &'static str {
        "lsp_goto_definition"
    }

    fn description(&self) -> &'static str {
        "Go to definition of symbol under cursor"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        // Buffer position: y = line (row), x = column
        let line = ctx.buffer.cur.y as usize;
        let column = ctx.buffer.cur.x as usize;
        CommandResult::EmitEvent(DynEvent::new(LspGotoDefinition {
            buffer_id: ctx.buffer_id,
            line,
            column,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Command: Find all references to symbol under cursor
#[derive(Debug, Clone, Copy)]
pub struct LspGotoReferencesCommand;

impl CommandTrait for LspGotoReferencesCommand {
    fn name(&self) -> &'static str {
        "lsp_goto_references"
    }

    fn description(&self) -> &'static str {
        "Find all references to symbol under cursor"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        // Buffer position: y = line (row), x = column
        let line = ctx.buffer.cur.y as usize;
        let column = ctx.buffer.cur.x as usize;
        CommandResult::EmitEvent(DynEvent::new(LspGotoReferences {
            buffer_id: ctx.buffer_id,
            line,
            column,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Command: Show hover documentation for symbol under cursor
#[derive(Debug, Clone, Copy)]
pub struct LspShowHoverCommand;

impl CommandTrait for LspShowHoverCommand {
    fn name(&self) -> &'static str {
        "lsp_show_hover"
    }

    fn description(&self) -> &'static str {
        "Show hover documentation for symbol under cursor"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        // Buffer position: y = line (row), x = column
        let line = ctx.buffer.cur.y as usize;
        let column = ctx.buffer.cur.x as usize;
        CommandResult::EmitEvent(DynEvent::new(LspShowHover {
            buffer_id: ctx.buffer_id,
            line,
            column,
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
