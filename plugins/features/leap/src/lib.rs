//! Leap motion plugin for reovim
//!
//! This plugin provides two-character jump navigation (like leap.nvim/hop.nvim).
//!
//! # Architecture Note
//!
//! Commands emit `EventBus` events that are handled by the runtime.
//! Uses `PluginStateRegistry` for state management.

mod commands;
mod events;
mod handlers;
mod state;

use std::{any::TypeId, sync::Arc};

use reovim_core::{
    bind::{CommandRef, KeymapScope},
    display::{DisplayInfo, SubModeKey},
    event_bus::{EventBus, EventResult},
    frame::FrameBuffer,
    highlight::{Style, Theme},
    keys,
    plugin::{
        EditorContext, Plugin, PluginContext, PluginId, PluginStateRegistry, PluginWindow, Rect,
        WindowConfig,
    },
    ui_component::ComponentId,
};

use reovim_sys::style::Color;

pub use {
    commands::{
        LEAP_BACKWARD, LEAP_CANCEL, LEAP_FORWARD, LeapBackwardCommand, LeapCancelCommand,
        LeapForwardCommand,
    },
    events::{
        LeapCancelEvent, LeapFirstCharEvent, LeapJumpEvent, LeapMatchesFoundEvent,
        LeapSecondCharEvent, LeapSelectLabelEvent, LeapStartEvent,
    },
    handlers::{LeapStartHandler, leap_start_result},
    state::{LeapDirection, LeapMatch, LeapPhase, LeapState, find_matches, generate_labels},
};

/// Leap styles for rendering
///
/// Since leap styles were removed from core theme, this plugin defines its own.
#[derive(Debug, Clone)]
pub struct LeapStyles {
    pub label: Style,
    pub match_highlight: Style,
}

impl Default for LeapStyles {
    fn default() -> Self {
        Self {
            label: Style::new().fg(Color::Black).bg(Color::Yellow).bold(),
            match_highlight: Style::new().fg(Color::Magenta).bold(),
        }
    }
}

/// Plugin window for leap labels
pub struct LeapPluginWindow;

impl PluginWindow for LeapPluginWindow {
    fn window_config(
        &self,
        state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        let is_showing = state
            .with::<LeapState, _, _>(|leap| leap.is_showing_labels())
            .unwrap_or(false);

        if !is_showing {
            return None;
        }

        // Leap renders labels across the entire screen
        Some(WindowConfig {
            bounds: Rect::new(0, 0, ctx.screen_width, ctx.screen_height),
            z_order: 100, // Same level as editor windows
            visible: true,
        })
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(
        &self,
        state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        _bounds: Rect,
        _theme: &Theme,
    ) {
        // Use plugin's own leap styles
        let styles = LeapStyles::default();
        let label_style = &styles.label;

        state.with::<LeapState, _, _>(|leap| {
            for m in &leap.matches {
                let screen_x = m.col;
                let screen_y = m.line;

                if screen_y >= ctx.screen_height.saturating_sub(1) {
                    continue;
                }

                for (i, ch) in m.label.chars().enumerate() {
                    let x = screen_x + i as u16;
                    if x < buffer.width() {
                        buffer.put_char(x, screen_y, ch, label_style);
                    }
                }
            }
        });
    }
}

/// Component ID for leap (used in sub-mode)
pub const COMPONENT_ID: ComponentId = ComponentId("leap");

/// Leap motion plugin
///
/// Provides two-character jump navigation:
/// - s/S for forward/backward leap
/// - Character-based label selection
pub struct LeapPlugin;

impl Plugin for LeapPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:leap")
    }

    fn name(&self) -> &'static str {
        "Leap"
    }

    fn description(&self) -> &'static str {
        "Two-character jump navigation"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register sub-mode display info
        ctx.register_sub_mode_display(
            SubModeKey::Interactor(COMPONENT_ID),
            DisplayInfo::new(" LEAP ", "\u{f0168} "),
        );

        // Register commands
        let _ = ctx.register_command(LeapForwardCommand);
        let _ = ctx.register_command(LeapBackwardCommand);
        let _ = ctx.register_command(LeapCancelCommand);

        // Register keybindings (previously in core's bind/mod.rs)
        ctx.bind_key_scoped(
            KeymapScope::editor_normal(),
            keys!['s'],
            CommandRef::Registered(LEAP_FORWARD),
        );
        ctx.bind_key_scoped(
            KeymapScope::editor_normal(),
            keys!['S'],
            CommandRef::Registered(LEAP_BACKWARD),
        );
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        registry.register(LeapState::new());
        // Register the plugin window
        registry.register_plugin_window(Arc::new(LeapPluginWindow));
        tracing::debug!("LeapPlugin: initialized state in registry");
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        let state_clone = Arc::clone(&state);
        bus.subscribe::<LeapStartEvent, _>(100, move |event, _ctx| {
            tracing::trace!(
                direction = ?event.direction,
                operator = ?event.operator,
                "LeapPlugin: leap mode started via event bus"
            );

            state_clone.with_mut::<LeapState, _, _>(|leap_state| {
                leap_state.start(event.direction, event.operator, event.count);
            });

            EventResult::Handled
        });

        bus.subscribe::<LeapJumpEvent, _>(100, |event, _ctx| {
            tracing::trace!(
                from = ?event.from,
                to = ?event.to,
                direction = ?event.direction,
                "LeapPlugin: jump occurred via event bus"
            );
            EventResult::Handled
        });

        bus.subscribe::<LeapMatchesFoundEvent, _>(100, |event, _ctx| {
            tracing::trace!(
                match_count = event.match_count,
                pattern = %event.pattern,
                "LeapPlugin: matches found via event bus"
            );
            EventResult::Handled
        });

        let state_clone2 = Arc::clone(&state);
        bus.subscribe::<LeapCancelEvent, _>(100, move |_event, _ctx| {
            tracing::trace!("LeapPlugin: leap cancelled via event bus");

            state_clone2.with_mut::<LeapState, _, _>(|leap_state| {
                leap_state.reset();
            });

            EventResult::Handled
        });

        tracing::debug!("LeapPlugin: subscribed to leap events via event bus");
    }
}
