//! Telescope fuzzy finder plugin for reovim
//!
//! This plugin provides fuzzy finding capabilities:
//! - File picker (Space ff)
//! - Buffer picker (Space fb)
//! - Grep picker (Space fg)
//! - Command palette
//! - Themes picker
//! - Keymaps viewer
//! - And more...
//!
//! # Architecture
//!
//! Commands emit `EventBus` events that are handled by the runtime.
//! State is managed via `PluginStateRegistry`.
//! Rendering is done via the `PluginWindow` trait.

pub mod commands;
pub mod telescope;

use std::{any::TypeId, sync::Arc};

use reovim_core::{
    bind::{CommandRef, KeymapScope},
    display::{DisplayInfo, EditModeKey},
    frame::FrameBuffer,
    highlight::Theme,
    keys,
    modd::ComponentId,
    plugin::{
        EditorContext, Plugin, PluginContext, PluginId, PluginStateRegistry, PluginWindow, Rect,
        WindowConfig,
    },
};

// Re-export unified command-event types
pub use commands::{
    TelescopeBackspace, TelescopeClose, TelescopeCommands, TelescopeConfirm, TelescopeCursorLeft,
    TelescopeCursorRight, TelescopeEnterInsert, TelescopeEnterNormal, TelescopeFindBuffers,
    TelescopeFindFiles, TelescopeFindRecent, TelescopeGotoFirst, TelescopeGotoLast, TelescopeHelp,
    TelescopeInsertChar, TelescopeKeymaps, TelescopeLiveGrep, TelescopeOpen, TelescopePageDown,
    TelescopePageUp, TelescopeProfiles, TelescopeSelectNext, TelescopeSelectPrev, TelescopeThemes,
};

// Re-export telescope types (non-command/event)
pub use telescope::{TelescopeItem, TelescopeMatcher, TelescopeState};

/// Plugin window for telescope
pub struct TelescopePluginWindow;

impl PluginWindow for TelescopePluginWindow {
    fn window_config(
        &self,
        state: &Arc<PluginStateRegistry>,
        _ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        state.with::<TelescopeState, _, _>(|telescope| {
            if !telescope.active {
                return None;
            }

            let layout = &telescope.layout;
            Some(WindowConfig {
                bounds: Rect::new(layout.x, layout.y, layout.width, layout.height),
                z_order: 300, // Floating picker
                visible: true,
            })
        })?
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(
        &self,
        state: &Arc<PluginStateRegistry>,
        _ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        bounds: Rect,
        theme: &Theme,
    ) {
        let Some(telescope) = state.with::<TelescopeState, _, _>(Clone::clone) else {
            return;
        };

        let x = bounds.x;
        let y = bounds.y;
        let width = bounds.width;
        let height = bounds.height;

        let border_style = &theme.popup.border;
        let normal_style = &theme.popup.normal;
        let selected_style = &theme.popup.selected;

        // Top border with title
        buffer.put_char(x, y, '╭', border_style);
        let title = format!(" {} ", telescope.picker_name);
        for (i, ch) in title.chars().enumerate() {
            let cx = x + 1 + i as u16;
            if cx < x + width - 1 {
                buffer.put_char(cx, y, ch, border_style);
            }
        }
        for cx in (x + 1 + title.len() as u16)..(x + width - 1) {
            buffer.put_char(cx, y, '─', border_style);
        }
        buffer.put_char(x + width - 1, y, '╮', border_style);

        // Input line
        let input_y = y + 1;
        buffer.put_char(x, input_y, '│', border_style);
        buffer.put_char(x + 1, input_y, '>', normal_style);
        buffer.put_char(x + 2, input_y, ' ', normal_style);

        for (i, ch) in telescope.query.chars().enumerate() {
            let cx = x + 3 + i as u16;
            if cx < x + width - 1 {
                buffer.put_char(cx, input_y, ch, normal_style);
            }
        }
        for cx in (x + 3 + telescope.query.len() as u16)..(x + width - 1) {
            buffer.put_char(cx, input_y, ' ', normal_style);
        }
        buffer.put_char(x + width - 1, input_y, '│', border_style);

        // Separator
        let sep_y = y + 2;
        buffer.put_char(x, sep_y, '├', border_style);
        for cx in (x + 1)..(x + width - 1) {
            buffer.put_char(cx, sep_y, '─', border_style);
        }
        buffer.put_char(x + width - 1, sep_y, '┤', border_style);

        // Results
        let results_start = y + 3;
        let visible_items = telescope.visible_items();
        let max_results = (height - 4) as usize;

        for (idx, item) in visible_items.iter().take(max_results).enumerate() {
            let ry = results_start + idx as u16;
            let is_selected = idx == telescope.selected_index;
            let style = if is_selected {
                selected_style
            } else {
                normal_style
            };

            buffer.put_char(x, ry, '│', border_style);

            let indicator = if is_selected { '>' } else { ' ' };
            buffer.put_char(x + 1, ry, indicator, style);
            buffer.put_char(x + 2, ry, ' ', style);

            for (i, ch) in item.display.chars().enumerate() {
                let cx = x + 3 + i as u16;
                if cx < x + width - 1 {
                    buffer.put_char(cx, ry, ch, style);
                }
            }

            for cx in (x + 3 + item.display.len() as u16)..(x + width - 1) {
                buffer.put_char(cx, ry, ' ', style);
            }

            buffer.put_char(x + width - 1, ry, '│', border_style);
        }

        // Empty rows
        for ry in (results_start + visible_items.len().min(max_results) as u16)..(y + height - 1) {
            buffer.put_char(x, ry, '│', border_style);
            for cx in (x + 1)..(x + width - 1) {
                buffer.put_char(cx, ry, ' ', normal_style);
            }
            buffer.put_char(x + width - 1, ry, '│', border_style);
        }

        // Bottom border
        let bottom_y = y + height - 1;
        buffer.put_char(x, bottom_y, '╰', border_style);
        for cx in (x + 1)..(x + width - 1) {
            buffer.put_char(cx, bottom_y, '─', border_style);
        }
        buffer.put_char(x + width - 1, bottom_y, '╯', border_style);
    }
}

/// Component ID for telescope
pub const COMPONENT_ID: ComponentId = ComponentId("telescope");

/// Command IDs for telescope
pub mod command_id {
    use reovim_core::command::CommandId;

    pub const TELESCOPE_FIND_FILES: CommandId = CommandId::new("telescope_find_files");
    pub const TELESCOPE_FIND_BUFFERS: CommandId = CommandId::new("telescope_find_buffers");
    pub const TELESCOPE_LIVE_GREP: CommandId = CommandId::new("telescope_live_grep");
    pub const TELESCOPE_RECENT_FILES: CommandId = CommandId::new("telescope_recent_files");
    pub const TELESCOPE_COMMANDS: CommandId = CommandId::new("telescope_commands");
    pub const TELESCOPE_HELP_TAGS: CommandId = CommandId::new("telescope_help_tags");
    pub const TELESCOPE_KEYMAPS: CommandId = CommandId::new("telescope_keymaps");
    pub const TELESCOPE_THEMES: CommandId = CommandId::new("telescope_themes");
    pub const TELESCOPE_SELECT_NEXT: CommandId = CommandId::new("telescope_select_next");
    pub const TELESCOPE_SELECT_PREV: CommandId = CommandId::new("telescope_select_prev");
    pub const TELESCOPE_PAGE_DOWN: CommandId = CommandId::new("telescope_page_down");
    pub const TELESCOPE_PAGE_UP: CommandId = CommandId::new("telescope_page_up");
    pub const TELESCOPE_CONFIRM: CommandId = CommandId::new("telescope_confirm");
    pub const TELESCOPE_CLOSE: CommandId = CommandId::new("telescope_close");
    pub const TELESCOPE_BACKSPACE: CommandId = CommandId::new("telescope_backspace");
    pub const TELESCOPE_GOTO_FIRST: CommandId = CommandId::new("telescope_goto_first");
    pub const TELESCOPE_GOTO_LAST: CommandId = CommandId::new("telescope_goto_last");
    pub const TELESCOPE_ENTER_INSERT: CommandId = CommandId::new("telescope_enter_insert");
    pub const TELESCOPE_ENTER_NORMAL: CommandId = CommandId::new("telescope_enter_normal");
}

/// Telescope fuzzy finder plugin
///
/// Provides fuzzy finding capabilities:
/// - File picker (Space ff)
/// - Buffer picker (Space fb)
/// - Grep picker (Space fg)
/// - Command palette
/// - And more...
pub struct TelescopePlugin;

impl Plugin for TelescopePlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:telescope")
    }

    fn name(&self) -> &'static str {
        "Telescope"
    }

    fn description(&self) -> &'static str {
        "Fuzzy finder: files, buffers, grep, commands"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register display info for status line
        ctx.register_display(COMPONENT_ID, DisplayInfo::new(" TELESCOPE ", "󰍉 "));
        ctx.register_component_mode_display(
            COMPONENT_ID,
            EditModeKey::Normal,
            DisplayInfo::new(" TELESCOPE ", "󰍉 "),
        );
        ctx.register_component_mode_display(
            COMPONENT_ID,
            EditModeKey::Insert,
            DisplayInfo::new(" TELESCOPE | INSERT ", "󰍉 "),
        );

        // Register commands (unified types)
        let _ = ctx.register_command(TelescopeFindFiles);
        let _ = ctx.register_command(TelescopeFindBuffers);
        let _ = ctx.register_command(TelescopeLiveGrep);
        let _ = ctx.register_command(TelescopeFindRecent);
        let _ = ctx.register_command(TelescopeCommands);
        let _ = ctx.register_command(TelescopeHelp);
        let _ = ctx.register_command(TelescopeKeymaps);
        let _ = ctx.register_command(TelescopeThemes);
        let _ = ctx.register_command(TelescopeProfiles);
        let _ = ctx.register_command(TelescopeSelectNext);
        let _ = ctx.register_command(TelescopeSelectPrev);
        let _ = ctx.register_command(TelescopePageDown);
        let _ = ctx.register_command(TelescopePageUp);
        let _ = ctx.register_command(TelescopeGotoFirst);
        let _ = ctx.register_command(TelescopeGotoLast);
        let _ = ctx.register_command(TelescopeConfirm);
        let _ = ctx.register_command(TelescopeClose);
        let _ = ctx.register_command(TelescopeBackspace);
        let _ = ctx.register_command(TelescopeCursorLeft);
        let _ = ctx.register_command(TelescopeCursorRight);
        let _ = ctx.register_command(TelescopeEnterInsert);
        let _ = ctx.register_command(TelescopeEnterNormal);

        // Register keybindings (editor normal mode)
        let editor_normal = KeymapScope::editor_normal();
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![Space 'f' 'f'],
            CommandRef::Registered(command_id::TELESCOPE_FIND_FILES),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![Space 'f' 'b'],
            CommandRef::Registered(command_id::TELESCOPE_FIND_BUFFERS),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![Space 'f' 'g'],
            CommandRef::Registered(command_id::TELESCOPE_LIVE_GREP),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![Space 'f' 'r'],
            CommandRef::Registered(command_id::TELESCOPE_RECENT_FILES),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![Space 'f' 'c'],
            CommandRef::Registered(command_id::TELESCOPE_COMMANDS),
        );
        ctx.bind_key_scoped(
            editor_normal.clone(),
            keys![Space 'f' 'h'],
            CommandRef::Registered(command_id::TELESCOPE_HELP_TAGS),
        );
        ctx.bind_key_scoped(
            editor_normal,
            keys![Space 'f' 'k'],
            CommandRef::Registered(command_id::TELESCOPE_KEYMAPS),
        );
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register the plugin window
        registry.register_plugin_window(Arc::new(TelescopePluginWindow));
    }
}
