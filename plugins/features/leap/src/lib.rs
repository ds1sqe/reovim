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
    component::RenderContext,
    display::{DisplayInfo, SubModeKey},
    event_bus::{EventBus, EventResult},
    frame::FrameBuffer,
    highlight::Style,
    keys,
    overlay::{OverlayBounds, OverlayRenderer},
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
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

/// Leap labels overlay
///
/// Stateless overlay that accesses `LeapState` through `RenderContext`.
pub struct LeapOverlay;

impl OverlayRenderer for LeapOverlay {
    fn id(&self) -> &'static str {
        "leap"
    }

    fn z_order(&self) -> u16 {
        100
    }

    fn is_visible(&self, ctx: &RenderContext<'_>) -> bool {
        ctx.state
            .and_then(|s| {
                s.plugin_state
                    .with::<LeapState, _, _>(|leap| leap.is_showing_labels())
            })
            .unwrap_or(false)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(&self, buffer: &mut FrameBuffer, ctx: &RenderContext<'_>) {
        let Some(state) = ctx.state else { return };

        // Use plugin's own leap styles
        let styles = LeapStyles::default();
        let label_style = &styles.label;

        state.plugin_state.with::<LeapState, _, _>(|leap| {
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

    fn bounds(&self, _ctx: &RenderContext<'_>) -> OverlayBounds {
        OverlayBounds::default()
    }

    fn captures_input(&self, _ctx: &RenderContext<'_>) -> bool {
        true
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

        // Register overlay
        ctx.register_overlay(LeapOverlay);
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        registry.register(LeapState::new());
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
