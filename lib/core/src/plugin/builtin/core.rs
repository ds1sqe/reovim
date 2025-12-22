//! Core editor plugin
//!
//! Provides fundamental editor commands: cursor movement, mode switching,
//! basic text operations, and the Editor interactor.

use crate::{
    bind::{CommandRef, KeymapScope, SubModeKind},
    command::{
        builtin::{
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
        id::CommandId,
    },
    display::{DisplayInfo, EditModeKey, SubModeKey},
    keys,
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
        self.register_default_keybindings(ctx);
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

    /// Register default vim-style keybindings for the editor
    #[allow(clippy::too_many_lines)]
    fn register_default_keybindings(&self, ctx: &mut PluginContext) {
        let editor_normal = KeymapScope::editor_normal();
        let editor_insert = KeymapScope::editor_insert();
        let editor_visual = KeymapScope::editor_visual();
        let command_mode = KeymapScope::SubMode(SubModeKind::Command);

        // === Normal mode movement ===
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['h'],
            CommandRef::Registered(CommandId::new("cursor_left")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['j'],
            CommandRef::Registered(CommandId::new("cursor_down")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['k'],
            CommandRef::Registered(CommandId::new("cursor_up")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['l'],
            CommandRef::Registered(CommandId::new("cursor_right")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['w'],
            CommandRef::Registered(CommandId::new("cursor_word_forward")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['b'],
            CommandRef::Registered(CommandId::new("cursor_word_backward")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['e'],
            CommandRef::Registered(CommandId::new("cursor_word_end")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['0'],
            CommandRef::Registered(CommandId::new("cursor_line_start")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['$'],
            CommandRef::Registered(CommandId::new("cursor_line_end")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['g' 'g'],
            CommandRef::Registered(CommandId::new("goto_first_line")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['G'],
            CommandRef::Registered(CommandId::new("goto_last_line")),
        );

        // === Mode switching ===
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['i'],
            CommandRef::Registered(CommandId::new("enter_insert_mode")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['a'],
            CommandRef::Registered(CommandId::new("enter_insert_mode_after")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['A'],
            CommandRef::Registered(CommandId::new("enter_insert_mode_eol")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['o'],
            CommandRef::Registered(CommandId::new("open_line_below")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['O'],
            CommandRef::Registered(CommandId::new("open_line_above")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['v'],
            CommandRef::Registered(CommandId::new("enter_visual_mode")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['V'],
            CommandRef::Registered(CommandId::new("enter_visual_line_mode")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![(Ctrl 'v')],
            CommandRef::Registered(CommandId::new("enter_visual_block_mode")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![':'],
            CommandRef::Registered(CommandId::new("enter_command_mode")),
        );

        // === Operators ===
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['d'],
            CommandRef::Registered(CommandId::new("enter_delete_operator")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['y'],
            CommandRef::Registered(CommandId::new("enter_yank_operator")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['c'],
            CommandRef::Registered(CommandId::new("enter_change_operator")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['d' 'd'],
            CommandRef::Registered(CommandId::new("delete_line")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['y' 'y'],
            CommandRef::Registered(CommandId::new("yank_line")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['C'],
            CommandRef::Registered(CommandId::new("change_line")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['Y'],
            CommandRef::Registered(CommandId::new("yank_to_end")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['x'],
            CommandRef::Registered(CommandId::new("delete_char_forward")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['X'],
            CommandRef::Registered(CommandId::new("delete_char_backward")),
        );

        // === Paste ===
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['p'],
            CommandRef::Registered(CommandId::new("paste")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['P'],
            CommandRef::Registered(CommandId::new("paste_before")),
        );

        // === Undo/Redo ===
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['u'],
            CommandRef::Registered(CommandId::new("undo")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![(Ctrl 'r')],
            CommandRef::Registered(CommandId::new("redo")),
        );

        // === Jump list ===
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![(Ctrl 'o')],
            CommandRef::Registered(CommandId::new("jump_older")),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![(Ctrl 'i')],
            CommandRef::Registered(CommandId::new("jump_newer")),
        );

        // === Buffer navigation ===
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![':' 'b' 'n'],
            CommandRef::Registered(CommandId::new("buffer_next")),
        );
        ctx.bind_key_scoped(
            editor_normal,
            keys![':' 'b' 'p'],
            CommandRef::Registered(CommandId::new("buffer_prev")),
        );

        // === Insert mode ===
        ctx.bind_key_scoped(
            editor_insert.clone(),
            keys![Escape],
            CommandRef::Registered(CommandId::new("enter_normal_mode")),
        );
        ctx.bind_key_scoped(
            editor_insert,
            keys![(Ctrl 'c')],
            CommandRef::Registered(CommandId::new("enter_normal_mode")),
        );

        // === Visual mode ===
        ctx.bind_key_scoped(
            editor_visual.clone(),
            keys!['j'],
            CommandRef::Registered(CommandId::new("visual_extend_down")),
        );
        ctx.bind_key_scoped(
            editor_visual.clone(),
            keys!['k'],
            CommandRef::Registered(CommandId::new("visual_extend_up")),
        );
        ctx.bind_key_scoped(
            editor_visual.clone(),
            keys!['h'],
            CommandRef::Registered(CommandId::new("visual_extend_left")),
        );
        ctx.bind_key_scoped(
            editor_visual.clone(),
            keys!['l'],
            CommandRef::Registered(CommandId::new("visual_extend_right")),
        );
        ctx.bind_key_scoped(
            editor_visual.clone(),
            keys!['d'],
            CommandRef::Registered(CommandId::new("visual_delete")),
        );
        ctx.bind_key_scoped(
            editor_visual.clone(),
            keys!['y'],
            CommandRef::Registered(CommandId::new("visual_yank")),
        );
        ctx.bind_key_scoped(
            editor_visual.clone(),
            keys![':'],
            CommandRef::Registered(CommandId::new("enter_command_mode")),
        );
        ctx.bind_key_scoped(
            editor_visual,
            keys![Escape],
            CommandRef::Registered(CommandId::new("enter_normal_mode")),
        );

        // === Command mode ===
        ctx.bind_key_scoped(
            command_mode.clone(),
            keys![Enter],
            CommandRef::Registered(CommandId::new("command_line_execute")),
        );
        ctx.bind_key_scoped(
            command_mode.clone(),
            keys![Escape],
            CommandRef::Registered(CommandId::new("command_line_cancel")),
        );
        ctx.bind_key_scoped(
            command_mode,
            keys![Backspace],
            CommandRef::Registered(CommandId::new("command_line_backspace")),
        );
    }
}
