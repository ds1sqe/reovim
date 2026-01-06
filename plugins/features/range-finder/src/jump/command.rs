//! Jump navigation commands
//!
//! Command-event types for leap-style jump navigation.

use reovim_core::{
    command::traits::{CommandResult, CommandTrait, ExecutionContext},
    declare_event_command,
    event_bus::{DynEvent, Event},
};

// === Jump mode types ===

/// Jump search mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpMode {
    /// Multi-character search (2 chars + label)
    MultiChar,
    /// Single-char find - cursor ON match (f/F keys)
    FindChar,
    /// Single-char till - cursor BEFORE match (t/T keys)
    TillChar,
}

/// Search direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Search forward from cursor
    Forward,
    /// Search backward from cursor
    Backward,
}

// === Navigation commands with buffer context ===

/// Multi-char search command (s key)
#[derive(Debug, Clone, Copy)]
pub struct JumpSearch {
    /// Buffer ID where jump was triggered
    pub buffer_id: usize,
}

impl JumpSearch {
    /// Create a default instance for registration
    #[must_use]
    pub const fn default_instance() -> Self {
        Self { buffer_id: 0 }
    }
}

impl CommandTrait for JumpSearch {
    fn name(&self) -> &'static str {
        "jump_search"
    }

    fn description(&self) -> &'static str {
        "Multi-char search in current buffer (s key)"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let cursor_line = u32::from(ctx.buffer.cur.y);
        let cursor_col = u32::from(ctx.buffer.cur.x);
        let lines: Vec<String> = ctx
            .buffer
            .contents
            .iter()
            .map(|line| line.inner.clone())
            .collect();

        CommandResult::EmitEvent(DynEvent::new(JumpSearchStarted {
            buffer_id: ctx.buffer_id,
            cursor_line,
            cursor_col,
            lines,
            direction: Direction::Forward,
            operator_context: ctx.operator_context.map(Into::into),
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Find character forward command (f key)
#[derive(Debug, Clone, Copy)]
pub struct JumpFindChar {
    /// Buffer ID where jump was triggered
    pub buffer_id: usize,
}

impl JumpFindChar {
    /// Create a default instance for registration
    #[must_use]
    pub const fn default_instance() -> Self {
        Self { buffer_id: 0 }
    }
}

impl CommandTrait for JumpFindChar {
    fn name(&self) -> &'static str {
        "jump_find_char"
    }

    fn description(&self) -> &'static str {
        "Find character forward with labels (f key)"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let cursor_line = u32::from(ctx.buffer.cur.y);
        let cursor_col = u32::from(ctx.buffer.cur.x);
        let lines: Vec<String> = ctx
            .buffer
            .contents
            .iter()
            .map(|line| line.inner.clone())
            .collect();

        CommandResult::EmitEvent(DynEvent::new(JumpFindCharStarted {
            buffer_id: ctx.buffer_id,
            cursor_line,
            cursor_col,
            lines,
            mode: JumpMode::FindChar,
            direction: Direction::Forward,
            operator_context: ctx.operator_context.map(Into::into),
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Find character backward command (F key)
#[derive(Debug, Clone, Copy)]
pub struct JumpFindCharBack {
    /// Buffer ID where jump was triggered
    pub buffer_id: usize,
}

impl JumpFindCharBack {
    /// Create a default instance for registration
    #[must_use]
    pub const fn default_instance() -> Self {
        Self { buffer_id: 0 }
    }
}

impl CommandTrait for JumpFindCharBack {
    fn name(&self) -> &'static str {
        "jump_find_char_back"
    }

    fn description(&self) -> &'static str {
        "Find character backward with labels (F key)"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let cursor_line = u32::from(ctx.buffer.cur.y);
        let cursor_col = u32::from(ctx.buffer.cur.x);
        let lines: Vec<String> = ctx
            .buffer
            .contents
            .iter()
            .map(|line| line.inner.clone())
            .collect();

        CommandResult::EmitEvent(DynEvent::new(JumpFindCharStarted {
            buffer_id: ctx.buffer_id,
            cursor_line,
            cursor_col,
            lines,
            mode: JumpMode::FindChar,
            direction: Direction::Backward,
            operator_context: ctx.operator_context.map(Into::into),
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Till character forward command (t key)
#[derive(Debug, Clone, Copy)]
pub struct JumpTillChar {
    /// Buffer ID where jump was triggered
    pub buffer_id: usize,
}

impl JumpTillChar {
    /// Create a default instance for registration
    #[must_use]
    pub const fn default_instance() -> Self {
        Self { buffer_id: 0 }
    }
}

impl CommandTrait for JumpTillChar {
    fn name(&self) -> &'static str {
        "jump_till_char"
    }

    fn description(&self) -> &'static str {
        "Till character forward with labels (t key)"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let cursor_line = u32::from(ctx.buffer.cur.y);
        let cursor_col = u32::from(ctx.buffer.cur.x);
        let lines: Vec<String> = ctx
            .buffer
            .contents
            .iter()
            .map(|line| line.inner.clone())
            .collect();

        CommandResult::EmitEvent(DynEvent::new(JumpFindCharStarted {
            buffer_id: ctx.buffer_id,
            cursor_line,
            cursor_col,
            lines,
            mode: JumpMode::TillChar,
            direction: Direction::Forward,
            operator_context: ctx.operator_context.map(Into::into),
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Till character backward command (T key)
#[derive(Debug, Clone, Copy)]
pub struct JumpTillCharBack {
    /// Buffer ID where jump was triggered
    pub buffer_id: usize,
}

impl JumpTillCharBack {
    /// Create a default instance for registration
    #[must_use]
    pub const fn default_instance() -> Self {
        Self { buffer_id: 0 }
    }
}

impl CommandTrait for JumpTillCharBack {
    fn name(&self) -> &'static str {
        "jump_till_char_back"
    }

    fn description(&self) -> &'static str {
        "Till character backward with labels (T key)"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let cursor_line = u32::from(ctx.buffer.cur.y);
        let cursor_col = u32::from(ctx.buffer.cur.x);
        let lines: Vec<String> = ctx
            .buffer
            .contents
            .iter()
            .map(|line| line.inner.clone())
            .collect();

        CommandResult::EmitEvent(DynEvent::new(JumpFindCharStarted {
            buffer_id: ctx.buffer_id,
            cursor_line,
            cursor_col,
            lines,
            mode: JumpMode::TillChar,
            direction: Direction::Backward,
            operator_context: ctx.operator_context.map(Into::into),
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// === Cancel command (zero-sized, no buffer context needed) ===

declare_event_command! {
    JumpCancel,
    id: "jump_cancel",
    description: "Cancel jump mode (Escape key)",
}

// === Started events (emitted by command execution) ===

/// Event emitted when multi-char search is started
#[derive(Debug, Clone)]
pub struct JumpSearchStarted {
    /// Buffer ID
    pub buffer_id: usize,
    /// Current cursor line
    pub cursor_line: u32,
    /// Current cursor column
    pub cursor_col: u32,
    /// All buffer lines
    pub lines: Vec<String>,
    /// Search direction
    pub direction: Direction,
    /// Operator context if started from `OperatorPending` mode
    pub operator_context: Option<super::state::OperatorContext>,
}

impl Event for JumpSearchStarted {
    fn priority(&self) -> u32 {
        100
    }
}

/// Event emitted when single-char find/till is started
#[derive(Debug, Clone)]
pub struct JumpFindCharStarted {
    /// Buffer ID
    pub buffer_id: usize,
    /// Current cursor line
    pub cursor_line: u32,
    /// Current cursor column
    pub cursor_col: u32,
    /// All buffer lines
    pub lines: Vec<String>,
    /// Jump mode (`FindChar` or `TillChar`)
    pub mode: JumpMode,
    /// Search direction
    pub direction: Direction,
    /// Operator context if started from `OperatorPending` mode
    pub operator_context: Option<super::state::OperatorContext>,
}

impl Event for JumpFindCharStarted {
    fn priority(&self) -> u32 {
        100
    }
}

// === Input events (used during jump mode) ===

/// Character input during jump mode
#[derive(Debug, Clone, Copy)]
pub struct JumpInputChar {
    /// The input character
    pub c: char,
    /// Buffer ID where jump is active
    pub buffer_id: usize,
}

impl Event for JumpInputChar {
    fn priority(&self) -> u32 {
        100
    }
}

/// Label selection during jump mode
#[derive(Debug, Clone)]
pub struct JumpSelectLabel {
    /// The selected label
    pub label: String,
    /// Buffer ID where jump is active
    pub buffer_id: usize,
}

impl Event for JumpSelectLabel {
    fn priority(&self) -> u32 {
        100
    }
}

/// Jump execution event
#[derive(Debug, Clone, Copy)]
pub struct JumpExecute {
    /// Target buffer ID
    pub buffer_id: usize,
    /// Target line
    pub line: u32,
    /// Target column
    pub col: u32,
    /// Jump mode (affects cursor positioning)
    pub mode: JumpMode,
    /// Search direction (Forward or Backward)
    pub direction: Direction,
}

impl Event for JumpExecute {
    fn priority(&self) -> u32 {
        100
    }
}
