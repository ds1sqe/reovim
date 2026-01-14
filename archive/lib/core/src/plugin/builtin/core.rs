//! Core editor plugin
//!
//! Provides fundamental editor commands: cursor movement, mode switching,
//! basic text operations, and the Editor interactor.

use std::sync::Arc;

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
            CursorMatchingBracketCommand,
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
            // Window Mode
            EnterWindowModeCommand,
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
            WindowModeCloseCommand,
            WindowModeEqualizeCommand,
            WindowModeFocusDownCommand,
            WindowModeFocusLeftCommand,
            WindowModeFocusRightCommand,
            WindowModeFocusUpCommand,
            WindowModeMoveDownCommand,
            WindowModeMoveLeftCommand,
            WindowModeMoveRightCommand,
            WindowModeMoveUpCommand,
            WindowModeOnlyCommand,
            WindowModeSplitHCommand,
            WindowModeSplitVCommand,
            WindowModeSwapDownCommand,
            WindowModeSwapLeftCommand,
            WindowModeSwapRightCommand,
            WindowModeSwapUpCommand,
            // Yank
            YankLineCommand,
            YankToEndCommand,
        },
        id::CommandId,
    },
    event_bus::{
        EventBus, EventResult,
        core_events::{
            RequestSetIndentGuide, RequestSetLineNumbers, RequestSetRelativeLineNumbers,
            RequestSetScrollbar, RequestSetSignColumn, RequestSetTheme,
        },
    },
    keys,
    modd::ComponentId,
    option::{
        ChangeSource, OptionCategory, OptionChanged, OptionConstraint, OptionScope, OptionSpec,
        OptionValue, RegisterOption, RegisterSettingSection,
    },
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    screen::window::SignColumnMode,
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
        self.register_window_mode_commands(ctx);
        self.register_default_keybindings(ctx);
    }

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        self.register_core_options(bus);
        self.subscribe_option_changes(bus);
    }
}

impl CorePlugin {
    /// Subscribe to `OptionChanged` events and map option names to runtime capability requests.
    ///
    /// This bridges the gap between the option system and runtime capabilities.
    /// When an option changes (e.g., from the settings menu), we emit the appropriate
    /// `Request*` event that the runtime subscribes to.
    #[allow(clippy::unused_self)]
    fn subscribe_option_changes(&self, bus: &EventBus) {
        bus.subscribe::<OptionChanged, _>(100, move |event, ctx| {
            // Skip if from UserCommand - the :set handler already applies these directly
            if event.source == ChangeSource::UserCommand {
                return EventResult::Handled;
            }

            // Map option names to runtime capability requests
            let mut needs_render = false;
            match event.name.as_str() {
                "number" => {
                    if let OptionValue::Bool(enabled) = &event.new_value {
                        ctx.emit(RequestSetLineNumbers { enabled: *enabled });
                        needs_render = true;
                    }
                }
                "relativenumber" => {
                    if let OptionValue::Bool(enabled) = &event.new_value {
                        ctx.emit(RequestSetRelativeLineNumbers { enabled: *enabled });
                        needs_render = true;
                    }
                }
                "theme" => {
                    if let OptionValue::Choice { value, .. } = &event.new_value {
                        ctx.emit(RequestSetTheme {
                            name: value.clone(),
                        });
                        needs_render = true;
                    }
                }
                "scrollbar" => {
                    if let OptionValue::Bool(enabled) = &event.new_value {
                        ctx.emit(RequestSetScrollbar { enabled: *enabled });
                        needs_render = true;
                    }
                }
                "signcolumn" => {
                    if let OptionValue::String(value) = &event.new_value {
                        // Parse signcolumn value: "auto", "yes", "no", "number"
                        let mode = match value.as_str() {
                            "yes" => SignColumnMode::Yes(2),
                            "no" => SignColumnMode::No,
                            "number" => SignColumnMode::Number,
                            // "auto" and any invalid value default to Auto
                            _ => SignColumnMode::Auto,
                        };
                        ctx.emit(RequestSetSignColumn { mode });
                        needs_render = true;
                    }
                }
                "indentguide" => {
                    if let OptionValue::Bool(enabled) = &event.new_value {
                        ctx.emit(RequestSetIndentGuide { enabled: *enabled });
                        needs_render = true;
                    }
                }
                // Other options don't have runtime effects yet
                _ => {}
            }
            if needs_render {
                ctx.request_render();
            }
            EventResult::Handled
        });
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
        let _ = ctx.register_command(CursorMatchingBracketCommand);
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

    fn register_window_mode_commands(&self, ctx: &PluginContext) {
        // Window mode entry
        let _ = ctx.register_command(EnterWindowModeCommand);

        // Window mode navigation (exit after)
        let _ = ctx.register_command(WindowModeFocusLeftCommand);
        let _ = ctx.register_command(WindowModeFocusDownCommand);
        let _ = ctx.register_command(WindowModeFocusUpCommand);
        let _ = ctx.register_command(WindowModeFocusRightCommand);

        // Window mode move (exit after)
        let _ = ctx.register_command(WindowModeMoveLeftCommand);
        let _ = ctx.register_command(WindowModeMoveDownCommand);
        let _ = ctx.register_command(WindowModeMoveUpCommand);
        let _ = ctx.register_command(WindowModeMoveRightCommand);

        // Window mode swap (exit after)
        let _ = ctx.register_command(WindowModeSwapLeftCommand);
        let _ = ctx.register_command(WindowModeSwapDownCommand);
        let _ = ctx.register_command(WindowModeSwapUpCommand);
        let _ = ctx.register_command(WindowModeSwapRightCommand);

        // Window mode split/close (exit after)
        let _ = ctx.register_command(WindowModeSplitHCommand);
        let _ = ctx.register_command(WindowModeSplitVCommand);
        let _ = ctx.register_command(WindowModeCloseCommand);
        let _ = ctx.register_command(WindowModeOnlyCommand);
        let _ = ctx.register_command(WindowModeEqualizeCommand);
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
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys!['%'],
            CommandRef::Registered(CommandId::new("cursor_matching_bracket")),
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

        // === Default Normal bindings (fallback for all components) ===
        let default_normal = KeymapScope::DefaultNormal;
        ctx.bind_key_scoped(
            default_normal,
            keys![(Ctrl 'w')],
            CommandRef::Registered(CommandId::new("enter_window_mode")),
        );

        // === Window mode keybindings ===
        let window_mode = KeymapScope::SubMode(SubModeKind::Interactor(ComponentId::WINDOW));

        // Navigation (h/j/k/l)
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['h'],
            CommandRef::Registered(CommandId::new("window_mode_focus_left")),
        );
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['j'],
            CommandRef::Registered(CommandId::new("window_mode_focus_down")),
        );
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['k'],
            CommandRef::Registered(CommandId::new("window_mode_focus_up")),
        );
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['l'],
            CommandRef::Registered(CommandId::new("window_mode_focus_right")),
        );

        // Move window (H/J/K/L)
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['H'],
            CommandRef::Registered(CommandId::new("window_mode_move_left")),
        );
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['J'],
            CommandRef::Registered(CommandId::new("window_mode_move_down")),
        );
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['K'],
            CommandRef::Registered(CommandId::new("window_mode_move_up")),
        );
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['L'],
            CommandRef::Registered(CommandId::new("window_mode_move_right")),
        );

        // Swap (x + direction)
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['x' 'h'],
            CommandRef::Registered(CommandId::new("window_mode_swap_left")),
        );
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['x' 'j'],
            CommandRef::Registered(CommandId::new("window_mode_swap_down")),
        );
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['x' 'k'],
            CommandRef::Registered(CommandId::new("window_mode_swap_up")),
        );
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['x' 'l'],
            CommandRef::Registered(CommandId::new("window_mode_swap_right")),
        );

        // Split/Close/Other
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['s'],
            CommandRef::Registered(CommandId::new("window_mode_split_h")),
        );
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['v'],
            CommandRef::Registered(CommandId::new("window_mode_split_v")),
        );
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['c'],
            CommandRef::Registered(CommandId::new("window_mode_close")),
        );
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['o'],
            CommandRef::Registered(CommandId::new("window_mode_only")),
        );
        ctx.bind_key_scoped(
            window_mode.clone(),
            keys!['='],
            CommandRef::Registered(CommandId::new("window_mode_equalize")),
        );

        // Cancel - exit window mode
        ctx.bind_key_scoped(
            window_mode,
            keys![Escape],
            CommandRef::Registered(CommandId::new("enter_normal_mode")),
        );
    }

    /// Register core editor options via the extensible settings system.
    ///
    /// These options are dynamically registered and will appear in the settings menu.
    #[allow(clippy::too_many_lines)]
    fn register_core_options(&self, bus: &EventBus) {
        // === Register Settings Sections ===

        // Editor section
        bus.emit(RegisterSettingSection::new("Editor", "Editor").with_order(0));

        // Display section
        bus.emit(
            RegisterSettingSection::new("Display", "Display")
                .with_description("Display and rendering settings")
                .with_order(10),
        );

        // Window section
        bus.emit(
            RegisterSettingSection::new("Window", "Window")
                .with_description("Window and split settings")
                .with_order(20),
        );

        // === Register Editor Options ===

        // Line numbers
        bus.emit(RegisterOption::new(
            OptionSpec::new("number", "Show line numbers", OptionValue::Bool(true))
                .with_short("nu")
                .with_category(OptionCategory::Editor)
                .with_section("Editor")
                .with_scope(OptionScope::Window)
                .with_display_order(10),
        ));

        // Relative line numbers
        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "relativenumber",
                "Show relative line numbers",
                OptionValue::Bool(false),
            )
            .with_short("rnu")
            .with_category(OptionCategory::Editor)
            .with_section("Editor")
            .with_scope(OptionScope::Window)
            .with_display_order(11),
        ));

        // Sign column
        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "signcolumn",
                "Sign column display ('auto', 'yes', 'no', 'number')",
                OptionValue::String("yes".into()),
            )
            .with_category(OptionCategory::Editor)
            .with_section("Editor")
            .with_scope(OptionScope::Window)
            .with_display_order(12),
        ));

        // Tab width
        bus.emit(RegisterOption::new(
            OptionSpec::new("tabwidth", "Spaces per tab", OptionValue::Integer(4))
                .with_short("tw")
                .with_category(OptionCategory::Editor)
                .with_section("Editor")
                .with_scope(OptionScope::Buffer)
                .with_constraint(OptionConstraint::range(1, 8))
                .with_display_order(20),
        ));

        // Expand tab
        bus.emit(RegisterOption::new(
            OptionSpec::new("expandtab", "Use spaces instead of tabs", OptionValue::Bool(true))
                .with_short("et")
                .with_category(OptionCategory::Editor)
                .with_section("Editor")
                .with_scope(OptionScope::Buffer)
                .with_display_order(21),
        ));

        // Indent guides
        bus.emit(RegisterOption::new(
            OptionSpec::new("indentguide", "Show indentation guides", OptionValue::Bool(true))
                .with_category(OptionCategory::Editor)
                .with_section("Editor")
                .with_display_order(30),
        ));

        // Scrollbar
        bus.emit(RegisterOption::new(
            OptionSpec::new("scrollbar", "Show scrollbar", OptionValue::Bool(true))
                .with_category(OptionCategory::Editor)
                .with_section("Editor")
                .with_display_order(31),
        ));

        // Scroll offset
        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "scrolloff",
                "Lines to keep visible above/below cursor",
                OptionValue::Integer(5),
            )
            .with_short("so")
            .with_category(OptionCategory::Editor)
            .with_section("Editor")
            .with_constraint(OptionConstraint::min(0))
            .with_display_order(32),
        ));

        // === Register Display Options ===

        // Theme
        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "theme",
                "Color theme",
                OptionValue::choice(
                    "dark",
                    vec!["dark".into(), "light".into(), "tokyonight".into()],
                ),
            )
            .with_category(OptionCategory::Display)
            .with_section("Display")
            .with_display_order(10),
        ));

        // Color mode
        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "colormode",
                "Terminal color support",
                OptionValue::choice(
                    "truecolor",
                    vec!["ansi".into(), "256".into(), "truecolor".into()],
                ),
            )
            .with_category(OptionCategory::Display)
            .with_section("Display")
            .with_display_order(11),
        ));

        // === Register Window Options ===

        // Default split direction
        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "splitbelow",
                "New horizontal splits below current",
                OptionValue::Bool(true),
            )
            .with_short("sb")
            .with_category(OptionCategory::Window)
            .with_section("Window")
            .with_display_order(10),
        ));

        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "splitright",
                "New vertical splits to the right",
                OptionValue::Bool(true),
            )
            .with_short("spr")
            .with_category(OptionCategory::Window)
            .with_section("Window")
            .with_display_order(11),
        ));

        // Virtual text options (diagnostics)
        bus.emit(RegisterOption::new(
            OptionSpec::new("virtual_text", "Show inline diagnostics", OptionValue::Bool(true))
                .with_short("vt")
                .with_category(OptionCategory::Editor)
                .with_section("Diagnostics")
                .with_display_order(40),
        ));

        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "virtual_text_prefix",
                "Prefix for virtual text",
                OptionValue::String(String::new()),
            )
            .with_category(OptionCategory::Editor)
            .with_section("Diagnostics")
            .with_display_order(41),
        ));

        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "virtual_text_max_length",
                "Maximum virtual text length",
                OptionValue::Integer(80),
            )
            .with_category(OptionCategory::Editor)
            .with_section("Diagnostics")
            .with_constraint(OptionConstraint::range(10, 200))
            .with_display_order(42),
        ));

        bus.emit(RegisterOption::new(
            OptionSpec::new(
                "virtual_text_show",
                "Which diagnostics to show ('first', 'highest', 'all')",
                OptionValue::choice("first", vec!["first".into(), "highest".into(), "all".into()]),
            )
            .with_category(OptionCategory::Editor)
            .with_section("Diagnostics")
            .with_display_order(43),
        ));
    }
}
