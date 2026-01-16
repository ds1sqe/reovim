//! Visual mode commands - entry, exit, selection manipulation, and operators.
//!
//! This module provides commands for visual mode operation:
//! - Mode entry: enter visual, visual-line, visual-block
//! - Mode exit: exit visual mode
//! - Selection manipulation: swap anchor/cursor, mode switching
//! - Selection operators: delete, yank, change, indent, dedent
//!
//! # Visual Mode Philosophy
//!
//! Visual mode commands follow Vim semantics:
//! - `v` enters character-wise selection from cursor position
//! - `V` enters line-wise selection
//! - `Ctrl-V` enters block (rectangular) selection
//! - `Esc` exits visual mode, clearing selection
//! - `o` swaps cursor and anchor positions
//! - `d` deletes selection
//! - `y` yanks selection
//! - `c` changes selection (delete + insert mode)

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_kernel::api::v1::{
        CommandId, KernelContext, Position, SelectionMode, events::ModeChanged,
    },
    reovim_module_operators::{DeleteOperator, Operator, OperatorContext, Range, YankOperator},
};

use super::mode::EDITOR_MODULE;

// =============================================================================
// Visual Mode Entry Commands
// =============================================================================

/// Enter visual mode (character-wise selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualMode;

impl Command for EnterVisualMode {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-visual")
    }

    fn description(&self) -> &'static str {
        "Enter visual mode (character-wise)"
    }
}

impl CommandHandler for EnterVisualMode {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Start character-wise selection at current cursor position
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();
            buffer.selection_mut().start(pos, SelectionMode::Character);
        }

        // Emit mode change event
        ctx.event_bus.emit(ModeChanged {
            from: "normal".to_string(),
            to: "visual".to_string(),
        });

        CommandResult::Success
    }
}

/// Enter visual line mode (line-wise selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualLineMode;

impl Command for EnterVisualLineMode {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-visual-line")
    }

    fn description(&self) -> &'static str {
        "Enter visual line mode"
    }
}

impl CommandHandler for EnterVisualLineMode {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Start line-wise selection at current cursor position
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();
            buffer.selection_mut().start(pos, SelectionMode::Line);
        }

        // Emit mode change event
        ctx.event_bus.emit(ModeChanged {
            from: "normal".to_string(),
            to: "visual-line".to_string(),
        });

        CommandResult::Success
    }
}

/// Enter visual block mode (rectangular selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualBlockMode;

impl Command for EnterVisualBlockMode {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "enter-visual-block")
    }

    fn description(&self) -> &'static str {
        "Enter visual block mode"
    }
}

impl CommandHandler for EnterVisualBlockMode {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Start block selection at current cursor position
        {
            let mut buffer = buffer_arc.write();
            let pos = buffer.position();
            buffer.selection_mut().start(pos, SelectionMode::Block);
        }

        // Emit mode change event
        ctx.event_bus.emit(ModeChanged {
            from: "normal".to_string(),
            to: "visual-block".to_string(),
        });

        CommandResult::Success
    }
}

// =============================================================================
// Visual Mode Exit Commands
// =============================================================================

/// Exit visual mode and return to normal mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitVisualMode;

impl Command for ExitVisualMode {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "exit-visual")
    }

    fn description(&self) -> &'static str {
        "Exit visual mode and return to normal mode"
    }
}

impl CommandHandler for ExitVisualMode {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        // Clear selection if we have a buffer
        if let Some(buffer_id) = args.buffer_id()
            && let Some(buffer_arc) = ctx.buffers.get(buffer_id)
        {
            let mut buffer = buffer_arc.write();
            buffer.selection_mut().clear();
        }

        // Emit mode change event
        ctx.event_bus.emit(ModeChanged {
            from: "visual".to_string(),
            to: "normal".to_string(),
        });

        CommandResult::Success
    }
}

// =============================================================================
// Selection Manipulation Commands
// =============================================================================

/// Swap cursor and anchor positions (o in visual mode).
///
/// In Vim, pressing 'o' in visual mode swaps the cursor position with the
/// anchor position, allowing you to adjust the other end of the selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct SwapAnchor;

impl Command for SwapAnchor {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "visual-swap-anchor")
    }

    fn description(&self) -> &'static str {
        "Swap cursor and anchor in visual mode"
    }
}

impl CommandHandler for SwapAnchor {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();

        // Only operate if selection is active
        if !buffer.selection().is_active() {
            return CommandResult::Success;
        }

        // Swap anchor and cursor
        let old_anchor = buffer.selection().anchor;
        let cursor = buffer.position();

        buffer.selection_mut().anchor = cursor;
        buffer.set_position(old_anchor);

        CommandResult::Success
    }
}

/// Toggle to character-wise visual mode (v in visual mode).
///
/// If already in character-wise mode, exit to normal mode.
/// Otherwise, switch to character-wise selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleVisualChar;

impl Command for ToggleVisualChar {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "toggle-visual-char")
    }

    fn description(&self) -> &'static str {
        "Toggle to character-wise visual mode"
    }
}

impl CommandHandler for ToggleVisualChar {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let (from_mode, to_mode) =
            if buffer.selection().is_active() && buffer.selection().mode().is_character() {
                // Already in character mode - exit to normal
                buffer.selection_mut().clear();
                ("visual", "normal")
            } else {
                // Switch to character mode
                buffer.selection_mut().set_mode(SelectionMode::Character);
                ("visual-line", "visual")
            };
        drop(buffer);

        ctx.event_bus.emit(ModeChanged {
            from: from_mode.to_string(),
            to: to_mode.to_string(),
        });

        CommandResult::Success
    }
}

/// Toggle to line-wise visual mode (V in visual mode).
///
/// If already in line-wise mode, exit to normal mode.
/// Otherwise, switch to line-wise selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleVisualLine;

impl Command for ToggleVisualLine {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "toggle-visual-line")
    }

    fn description(&self) -> &'static str {
        "Toggle to line-wise visual mode"
    }
}

impl CommandHandler for ToggleVisualLine {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let (from_mode, to_mode) =
            if buffer.selection().is_active() && buffer.selection().mode().is_line() {
                // Already in line mode - exit to normal
                buffer.selection_mut().clear();
                ("visual-line", "normal")
            } else {
                // Switch to line mode
                buffer.selection_mut().set_mode(SelectionMode::Line);
                ("visual", "visual-line")
            };
        drop(buffer);

        ctx.event_bus.emit(ModeChanged {
            from: from_mode.to_string(),
            to: to_mode.to_string(),
        });

        CommandResult::Success
    }
}

/// Toggle to block-wise visual mode (Ctrl-V in visual mode).
///
/// If already in block mode, exit to normal mode.
/// Otherwise, switch to block selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleVisualBlock;

impl Command for ToggleVisualBlock {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "toggle-visual-block")
    }

    fn description(&self) -> &'static str {
        "Toggle to block-wise visual mode"
    }
}

impl CommandHandler for ToggleVisualBlock {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let (from_mode, to_mode) =
            if buffer.selection().is_active() && buffer.selection().mode().is_block() {
                // Already in block mode - exit to normal
                buffer.selection_mut().clear();
                ("visual-block", "normal")
            } else {
                // Switch to block mode
                buffer.selection_mut().set_mode(SelectionMode::Block);
                ("visual", "visual-block")
            };
        drop(buffer);

        ctx.event_bus.emit(ModeChanged {
            from: from_mode.to_string(),
            to: to_mode.to_string(),
        });

        CommandResult::Success
    }
}

/// Reselect the last visual selection (gv in normal mode).
///
/// Restores the previous visual selection bounds and enters the appropriate
/// visual mode. If no previous selection exists, this is a no-op.
///
/// Note: The actual restoration logic is handled by the event loop since it
/// requires access to `AppState.last_visual_selection`. This command just signals
/// the intent to reselect.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReselectLast;

impl Command for ReselectLast {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "reselect-last")
    }

    fn description(&self) -> &'static str {
        "Reselect the last visual selection (gv)"
    }
}

impl CommandHandler for ReselectLast {
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // The actual reselection is handled by the event loop since it requires
        // AppState.last_visual_selection. This command just signals the intent.
        // If there's no last selection, the event loop handles it as a no-op.
        CommandResult::Success
    }
}

// =============================================================================
// Visual Operators
// =============================================================================

/// Helper function to get selection range from buffer.
///
/// Returns (range, `cursor_position`) if selection is active, None otherwise.
fn get_selection_range(
    ctx: &KernelContext,
    buffer_id: reovim_kernel::api::v1::BufferId,
) -> Option<(Range, Position)> {
    let buffer_arc = ctx.buffers.get(buffer_id)?;
    let buffer = buffer_arc.read();

    let selection = buffer.selection();
    if !selection.is_active() {
        return None;
    }

    let cursor = buffer.position();
    let anchor = selection.anchor;
    let mode = selection.mode();

    // Normalize: start should be before end
    let (start, end) = if anchor < cursor {
        (anchor, cursor)
    } else {
        (cursor, anchor)
    };

    // Get line info for Line mode (must be done before dropping buffer)
    let end_line_len = buffer.line_len(end.line).unwrap_or(0);
    let total_lines = buffer.line_count();
    drop(buffer);

    // Create range based on selection mode
    let range = match mode {
        SelectionMode::Character => {
            // Include the character at end position
            Range::new(start, Position::new(end.line, end.column + 1))
        }
        SelectionMode::Line => {
            // Expand to full lines, including the trailing newline
            let start = Position::new(start.line, 0);
            // For non-last lines, extend to start of next line (includes newline)
            // For last line, end at line length
            let end = if end.line + 1 < total_lines {
                Position::new(end.line + 1, 0)
            } else {
                Position::new(end.line, end_line_len)
            };
            Range::linewise(start, end)
        }
        SelectionMode::Block => {
            // Block mode: for now, treat as character range
            // Full block support is deferred
            Range::new(start, Position::new(end.line, end.column + 1))
        }
    };

    Some((range, start))
}

/// Delete selection (d in visual mode).
///
/// Deletes the selected text and stores it in the register.
/// Returns to Normal mode after execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteSelection;

impl Command for DeleteSelection {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "delete-selection")
    }

    fn description(&self) -> &'static str {
        "Delete visual selection"
    }
}

impl CommandHandler for DeleteSelection {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some((range, cursor_pos)) = get_selection_range(ctx, buffer_id) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Execute delete operator
        let delete_op = DeleteOperator;
        let mut op_ctx = OperatorContext {
            kernel: ctx,
            buffer_id,
            register: args.register(),
            count: 1,
        };

        if let Err(e) = delete_op.execute(&mut op_ctx, range) {
            return CommandResult::error(format!("Delete failed: {e}"));
        }

        // Clear selection and set cursor
        if let Some(buffer_arc) = ctx.buffers.get(buffer_id) {
            let mut buffer = buffer_arc.write();
            buffer.selection_mut().clear();
            buffer.set_position(cursor_pos);
        }

        // Mode transition to Normal is handled by event loop
        ctx.event_bus.emit(ModeChanged {
            from: "visual".to_string(),
            to: "normal".to_string(),
        });

        CommandResult::Success
    }
}

/// Yank selection (y in visual mode).
///
/// Copies the selected text to the register without deleting it.
/// Returns to Normal mode after execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct YankSelection;

impl Command for YankSelection {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "yank-selection")
    }

    fn description(&self) -> &'static str {
        "Yank visual selection"
    }
}

impl CommandHandler for YankSelection {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some((range, _cursor_pos)) = get_selection_range(ctx, buffer_id) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Execute yank operator
        let yank_op = YankOperator;
        let mut op_ctx = OperatorContext {
            kernel: ctx,
            buffer_id,
            register: args.register(),
            count: 1,
        };

        if let Err(e) = yank_op.execute(&mut op_ctx, range) {
            return CommandResult::error(format!("Yank failed: {e}"));
        }

        // Clear selection (yank doesn't move cursor in Vim, but returns to normal)
        if let Some(buffer_arc) = ctx.buffers.get(buffer_id) {
            let mut buffer = buffer_arc.write();
            buffer.selection_mut().clear();
        }

        // Mode transition to Normal is handled by event loop
        ctx.event_bus.emit(ModeChanged {
            from: "visual".to_string(),
            to: "normal".to_string(),
        });

        CommandResult::Success
    }
}

/// Change selection (c in visual mode).
///
/// Deletes the selected text and enters Insert mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangeSelection;

impl Command for ChangeSelection {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "change-selection")
    }

    fn description(&self) -> &'static str {
        "Change visual selection (delete and enter insert mode)"
    }
}

impl CommandHandler for ChangeSelection {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some((range, cursor_pos)) = get_selection_range(ctx, buffer_id) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Execute delete operator (change = delete + insert mode)
        let delete_op = DeleteOperator;
        let mut op_ctx = OperatorContext {
            kernel: ctx,
            buffer_id,
            register: args.register(),
            count: 1,
        };

        if let Err(e) = delete_op.execute(&mut op_ctx, range) {
            return CommandResult::error(format!("Change failed: {e}"));
        }

        // Clear selection and set cursor
        if let Some(buffer_arc) = ctx.buffers.get(buffer_id) {
            let mut buffer = buffer_arc.write();
            buffer.selection_mut().clear();
            buffer.set_position(cursor_pos);
        }

        // Mode transition to Insert is handled by event loop
        ctx.event_bus.emit(ModeChanged {
            from: "visual".to_string(),
            to: "insert".to_string(),
        });

        CommandResult::Success
    }
}

/// Indent selection (> in visual mode).
///
/// Increases indentation of selected lines.
/// Returns to Normal mode after execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct IndentSelection;

impl Command for IndentSelection {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "indent-selection")
    }

    fn description(&self) -> &'static str {
        "Indent visual selection"
    }
}

impl CommandHandler for IndentSelection {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let selection = buffer.selection();

        if !selection.is_active() {
            return CommandResult::Success; // No selection - no-op
        }

        let cursor = buffer.position();
        let anchor = selection.anchor;

        // Get line range
        let start_line = anchor.line.min(cursor.line);
        let end_line = anchor.line.max(cursor.line);

        // Indent each line (add tab/spaces at start)
        // Using 4 spaces as default indent
        let indent = "    ";
        for line_idx in start_line..=end_line {
            buffer.insert_at(Position::new(line_idx, 0), indent);
        }

        // Clear selection
        buffer.selection_mut().clear();

        // Mode transition to Normal is handled by event loop
        drop(buffer);
        ctx.event_bus.emit(ModeChanged {
            from: "visual".to_string(),
            to: "normal".to_string(),
        });

        CommandResult::Success
    }
}

/// Dedent selection (< in visual mode).
///
/// Decreases indentation of selected lines.
/// Returns to Normal mode after execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct DedentSelection;

impl Command for DedentSelection {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "dedent-selection")
    }

    fn description(&self) -> &'static str {
        "Dedent visual selection"
    }
}

impl CommandHandler for DedentSelection {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let selection = buffer.selection();

        if !selection.is_active() {
            return CommandResult::Success; // No selection - no-op
        }

        let cursor = buffer.position();
        let anchor = selection.anchor;

        // Get line range
        let start_line = anchor.line.min(cursor.line);
        let end_line = anchor.line.max(cursor.line);

        // Dedent each line (remove leading whitespace, up to 4 chars or one tab)
        for line_idx in start_line..=end_line {
            if let Some(line) = buffer.lines().get(line_idx) {
                let mut chars_to_remove = 0;
                for (i, c) in line.chars().enumerate() {
                    if c == '\t' {
                        chars_to_remove = i + 1;
                        break;
                    } else if c == ' ' && i < 4 {
                        chars_to_remove = i + 1;
                    } else {
                        break;
                    }
                }
                if chars_to_remove > 0 {
                    buffer.delete_at(Position::new(line_idx, 0), chars_to_remove);
                }
            }
        }

        // Clear selection
        buffer.selection_mut().clear();

        // Mode transition to Normal is handled by event loop
        drop(buffer);
        ctx.event_bus.emit(ModeChanged {
            from: "visual".to_string(),
            to: "normal".to_string(),
        });

        CommandResult::Success
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Get all visual mode selection commands.
#[must_use]
pub fn visual_selection_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(SwapAnchor),
        Box::new(ToggleVisualChar),
        Box::new(ToggleVisualLine),
        Box::new(ToggleVisualBlock),
        Box::new(ReselectLast),
    ]
}

/// Get all visual mode entry commands.
#[must_use]
pub fn visual_entry_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(EnterVisualMode),
        Box::new(EnterVisualLineMode),
        Box::new(EnterVisualBlockMode),
    ]
}

/// Get all visual mode exit commands.
#[must_use]
pub fn visual_exit_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(ExitVisualMode)]
}

/// Get all visual operator commands.
#[must_use]
pub fn visual_operator_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(DeleteSelection),
        Box::new(YankSelection),
        Box::new(ChangeSelection),
        Box::new(IndentSelection),
        Box::new(DedentSelection),
    ]
}

/// Get all visual mode commands (entry + exit + selection + operators).
#[must_use]
pub fn visual_commands() -> Vec<Box<dyn CommandHandler>> {
    let mut cmds = visual_entry_commands();
    cmds.extend(visual_exit_commands());
    cmds.extend(visual_selection_commands());
    cmds.extend(visual_operator_commands());
    cmds
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[allow(clippy::significant_drop_tightening)]
mod tests {
    use {
        super::*,
        reovim_kernel::api::v1::{
            Buffer, BufferError, BufferId, BufferManager, EventBus, MarkBank, MotionEngine,
            OptionRegistry, Position, RegisterBank, RwLock, TextObjectEngine,
        },
        std::{collections::HashMap, sync::Arc},
    };

    use super::super::mode::EDITOR_MODULE;

    /// Test buffer manager that actually stores buffers.
    struct TestBufferManager {
        buffers: RwLock<HashMap<BufferId, Arc<RwLock<Buffer>>>>,
    }

    impl TestBufferManager {
        fn new() -> Self {
            Self {
                buffers: RwLock::new(HashMap::new()),
            }
        }
    }

    impl BufferManager for TestBufferManager {
        fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
            self.buffers.read().get(&id).cloned()
        }

        fn create(&self) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(Buffer::new()));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn register(&self, buffer: Buffer) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(buffer));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError> {
            self.buffers
                .write()
                .remove(&id)
                .map_or(Err(BufferError::NotFound(id)), |arc_buffer| {
                    Arc::try_unwrap(arc_buffer)
                        .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
                })
        }

        fn list(&self) -> Vec<BufferId> {
            self.buffers.read().keys().copied().collect()
        }

        fn count(&self) -> usize {
            self.buffers.read().len()
        }
    }

    /// Create a `KernelContext` with a real buffer manager for testing.
    fn create_test_context() -> KernelContext {
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(RegisterBank::new())),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
        )
    }

    // =========================================================================
    // Command ID Tests
    // =========================================================================

    #[test]
    fn test_enter_visual_command_id() {
        let cmd = EnterVisualMode;
        assert_eq!(cmd.id().name(), "enter-visual");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_enter_visual_line_command_id() {
        let cmd = EnterVisualLineMode;
        assert_eq!(cmd.id().name(), "enter-visual-line");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_enter_visual_block_command_id() {
        let cmd = EnterVisualBlockMode;
        assert_eq!(cmd.id().name(), "enter-visual-block");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_exit_visual_command_id() {
        let cmd = ExitVisualMode;
        assert_eq!(cmd.id().name(), "exit-visual");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    // =========================================================================
    // Entry Command Execution Tests
    // =========================================================================

    #[test]
    fn test_enter_visual_activates_selection() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = EnterVisualMode.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Verify selection is active
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(buffer.selection().is_active());
        assert!(buffer.selection().mode().is_character());
    }

    #[test]
    fn test_enter_visual_line_activates_line_selection() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = EnterVisualLineMode.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Verify selection is active and in line mode
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(buffer.selection().is_active());
        assert!(buffer.selection().mode().is_line());
    }

    #[test]
    fn test_enter_visual_block_activates_block_selection() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = EnterVisualBlockMode.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Verify selection is active and in block mode
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(buffer.selection().is_active());
        assert!(buffer.selection().mode().is_block());
    }

    #[test]
    fn test_enter_visual_no_buffer_returns_error() {
        let mut ctx = create_test_context();
        let args = CommandContext::new();

        let result = EnterVisualMode.execute(&mut ctx, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    // =========================================================================
    // Exit Command Execution Tests
    // =========================================================================

    #[test]
    fn test_exit_visual_clears_selection() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // First enter visual mode
        let _ = EnterVisualMode.execute(&mut ctx, &args);

        // Verify selection is active
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let buffer = buffer_arc.read();
            assert!(buffer.selection().is_active());
        }

        // Exit visual mode
        let result = ExitVisualMode.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Verify selection is cleared
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(!buffer.selection().is_active());
    }

    #[test]
    fn test_exit_visual_without_buffer_succeeds() {
        let mut ctx = create_test_context();
        let args = CommandContext::new();

        // Should still succeed (just emits event)
        let result = ExitVisualMode.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    // =========================================================================
    // Selection Manipulation Tests
    // =========================================================================

    #[test]
    fn test_swap_anchor_command_id() {
        let cmd = SwapAnchor;
        assert_eq!(cmd.id().name(), "visual-swap-anchor");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_swap_anchor_swaps_positions() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Enter visual mode and move cursor
        let _ = EnterVisualMode.execute(&mut ctx, &args);

        // Move cursor to position 5
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer.set_position(Position::new(0, 5));
        }

        // Verify initial state: anchor at 0, cursor at 5
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let buffer = buffer_arc.read();
            assert_eq!(buffer.selection().anchor, Position::new(0, 0));
            assert_eq!(buffer.position(), Position::new(0, 5));
        }

        // Execute swap anchor
        let result = SwapAnchor.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Verify swapped: anchor at 5, cursor at 0
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.selection().anchor, Position::new(0, 5));
        assert_eq!(buffer.position(), Position::new(0, 0));
    }

    #[test]
    fn test_swap_anchor_noop_without_selection() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Don't enter visual mode - just execute swap
        let result = SwapAnchor.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn test_toggle_visual_char_exits_if_already_char() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Enter visual mode (character)
        let _ = EnterVisualMode.execute(&mut ctx, &args);

        // Toggle should exit
        let result = ToggleVisualChar.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Selection should be cleared
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(!buffer.selection().is_active());
    }

    #[test]
    fn test_toggle_visual_char_switches_from_line() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Enter visual line mode
        let _ = EnterVisualLineMode.execute(&mut ctx, &args);

        // Toggle to char mode
        let result = ToggleVisualChar.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Should be in character mode
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(buffer.selection().is_active());
        assert!(buffer.selection().mode().is_character());
    }

    #[test]
    fn test_toggle_visual_line_exits_if_already_line() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Enter visual line mode
        let _ = EnterVisualLineMode.execute(&mut ctx, &args);

        // Toggle should exit
        let result = ToggleVisualLine.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Selection should be cleared
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(!buffer.selection().is_active());
    }

    #[test]
    fn test_toggle_visual_block_switches_mode() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Enter visual mode (character)
        let _ = EnterVisualMode.execute(&mut ctx, &args);

        // Toggle to block mode
        let result = ToggleVisualBlock.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Should be in block mode
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(buffer.selection().is_active());
        assert!(buffer.selection().mode().is_block());
    }

    // =========================================================================
    // Helper Function Tests
    // =========================================================================

    #[test]
    fn test_visual_selection_commands_count() {
        let cmds = visual_selection_commands();
        assert_eq!(cmds.len(), 5); // SwapAnchor, Toggle x3, ReselectLast
    }

    #[test]
    fn test_visual_entry_commands_count() {
        let cmds = visual_entry_commands();
        assert_eq!(cmds.len(), 3);
    }

    #[test]
    fn test_visual_exit_commands_count() {
        let cmds = visual_exit_commands();
        assert_eq!(cmds.len(), 1);
    }

    #[test]
    fn test_visual_commands_count() {
        let cmds = visual_commands();
        assert_eq!(cmds.len(), 14); // 3 entry + 1 exit + 5 selection + 5 operators
    }

    #[test]
    fn test_visual_operator_commands_count() {
        let cmds = visual_operator_commands();
        assert_eq!(cmds.len(), 5); // delete, yank, change, indent, dedent
    }

    // =========================================================================
    // Reselect Last Tests
    // =========================================================================

    #[test]
    fn test_reselect_last_command_id() {
        let cmd = ReselectLast;
        assert_eq!(cmd.id().name(), "reselect-last");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_reselect_last_returns_success() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // ReselectLast always returns Success (actual logic is in event loop)
        let result = ReselectLast.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    // =========================================================================
    // Visual Operator Tests
    // =========================================================================

    #[test]
    fn test_delete_selection_command_id() {
        let cmd = DeleteSelection;
        assert_eq!(cmd.id().name(), "delete-selection");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_yank_selection_command_id() {
        let cmd = YankSelection;
        assert_eq!(cmd.id().name(), "yank-selection");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_change_selection_command_id() {
        let cmd = ChangeSelection;
        assert_eq!(cmd.id().name(), "change-selection");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_indent_selection_command_id() {
        let cmd = IndentSelection;
        assert_eq!(cmd.id().name(), "indent-selection");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_dedent_selection_command_id() {
        let cmd = DedentSelection;
        assert_eq!(cmd.id().name(), "dedent-selection");
        assert_eq!(cmd.id().module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_delete_selection_noop_without_selection() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Without an active selection, should be a no-op
        let result = DeleteSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer should be unchanged
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], "hello world");
    }

    #[test]
    fn test_yank_selection_noop_without_selection() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Without an active selection, should be a no-op
        let result = YankSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn test_change_selection_noop_without_selection() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Without an active selection, should be a no-op
        let result = ChangeSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn test_delete_selection_deletes_text() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        // Activate selection from position 0 to 5 (selecting "hello")
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer
                .selection_mut()
                .start(Position::new(0, 0), SelectionMode::Character);
            buffer.set_position(Position::new(0, 4)); // Selecting "hello"
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer should have "hello" deleted
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], " world");
    }

    #[test]
    fn test_indent_selection_adds_indentation() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("line1\nline2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        // Activate line selection for lines 0-1
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer
                .selection_mut()
                .start(Position::new(0, 0), SelectionMode::Line);
            buffer.set_position(Position::new(1, 0));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = IndentSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Lines 0-1 should be indented
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(buffer.lines()[0].starts_with("    ")); // 4 spaces
        assert!(buffer.lines()[1].starts_with("    "));
        assert!(!buffer.lines()[2].starts_with("    ")); // Line 3 not selected
    }

    #[test]
    fn test_dedent_selection_removes_indentation() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("    line1\n    line2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        // Activate line selection for lines 0-1
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer
                .selection_mut()
                .start(Position::new(0, 0), SelectionMode::Line);
            buffer.set_position(Position::new(1, 0));
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DedentSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Lines 0-1 should be dedented
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], "line1");
        assert_eq!(buffer.lines()[1], "line2");
        assert_eq!(buffer.lines()[2], "line3"); // Line 3 unchanged
    }

    // ========================================================================
    // P0 Missing Tests - Operator Behavior Validation
    // ========================================================================

    #[test]
    fn test_yank_selection_yanks_text() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        // Activate character-wise selection for "hello"
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer
                .selection_mut()
                .start(Position::new(0, 0), SelectionMode::Character);
            buffer.set_position(Position::new(0, 4)); // Selecting "hello"
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = YankSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer content should remain unchanged (yank doesn't delete)
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], "hello world");

        // Selection should be cleared after yank
        assert!(!buffer.selection().is_active());
    }

    #[test]
    fn test_change_selection_changes_text() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        // Activate character-wise selection for "hello"
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer
                .selection_mut()
                .start(Position::new(0, 0), SelectionMode::Character);
            buffer.set_position(Position::new(0, 4)); // Selecting "hello"
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = ChangeSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer should have "hello" deleted (like delete, but followed by insert mode)
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.lines()[0], " world");

        // Selection should be cleared after change
        assert!(!buffer.selection().is_active());
    }

    #[test]
    fn test_delete_selection_line_mode_deletes_entire_lines() {
        let mut ctx = create_test_context();
        let buffer = Buffer::from_string("line one\nline two\nline three");
        let buffer_id = ctx.buffers.register(buffer);

        // Activate line-wise selection for lines 0-1
        {
            let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
            let mut buffer = buffer_arc.write();
            buffer
                .selection_mut()
                .start(Position::new(0, 3), SelectionMode::Line);
            buffer.set_position(Position::new(1, 2)); // Selection spans lines 0 and 1
        }

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteSelection.execute(&mut ctx, &args);
        assert_eq!(result, CommandResult::Success);

        // Lines 0-1 should be completely deleted, leaving only "line three"
        let buffer_arc = ctx.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.lines()[0], "line three");
    }
}
