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
    bind::{CommandRef, EditModeKind, KeymapScope},
    command::id::CommandId,
    config::ProfileConfig,
    display::DisplayInfo,
    event_bus::{EventBus, EventResult, core_events::RequestFocusChange},
    frame::FrameBuffer,
    highlight::Theme,
    keys,
    modd::ComponentId,
    option::{ChangeSource, OptionChanged, RegisterOption, RegisterSettingSection},
    plugin::{
        EditorContext, Plugin, PluginContext, PluginId, PluginStateRegistry, PluginWindow, Rect,
        WindowConfig,
    },
    screen::border::{BorderConfig, BorderStyle, WindowAdjacency, render_border_to_buffer},
};

// Re-export key types for external use (non-command/event types)
pub use settings_menu::{
    ActionType, FlatItem, MenuLayout, MessageKind, RegisteredOption, SectionMeta, SettingChange,
    SettingItem, SettingSection, SettingValue, SettingsInputMode, SettingsMenuState,
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
            tracing::debug!("Settings render: no state available");
            return;
        };

        tracing::debug!(
            "Settings render: visible={}, items={}, selected={}, scroll={}",
            settings.visible,
            settings.flat_items.len(),
            settings.selected_index,
            settings.scroll_offset
        );

        // Log the value of the selected item
        if let Some(item) = settings.selected_item() {
            tracing::debug!(
                "Settings render: selected item key={}, value={:?}",
                item.key,
                item.value
            );
        }

        let normal_style = &theme.popup.normal;

        // Render border using the plugin window system's border utilities
        let border_config = BorderConfig::new(BorderStyle::Rounded).with_title("Settings");
        let adjacency = WindowAdjacency::default();
        render_border_to_buffer(
            buffer,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            &border_config,
            &theme.popup.border,
            &adjacency,
            true, // floating window
        );

        // Content rows (inside the border)
        let content_height = bounds.height.saturating_sub(2) as usize;
        for row in 0..content_height {
            let y = bounds.y + 1 + row as u16;

            // Get item for this row, accounting for scroll offset
            let item_idx = row + settings.scroll_offset;
            let item = settings.flat_items.get(item_idx);
            let is_selected = item_idx == settings.selected_index;
            let style = if is_selected {
                &theme.popup.selected
            } else {
                normal_style
            };

            let item_text = match item {
                Some(FlatItem::SectionHeader(name)) => format!("[{name}]"),
                Some(FlatItem::Setting {
                    section_idx,
                    item_idx,
                }) => {
                    if let Some(section) = settings.sections.get(*section_idx)
                        && let Some(setting) = section.items.get(*item_idx)
                    {
                        // Log the value for selected items
                        if is_selected {
                            tracing::debug!(
                                "Settings render selected: key={}, value={:?}",
                                setting.key,
                                setting.value
                            );
                        }
                        // Format: " Label: value" or " [x] Label" for booleans
                        let value_str = setting.value.display_value();
                        match &setting.value {
                            SettingValue::Bool(b) => {
                                let checkbox = if *b { "[x]" } else { "[ ]" };
                                format!(" {} {}", checkbox, setting.label)
                            }
                            SettingValue::Action(_) => format!(" {}", setting.label),
                            _ => format!(" {}: {}", setting.label, value_str),
                        }
                    } else {
                        " ???".to_string()
                    }
                }
                None => String::new(), // Empty row
            };

            // Draw content (inside border, starting at x+1)
            for (i, ch) in item_text.chars().enumerate() {
                let x = bounds.x + 1 + i as u16;
                if x < bounds.x + bounds.width - 1 {
                    buffer.put_char(x, y, ch, style);
                }
            }

            // Fill remaining space with background
            for x in (bounds.x + 1 + item_text.len() as u16)..(bounds.x + bounds.width - 1) {
                buffer.put_char(x, y, ' ', if item.is_some() { style } else { normal_style });
            }
        }
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
        use reovim_core::highlight::{Color, Style};

        // Gray background for settings/configuration
        let gray = Color::Rgb {
            r: 92,
            g: 99,
            b: 112,
        };
        let fg = Color::Rgb {
            r: 171,
            g: 178,
            b: 191,
        };
        let style = Style::new().fg(fg).bg(gray).bold();

        ctx.register_display(COMPONENT_ID, DisplayInfo::new(" SETTINGS ", " ", style));

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
        let settings_normal = KeymapScope::Component {
            id: COMPONENT_ID,
            mode: EditModeKind::Normal,
        };

        // Space+s to open settings menu (from editor)
        ctx.bind_key_scoped(
            editor_normal,
            keys![Space 's'],
            CommandRef::Registered(command_id::SETTINGS_MENU_OPEN),
        );

        // Settings-specific keybindings (when settings menu is focused)

        // Navigation
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['j'],
            CommandRef::Registered(command_id::SETTINGS_MENU_NEXT),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['k'],
            CommandRef::Registered(command_id::SETTINGS_MENU_PREV),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys![Down],
            CommandRef::Registered(command_id::SETTINGS_MENU_NEXT),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys![Up],
            CommandRef::Registered(command_id::SETTINGS_MENU_PREV),
        );

        // Actions
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys![Space],
            CommandRef::Registered(command_id::SETTINGS_MENU_TOGGLE),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys![Enter],
            CommandRef::Registered(command_id::SETTINGS_MENU_EXECUTE),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['l'],
            CommandRef::Registered(command_id::SETTINGS_MENU_CYCLE_NEXT),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['h'],
            CommandRef::Registered(command_id::SETTINGS_MENU_CYCLE_PREV),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys![Tab],
            CommandRef::Registered(command_id::SETTINGS_MENU_CYCLE_NEXT),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys![(Shift Tab)],
            CommandRef::Registered(command_id::SETTINGS_MENU_CYCLE_PREV),
        );

        // Number increment/decrement
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['+'],
            CommandRef::Registered(command_id::SETTINGS_MENU_INCREMENT),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['-'],
            CommandRef::Registered(command_id::SETTINGS_MENU_DECREMENT),
        );

        // Quick select (1-9)
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['1'],
            CommandRef::Registered(command_id::SETTINGS_MENU_QUICK_1),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['2'],
            CommandRef::Registered(command_id::SETTINGS_MENU_QUICK_2),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['3'],
            CommandRef::Registered(command_id::SETTINGS_MENU_QUICK_3),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['4'],
            CommandRef::Registered(command_id::SETTINGS_MENU_QUICK_4),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['5'],
            CommandRef::Registered(command_id::SETTINGS_MENU_QUICK_5),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['6'],
            CommandRef::Registered(command_id::SETTINGS_MENU_QUICK_6),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['7'],
            CommandRef::Registered(command_id::SETTINGS_MENU_QUICK_7),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['8'],
            CommandRef::Registered(command_id::SETTINGS_MENU_QUICK_8),
        );
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys!['9'],
            CommandRef::Registered(command_id::SETTINGS_MENU_QUICK_9),
        );

        // Close
        ctx.bind_key_scoped(
            settings_normal.clone(),
            keys![Escape],
            CommandRef::Registered(command_id::SETTINGS_MENU_CLOSE),
        );
        ctx.bind_key_scoped(
            settings_normal,
            keys!['q'],
            CommandRef::Registered(command_id::SETTINGS_MENU_CLOSE),
        );
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register the settings menu state
        registry.register(SettingsMenuState::new());

        // Register the plugin window
        registry.register_plugin_window(Arc::new(SettingsPluginWindow));
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        // Register :settings ex-command
        {
            use {
                crate::SettingsMenuOpen,
                reovim_core::{
                    command_line::ExCommandHandler,
                    event_bus::{DynEvent, core_events::RegisterExCommand},
                },
            };

            bus.emit(RegisterExCommand::new(
                "settings",
                ExCommandHandler::ZeroArg {
                    event_constructor: || DynEvent::new(SettingsMenuOpen),
                    description: "Open the settings menu",
                },
            ));
        }

        // Handle RegisterSettingSection events
        {
            let state = Arc::clone(&state);
            bus.subscribe::<RegisterSettingSection, _>(100, move |event, _ctx| {
                state.with_mut::<SettingsMenuState, _, _>(|menu| {
                    menu.register_section(
                        event.id.to_string(),
                        SectionMeta {
                            display_name: event.display_name.to_string(),
                            order: event.order,
                            description: event.description.as_ref().map(|s| s.to_string()),
                        },
                    );
                });
                EventResult::Handled
            });
        }

        // Handle RegisterOption events
        {
            let state = Arc::clone(&state);
            bus.subscribe::<RegisterOption, _>(100, move |event, _ctx| {
                state.with_mut::<SettingsMenuState, _, _>(|menu| {
                    menu.register_option(&event.spec, &event.spec.default);
                });
                EventResult::Handled
            });
        }

        // Handle OptionChanged events - update stored values when options change
        // from other sources (e.g., :set command, profile load)
        {
            let state = Arc::clone(&state);
            bus.subscribe::<OptionChanged, _>(100, move |event, ctx| {
                // Skip changes from settings-menu itself to avoid loops
                if event.source == ChangeSource::SettingsMenu {
                    return EventResult::Handled;
                }

                state.with_mut::<SettingsMenuState, _, _>(|menu| {
                    menu.update_option_value(&event.name, &event.new_value);
                });
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Handle SettingsMenuOpen - open the settings menu
        {
            let state = Arc::clone(&state);
            bus.subscribe::<SettingsMenuOpen, _>(100, move |_event, ctx| {
                state.with_mut::<SettingsMenuState, _, _>(|menu| {
                    // Use default profile config (profile is unused in open())
                    let profile = ProfileConfig::default();
                    menu.open(&profile, "default");
                    // Use reasonable default dimensions; actual bounds come from window_config()
                    menu.calculate_layout(120, 40);
                });
                // Request focus change to settings component
                ctx.emit(RequestFocusChange {
                    target: COMPONENT_ID,
                });
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Handle SettingsMenuClose - close the settings menu
        {
            let state = Arc::clone(&state);
            bus.subscribe::<SettingsMenuClose, _>(100, move |_event, ctx| {
                state.with_mut::<SettingsMenuState, _, _>(|menu| {
                    menu.close();
                });
                // Return focus to editor
                ctx.emit(RequestFocusChange {
                    target: ComponentId::EDITOR,
                });
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Handle SettingsMenuSelectNext - move to next item
        {
            let state = Arc::clone(&state);
            bus.subscribe::<SettingsMenuSelectNext, _>(100, move |_event, ctx| {
                state.with_mut::<SettingsMenuState, _, _>(|menu| {
                    menu.select_next();
                });
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Handle SettingsMenuSelectPrev - move to previous item
        {
            let state = Arc::clone(&state);
            bus.subscribe::<SettingsMenuSelectPrev, _>(100, move |_event, ctx| {
                state.with_mut::<SettingsMenuState, _, _>(|menu| {
                    menu.select_prev();
                });
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Handle SettingsMenuToggle - toggle boolean settings
        {
            let state = Arc::clone(&state);
            bus.subscribe::<SettingsMenuToggle, _>(100, move |_event, ctx| {
                tracing::debug!("SettingsMenuToggle event received");
                let change = state.with_mut::<SettingsMenuState, _, _>(|menu| {
                    tracing::debug!("Before toggle - selected_index: {}", menu.selected_index);
                    if let Some(item) = menu.selected_item() {
                        tracing::debug!("Selected item: {} = {:?}", item.key, item.value);
                    }
                    let result = menu.toggle_selected();
                    if let Some(item) = menu.selected_item() {
                        tracing::debug!("After toggle: {} = {:?}", item.key, item.value);
                    }
                    result
                });
                if let Some(Some(change)) = change {
                    tracing::info!(
                        "Settings toggle: {} changed from {:?} to {:?}",
                        change.key,
                        change.old_value,
                        change.new_value
                    );
                    ctx.emit(OptionChanged::new(
                        change.key,
                        change.old_value,
                        change.new_value,
                        ChangeSource::SettingsMenu,
                    ));
                } else {
                    tracing::debug!(
                        "Settings toggle: no change (not a boolean or no item selected)"
                    );
                }
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Handle SettingsMenuCycleNext - cycle to next value
        {
            let state = Arc::clone(&state);
            bus.subscribe::<SettingsMenuCycleNext, _>(100, move |_event, ctx| {
                let change =
                    state.with_mut::<SettingsMenuState, _, _>(|menu| menu.cycle_next_selected());
                if let Some(Some(change)) = change {
                    tracing::debug!("Settings cycle next: {} changed", change.key);
                    ctx.emit(OptionChanged::new(
                        change.key,
                        change.old_value,
                        change.new_value,
                        ChangeSource::SettingsMenu,
                    ));
                }
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Handle SettingsMenuCyclePrev - cycle to previous value
        {
            let state = Arc::clone(&state);
            bus.subscribe::<SettingsMenuCyclePrev, _>(100, move |_event, ctx| {
                let change =
                    state.with_mut::<SettingsMenuState, _, _>(|menu| menu.cycle_prev_selected());
                if let Some(Some(change)) = change {
                    tracing::debug!("Settings cycle prev: {} changed", change.key);
                    ctx.emit(OptionChanged::new(
                        change.key,
                        change.old_value,
                        change.new_value,
                        ChangeSource::SettingsMenu,
                    ));
                }
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Handle SettingsMenuIncrement - increment number value
        {
            let state = Arc::clone(&state);
            bus.subscribe::<SettingsMenuIncrement, _>(100, move |_event, ctx| {
                let change =
                    state.with_mut::<SettingsMenuState, _, _>(|menu| menu.increment_selected());
                if let Some(Some(change)) = change {
                    tracing::debug!("Settings increment: {} changed", change.key);
                    ctx.emit(OptionChanged::new(
                        change.key,
                        change.old_value,
                        change.new_value,
                        ChangeSource::SettingsMenu,
                    ));
                }
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Handle SettingsMenuDecrement - decrement number value
        {
            let state = Arc::clone(&state);
            bus.subscribe::<SettingsMenuDecrement, _>(100, move |_event, ctx| {
                let change =
                    state.with_mut::<SettingsMenuState, _, _>(|menu| menu.decrement_selected());
                if let Some(Some(change)) = change {
                    tracing::debug!("Settings decrement: {} changed", change.key);
                    ctx.emit(OptionChanged::new(
                        change.key,
                        change.old_value,
                        change.new_value,
                        ChangeSource::SettingsMenu,
                    ));
                }
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Handle SettingsMenuExecuteAction - execute selected action or toggle
        {
            let state = Arc::clone(&state);
            bus.subscribe::<SettingsMenuExecuteAction, _>(100, move |_event, ctx| {
                // First, try to get the action type
                let action = state
                    .with::<SettingsMenuState, _, _>(|menu| menu.get_selected_action())
                    .flatten();

                if let Some(action_type) = action {
                    // Handle profile actions
                    match action_type {
                        ActionType::SaveProfile => {
                            tracing::info!("Settings: Save profile action triggered");
                            // TODO: Emit profile save event
                            state.with_mut::<SettingsMenuState, _, _>(|menu| {
                                menu.set_message(
                                    "Profile save not yet implemented".to_string(),
                                    MessageKind::Info,
                                );
                            });
                        }
                        ActionType::LoadProfile => {
                            tracing::info!("Settings: Load profile action triggered");
                            // TODO: Emit profile load event
                            state.with_mut::<SettingsMenuState, _, _>(|menu| {
                                menu.set_message(
                                    "Profile load not yet implemented".to_string(),
                                    MessageKind::Info,
                                );
                            });
                        }
                        ActionType::ResetToDefault => {
                            tracing::info!("Settings: Reset to default action triggered");
                            // TODO: Emit reset event
                            state.with_mut::<SettingsMenuState, _, _>(|menu| {
                                menu.set_message(
                                    "Reset to default not yet implemented".to_string(),
                                    MessageKind::Info,
                                );
                            });
                        }
                    }
                    ctx.request_render();
                } else {
                    // Not an action - try toggle for booleans
                    let change =
                        state.with_mut::<SettingsMenuState, _, _>(|menu| menu.toggle_selected());
                    if let Some(Some(change)) = change {
                        ctx.emit(OptionChanged::new(
                            change.key,
                            change.old_value,
                            change.new_value,
                            ChangeSource::SettingsMenu,
                        ));
                        ctx.request_render();
                    }
                }
                EventResult::Handled
            });
        }
    }
}
