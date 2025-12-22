//! Core editor plugin
//!
//! Provides fundamental editor commands: cursor movement, mode switching,
//! basic text operations, and the Editor interactor.

use crate::{
    command::builtin::{
        // Buffer
        BufferDeleteCommand,
        BufferNextCommand,
        BufferPrevCommand,
        // Change
        ChangeLineCommand,
        // Command line
        CommandLineBackspaceCommand,
        CommandLineCancelCommand,
        CommandLineExecuteCommand,
        // Cursor
        CursorDownCommand,
        CursorLeftCommand,
        CursorLineEndCommand,
        CursorLineStartCommand,
        CursorRightCommand,
        CursorUpCommand,
        CursorWordBackwardCommand,
        CursorWordEndCommand,
        CursorWordForwardCommand,
        // Text
        DeleteCharBackwardCommand,
        DeleteCharForwardCommand,
        DeleteLineCommand,
        // Operators
        EnterChangeOperatorCommand,
        // Mode
        EnterCommandModeCommand,
        EnterDeleteOperatorCommand,
        EnterInsertModeAfterCommand,
        EnterInsertModeCommand,
        EnterInsertModeEolCommand,
        EnterNormalModeCommand,
        EnterVisualBlockModeCommand,
        EnterVisualLineModeCommand,
        EnterVisualModeCommand,
        EnterYankOperatorCommand,
        // Navigation
        GotoFirstLineCommand,
        GotoLastLineCommand,
        InsertNewlineCommand,
        // Jump
        JumpNewerCommand,
        JumpOlderCommand,
        // System
        NoopCommand,
        OpenLineAboveCommand,
        OpenLineBelowCommand,
        // Clipboard
        PasteBeforeCommand,
        PasteCommand,
        QuitCommand,
        // History
        RedoCommand,
        UndoCommand,
        // Visual
        VisualDeleteCommand,
        VisualExtendDownCommand,
        VisualExtendLeftCommand,
        VisualExtendRightCommand,
        VisualExtendUpCommand,
        VisualYankCommand,
        // Yank
        YankLineCommand,
        YankToEndCommand,
    },
    display::{DisplayInfo, EditModeKey, SubModeKey},
    plugin::{Plugin, PluginContext, PluginId},
    ui_component::ComponentId,
};

/// Core editor functionality plugin
///
/// This is the foundational plugin that all other plugins depend on.
/// It provides:
/// - Cursor movement commands (hjkl, w, e, b, etc.)
/// - Mode switching commands (normal, insert, visual, command)
/// - Basic text operations (insert, delete, newline)
/// - Operators (delete, yank, change)
/// - Undo/redo history
/// - Clipboard operations
///
/// This plugin has no dependencies and is always loaded first.
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:core")
    }

    fn name(&self) -> &'static str {
        "Core Editor"
    }

    fn description(&self) -> &'static str {
        "Core editing: cursor, modes, text ops, operators, clipboard"
    }

    fn build(&self, ctx: &mut PluginContext) {
        self.register_cursor_commands(ctx);
        self.register_mode_commands(ctx);
        self.register_operator_commands(ctx);
        self.register_text_commands(ctx);
        self.register_visual_commands(ctx);
        self.register_command_line_commands(ctx);
        self.register_history_commands(ctx);
        self.register_clipboard_commands(ctx);
        self.register_jump_commands(ctx);
        self.register_buffer_commands(ctx);
        self.register_system_commands(ctx);
        self.register_builtin_displays(ctx);
    }
}

#[allow(clippy::unused_self)]
impl CorePlugin {
    fn register_cursor_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(CursorUpCommand);
        let _ = ctx.register_command(CursorDownCommand);
        let _ = ctx.register_command(CursorLeftCommand);
        let _ = ctx.register_command(CursorRightCommand);
        let _ = ctx.register_command(CursorLineStartCommand);
        let _ = ctx.register_command(CursorLineEndCommand);
        let _ = ctx.register_command(CursorWordForwardCommand);
        let _ = ctx.register_command(CursorWordBackwardCommand);
        let _ = ctx.register_command(CursorWordEndCommand);
        let _ = ctx.register_command(GotoFirstLineCommand);
        let _ = ctx.register_command(GotoLastLineCommand);
    }

    fn register_mode_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(EnterNormalModeCommand);
        let _ = ctx.register_command(EnterInsertModeCommand);
        let _ = ctx.register_command(EnterInsertModeAfterCommand);
        let _ = ctx.register_command(EnterInsertModeEolCommand);
        let _ = ctx.register_command(OpenLineBelowCommand);
        let _ = ctx.register_command(OpenLineAboveCommand);
        let _ = ctx.register_command(EnterVisualModeCommand);
        let _ = ctx.register_command(EnterVisualBlockModeCommand);
        let _ = ctx.register_command(EnterVisualLineModeCommand);
        let _ = ctx.register_command(EnterCommandModeCommand);
    }

    fn register_operator_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(EnterDeleteOperatorCommand);
        let _ = ctx.register_command(EnterYankOperatorCommand);
        let _ = ctx.register_command(EnterChangeOperatorCommand);
    }

    fn register_text_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(DeleteCharBackwardCommand);
        let _ = ctx.register_command(DeleteCharForwardCommand);
        let _ = ctx.register_command(DeleteLineCommand);
        let _ = ctx.register_command(InsertNewlineCommand);
        let _ = ctx.register_command(YankLineCommand);
        let _ = ctx.register_command(YankToEndCommand);
        let _ = ctx.register_command(ChangeLineCommand);
    }

    fn register_visual_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(VisualExtendUpCommand);
        let _ = ctx.register_command(VisualExtendDownCommand);
        let _ = ctx.register_command(VisualExtendLeftCommand);
        let _ = ctx.register_command(VisualExtendRightCommand);
        let _ = ctx.register_command(VisualDeleteCommand);
        let _ = ctx.register_command(VisualYankCommand);
    }

    fn register_command_line_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(CommandLineBackspaceCommand);
        let _ = ctx.register_command(CommandLineExecuteCommand);
        let _ = ctx.register_command(CommandLineCancelCommand);
    }

    fn register_history_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(UndoCommand);
        let _ = ctx.register_command(RedoCommand);
    }

    fn register_clipboard_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(PasteCommand);
        let _ = ctx.register_command(PasteBeforeCommand);
    }

    fn register_jump_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(JumpOlderCommand);
        let _ = ctx.register_command(JumpNewerCommand);
    }

    fn register_buffer_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(BufferPrevCommand);
        let _ = ctx.register_command(BufferNextCommand);
        let _ = ctx.register_command(BufferDeleteCommand);
    }

    fn register_system_commands(&self, ctx: &PluginContext) {
        let _ = ctx.register_command(QuitCommand);
        let _ = ctx.register_command(NoopCommand);
    }

    /// Register built-in display strings and icons for core modes
    fn register_builtin_displays(&self, ctx: &mut PluginContext) {
        // === Editor modes ===
        ctx.register_component_mode_display(
            ComponentId::EDITOR,
            EditModeKey::Normal,
            DisplayInfo::new(" NORMAL ", "󰆾 "),
        );
        ctx.register_component_mode_display(
            ComponentId::EDITOR,
            EditModeKey::Insert,
            DisplayInfo::new(" INSERT ", "󰏫 "),
        );
        ctx.register_component_mode_display(
            ComponentId::EDITOR,
            EditModeKey::InsertReplace,
            DisplayInfo::new(" REPLACE ", "󰏫 "),
        );
        ctx.register_component_mode_display(
            ComponentId::EDITOR,
            EditModeKey::VisualChar,
            DisplayInfo::new(" VISUAL ", "󰒉 "),
        );
        ctx.register_component_mode_display(
            ComponentId::EDITOR,
            EditModeKey::VisualLine,
            DisplayInfo::new(" V-LINE ", "󰒉 "),
        );
        ctx.register_component_mode_display(
            ComponentId::EDITOR,
            EditModeKey::VisualBlock,
            DisplayInfo::new(" V-BLOCK ", "󰒉 "),
        );

        // === Sub-modes (core only) ===
        ctx.register_sub_mode_display(SubModeKey::Command, DisplayInfo::new(" COMMAND ", " "));
        ctx.register_sub_mode_display(
            SubModeKey::OperatorPending,
            DisplayInfo::new(" OPERATOR ", "󰆾 "),
        );
        // Note: Plugin sub-modes (Leap, etc.) are registered by their respective plugins
    }
}
