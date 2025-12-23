//! Settings menu plugin for reovim
//!
//! This plugin provides an interactive settings menu for configuring editor settings:
//! - Toggle options for booleans (checkbox style)
//! - Selection/cycle options for enums (dropdown style)
//! - Number input for numeric values
//! - Organized sections
//! - Vim-style navigation (j/k/h/l)
//! - Live preview of changes
//!
//! # Architecture
//!
//! This plugin is fully self-contained:
//! - Defines its own command IDs
//! - Manages its own state via `PluginStateRegistry`
//! - Renders via `PluginWindow` trait
//! - Communicates via `EventBus` events

mod commands;
mod settings_menu;

use std::{any::TypeId, sync::Arc};

// Import unified command-event types
pub use commands::{
    SettingsMenuClose, SettingsMenuCycleNext, SettingsMenuCyclePrev, SettingsMenuDecrement,
    SettingsMenuExecuteAction, SettingsMenuIncrement, SettingsMenuOpen, SettingsMenuQuick1,
    SettingsMenuQuick2, SettingsMenuQuick3, SettingsMenuQuick4, SettingsMenuQuick5,
    SettingsMenuQuick6, SettingsMenuQuick7, SettingsMenuQuick8, SettingsMenuQuick9,
    SettingsMenuQuickSelect, SettingsMenuSelectNext, SettingsMenuSelectPrev, SettingsMenuToggle,
};

use reovim_core::{
    bind::{CommandRef, KeymapScope},
    command::id::CommandId,
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

// Re-export key types for external use (non-command/event types)
pub use settings_menu::{
    ActionType, FlatItem, MenuLayout, MessageKind, SettingItem, SettingSection, SettingValue,
    SettingsInputMode, SettingsMenuState,
};

/// Plugin-local command IDs
pub mod command_id {
    use super::CommandId;

    pub const SETTINGS_MENU_OPEN: CommandId = CommandId::new("settings_menu_open");
    pub const SETTINGS_MENU_CLOSE: CommandId = CommandId::new("settings_menu_close");
    pub const SETTINGS_MENU_NEXT: CommandId = CommandId::new("settings_menu_next");
    pub const SETTINGS_MENU_PREV: CommandId = CommandId::new("settings_menu_prev");
    pub const SETTINGS_MENU_TOGGLE: CommandId = CommandId::new("settings_menu_toggle");
    pub const SETTINGS_MENU_CYCLE_NEXT: CommandId = CommandId::new("settings_menu_cycle_next");
    pub const SETTINGS_MENU_CYCLE_PREV: CommandId = CommandId::new("settings_menu_cycle_prev");
    pub const SETTINGS_MENU_INCREMENT: CommandId = CommandId::new("settings_menu_increment");
    pub const SETTINGS_MENU_DECREMENT: CommandId = CommandId::new("settings_menu_decrement");
    pub const SETTINGS_MENU_EXECUTE: CommandId = CommandId::new("settings_menu_execute");
    pub const SETTINGS_MENU_QUICK_1: CommandId = CommandId::new("settings_menu_quick_1");
    pub const SETTINGS_MENU_QUICK_2: CommandId = CommandId::new("settings_menu_quick_2");
    pub const SETTINGS_MENU_QUICK_3: CommandId = CommandId::new("settings_menu_quick_3");
    pub const SETTINGS_MENU_QUICK_4: CommandId = CommandId::new("settings_menu_quick_4");
    pub const SETTINGS_MENU_QUICK_5: CommandId = CommandId::new("settings_menu_quick_5");
    pub const SETTINGS_MENU_QUICK_6: CommandId = CommandId::new("settings_menu_quick_6");
    pub const SETTINGS_MENU_QUICK_7: CommandId = CommandId::new("settings_menu_quick_7");
    pub const SETTINGS_MENU_QUICK_8: CommandId = CommandId::new("settings_menu_quick_8");
    pub const SETTINGS_MENU_QUICK_9: CommandId = CommandId::new("settings_menu_quick_9");
}

/// Plugin window for settings menu
pub struct SettingsPluginWindow;

impl PluginWindow for SettingsPluginWindow {
    fn window_config(
        &self,
        state: &Arc<PluginStateRegistry>,
        _ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        state.with::<SettingsMenuState, _, _>(|settings| {
            if !settings.visible {
                return None;
            }

            let layout = &settings.layout;
            Some(WindowConfig {
                bounds: Rect::new(layout.x, layout.y, layout.width, layout.height),
                z_order: 400, // Modal settings
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
        let Some(settings) = state.with::<SettingsMenuState, _, _>(Clone::clone) else {
            return;
        };

        let border_style = &theme.popup.border;

        // Top border with title
        buffer.put_char(bounds.x, bounds.y, '╭', border_style);
        let title = " Settings ";
        for (i, ch) in title.chars().enumerate() {
            buffer.put_char(bounds.x + 1 + i as u16, bounds.y, ch, border_style);
        }
        for x in (bounds.x + 1 + title.len() as u16)..(bounds.x + bounds.width - 1) {
            buffer.put_char(x, bounds.y, '─', border_style);
        }
        buffer.put_char(bounds.x + bounds.width - 1, bounds.y, '╮', border_style);

        // Menu items
        for (row, item) in settings.flat_items.iter().enumerate() {
            let y = bounds.y + 1 + row as u16;
            if y >= bounds.y + bounds.height - 1 {
                break;
            }

            let is_selected = row == settings.selected_index;
            let style = if is_selected {
                &theme.popup.selected
            } else {
                &theme.popup.normal
            };

            buffer.put_char(bounds.x, y, '│', border_style);

            let item_text = match item {
                FlatItem::SectionHeader(name) => format!(" [{name}] "),
                FlatItem::Setting {
                    section_idx,
                    item_idx,
                } => {
                    if let Some(section) = settings.sections.get(*section_idx)
                        && let Some(setting) = section.items.get(*item_idx)
                    {
                        format!("  {} ", setting.label)
                    } else {
                        "  ??? ".to_string()
                    }
                }
            };

            for (i, ch) in item_text.chars().enumerate() {
                let x = bounds.x + 1 + i as u16;
                if x < bounds.x + bounds.width - 1 {
                    buffer.put_char(x, y, ch, style);
                }
            }

            for x in (bounds.x + 1 + item_text.len() as u16)..(bounds.x + bounds.width - 1) {
                buffer.put_char(x, y, ' ', style);
            }

            buffer.put_char(bounds.x + bounds.width - 1, y, '│', border_style);
        }

        // Bottom border
        let bottom_y = bounds.y + bounds.height - 1;
        buffer.put_char(bounds.x, bottom_y, '╰', border_style);
        for x in (bounds.x + 1)..(bounds.x + bounds.width - 1) {
            buffer.put_char(x, bottom_y, '─', border_style);
        }
        buffer.put_char(bounds.x + bounds.width - 1, bottom_y, '╯', border_style);
    }
}

/// Component ID for settings menu
pub const COMPONENT_ID: ComponentId = ComponentId("settings");

/// Settings menu plugin
///
/// Provides in-editor settings UI:
/// - Toggle settings
/// - Cycle through options
/// - Increment/decrement values
/// - Quick access keys (1-9)
pub struct SettingsMenuPlugin;

impl Plugin for SettingsMenuPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:settings-menu")
    }

    fn name(&self) -> &'static str {
        "Settings Menu"
    }

    fn description(&self) -> &'static str {
        "In-editor settings menu with live preview"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register display info for status line
        ctx.register_display(COMPONENT_ID, DisplayInfo::new(" SETTINGS ", " "));
        ctx.register_component_mode_display(
            COMPONENT_ID,
            EditModeKey::Insert,
            DisplayInfo::new(" SETTINGS | INSERT ", " "),
        );

        // Register navigation commands (unified types)
        let _ = ctx.register_command(SettingsMenuOpen);
        let _ = ctx.register_command(SettingsMenuClose);
        let _ = ctx.register_command(SettingsMenuSelectNext);
        let _ = ctx.register_command(SettingsMenuSelectPrev);

        // Register action commands (unified types)
        let _ = ctx.register_command(SettingsMenuToggle);
        let _ = ctx.register_command(SettingsMenuCycleNext);
        let _ = ctx.register_command(SettingsMenuCyclePrev);
        let _ = ctx.register_command(SettingsMenuIncrement);
        let _ = ctx.register_command(SettingsMenuDecrement);
        let _ = ctx.register_command(SettingsMenuExecuteAction);

        // Register quick select commands (1-9, unified types)
        let _ = ctx.register_command(SettingsMenuQuick1);
        let _ = ctx.register_command(SettingsMenuQuick2);
        let _ = ctx.register_command(SettingsMenuQuick3);
        let _ = ctx.register_command(SettingsMenuQuick4);
        let _ = ctx.register_command(SettingsMenuQuick5);
        let _ = ctx.register_command(SettingsMenuQuick6);
        let _ = ctx.register_command(SettingsMenuQuick7);
        let _ = ctx.register_command(SettingsMenuQuick8);
        let _ = ctx.register_command(SettingsMenuQuick9);

        // Register keybindings
        let editor_normal = KeymapScope::editor_normal();

        // Space+s to open settings menu
        ctx.bind_key_scoped(
            editor_normal,
            keys![Space 's'],
            CommandRef::Registered(command_id::SETTINGS_MENU_OPEN),
        );
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register the plugin window
        registry.register_plugin_window(Arc::new(SettingsPluginWindow));
    }
}
