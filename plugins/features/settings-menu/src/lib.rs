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
//! - Renders via `OverlayRenderer` trait
//! - Communicates via `EventBus` events

mod commands;
mod settings_menu;

use std::any::TypeId;

use {
    commands::{
        SettingsMenuCloseCommand, SettingsMenuCycleNextCommand, SettingsMenuCyclePrevCommand,
        SettingsMenuDecrementCommand, SettingsMenuExecuteCommand, SettingsMenuIncrementCommand,
        SettingsMenuNextCommand, SettingsMenuOpenCommand, SettingsMenuPrevCommand,
        SettingsMenuQuick1Command, SettingsMenuQuick2Command, SettingsMenuQuick3Command,
        SettingsMenuQuick4Command, SettingsMenuQuick5Command, SettingsMenuQuick6Command,
        SettingsMenuQuick7Command, SettingsMenuQuick8Command, SettingsMenuQuick9Command,
        SettingsMenuToggleCommand,
    },
    reovim_core::{
        bind::{CommandRef, KeymapScope},
        command::id::CommandId,
        component::RenderContext,
        display::{DisplayInfo, EditModeKey},
        frame::FrameBuffer,
        keys,
        overlay::{OverlayBounds, OverlayRenderer},
        plugin::{Plugin, PluginContext, PluginId},
        ui_component::ComponentId,
    },
};

// Re-export key types for external use
pub use settings_menu::{
    ActionType, FlatItem, MenuLayout, MessageKind, SettingItem, SettingSection, SettingValue,
    SettingsInputMode, SettingsMenuCloseEvent, SettingsMenuCycleNextEvent,
    SettingsMenuCyclePrevEvent, SettingsMenuDecrementEvent, SettingsMenuExecuteActionEvent,
    SettingsMenuIncrementEvent, SettingsMenuOpenEvent, SettingsMenuQuickSelectEvent,
    SettingsMenuSelectNextEvent, SettingsMenuSelectPrevEvent, SettingsMenuState,
    SettingsMenuToggleEvent,
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

/// Settings menu overlay
///
/// Stateless overlay that accesses `SettingsMenuState` through `RenderContext`.
pub struct SettingsOverlay;

impl OverlayRenderer for SettingsOverlay {
    fn id(&self) -> &'static str {
        "settings"
    }

    fn z_order(&self) -> u16 {
        400
    }

    fn is_visible(&self, ctx: &RenderContext<'_>) -> bool {
        ctx.state()
            .and_then(|s| {
                s.plugin_state
                    .with::<SettingsMenuState, _, _>(|m| m.visible)
            })
            .unwrap_or(false)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(&self, buffer: &mut FrameBuffer, ctx: &RenderContext<'_>) {
        let Some(state) = ctx.state() else { return };
        let Some(settings) = state
            .plugin_state
            .with::<SettingsMenuState, _, _>(Clone::clone)
        else {
            return;
        };
        let theme = ctx.theme;

        let layout = &settings.layout;
        let border_style = &theme.popup.border;

        // Top border with title
        buffer.put_char(layout.x, layout.y, '╭', border_style);
        let title = " Settings ";
        for (i, ch) in title.chars().enumerate() {
            buffer.put_char(layout.x + 1 + i as u16, layout.y, ch, border_style);
        }
        for x in (layout.x + 1 + title.len() as u16)..(layout.x + layout.width - 1) {
            buffer.put_char(x, layout.y, '─', border_style);
        }
        buffer.put_char(layout.x + layout.width - 1, layout.y, '╮', border_style);

        // Menu items
        for (row, item) in settings.flat_items.iter().enumerate() {
            let y = layout.y + 1 + row as u16;
            if y >= layout.y + layout.height - 1 {
                break;
            }

            let is_selected = row == settings.selected_index;
            let style = if is_selected {
                &theme.popup.selected
            } else {
                &theme.popup.normal
            };

            buffer.put_char(layout.x, y, '│', border_style);

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
                let x = layout.x + 1 + i as u16;
                if x < layout.x + layout.width - 1 {
                    buffer.put_char(x, y, ch, style);
                }
            }

            for x in (layout.x + 1 + item_text.len() as u16)..(layout.x + layout.width - 1) {
                buffer.put_char(x, y, ' ', style);
            }

            buffer.put_char(layout.x + layout.width - 1, y, '│', border_style);
        }

        // Bottom border
        let bottom_y = layout.y + layout.height - 1;
        buffer.put_char(layout.x, bottom_y, '╰', border_style);
        for x in (layout.x + 1)..(layout.x + layout.width - 1) {
            buffer.put_char(x, bottom_y, '─', border_style);
        }
        buffer.put_char(layout.x + layout.width - 1, bottom_y, '╯', border_style);
    }

    fn bounds(&self, ctx: &RenderContext<'_>) -> OverlayBounds {
        ctx.state()
            .and_then(|s| {
                s.plugin_state.with::<SettingsMenuState, _, _>(|m| {
                    let layout = &m.layout;
                    OverlayBounds::new(layout.x, layout.y, layout.width, layout.height)
                })
            })
            .unwrap_or_default()
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

        // Register navigation commands
        let _ = ctx.register_command(SettingsMenuOpenCommand);
        let _ = ctx.register_command(SettingsMenuCloseCommand);
        let _ = ctx.register_command(SettingsMenuNextCommand);
        let _ = ctx.register_command(SettingsMenuPrevCommand);

        // Register action commands
        let _ = ctx.register_command(SettingsMenuToggleCommand);
        let _ = ctx.register_command(SettingsMenuCycleNextCommand);
        let _ = ctx.register_command(SettingsMenuCyclePrevCommand);
        let _ = ctx.register_command(SettingsMenuIncrementCommand);
        let _ = ctx.register_command(SettingsMenuDecrementCommand);
        let _ = ctx.register_command(SettingsMenuExecuteCommand);

        // Register quick select commands (1-9)
        let _ = ctx.register_command(SettingsMenuQuick1Command);
        let _ = ctx.register_command(SettingsMenuQuick2Command);
        let _ = ctx.register_command(SettingsMenuQuick3Command);
        let _ = ctx.register_command(SettingsMenuQuick4Command);
        let _ = ctx.register_command(SettingsMenuQuick5Command);
        let _ = ctx.register_command(SettingsMenuQuick6Command);
        let _ = ctx.register_command(SettingsMenuQuick7Command);
        let _ = ctx.register_command(SettingsMenuQuick8Command);
        let _ = ctx.register_command(SettingsMenuQuick9Command);

        // Register overlay
        ctx.register_overlay(SettingsOverlay);

        // Register keybindings
        let editor_normal = KeymapScope::editor_normal();

        // Space+s to open settings menu
        ctx.bind_key_scoped(
            editor_normal,
            keys![Space 's'],
            CommandRef::Registered(command_id::SETTINGS_MENU_OPEN),
        );
    }
}
