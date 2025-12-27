//! Which-key plugin for reovim
//!
//! Shows available keybindings in a popup panel when pressing `?` after a prefix key.
//! For example, pressing `g?` shows all bindings starting with `g`.

mod commands;
mod filter;
mod saturator;
mod state;

pub use commands::{WhichKeyBackspace, WhichKeyClose, WhichKeyOpen, WhichKeyTrigger};

use std::sync::Arc;

use {
    arc_swap::ArcSwap,
    reovim_core::{
        bind::{CommandRef, KeymapScope, SubModeKind},
        command::CommandId,
        event::InnerEvent,
        event_bus::{EventBus, EventResult, PluginBackspace, PluginTextInput, RequestModeChange},
        frame::FrameBuffer,
        highlight::Theme,
        keys,
        keystroke::Keystroke,
        modd::{ComponentId, EditMode, ModeState, SubMode},
        plugin::{
            EditorContext, PanelPosition, Plugin, PluginContext, PluginId, PluginStateRegistry,
            PluginWindow, Rect, WindowConfig,
        },
    },
    tokio::sync::mpsc,
};

/// Component ID for the which-key plugin
pub const COMPONENT_ID: ComponentId = ComponentId("which_key");

use crate::{
    saturator::spawn_whichkey_saturator,
    state::{WhichKeyCacheHolder, WhichKeyState},
};

/// Which-key plugin for showing available keybindings
pub struct WhichKeyPlugin;

impl Plugin for WhichKeyPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:which-key")
    }

    fn name(&self) -> &'static str {
        "Which-Key"
    }

    fn description(&self) -> &'static str {
        "Shows available keybindings after pressing a prefix key"
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register commands
        let _ = ctx.register_command(WhichKeyClose);
        let _ = ctx.register_command(WhichKeyBackspace);

        // Register ? keybindings for common prefixes in editor normal mode
        // Each binding triggers which-key with the appropriate prefix
        let editor_normal = KeymapScope::editor_normal();

        // Standalone ? (shows all bindings)
        ctx.keymap_mut().bind_scoped(
            editor_normal.clone(),
            keys!['?'],
            CommandRef::Inline(WhichKeyTrigger::arc(keys![])),
        );

        // Common prefix + ? bindings
        // These allow g?, z?, Space?, d?, y?, c? to show context-specific bindings
        let common_prefixes = [
            keys!['g'],       // g-prefix commands
            keys!['z'],       // fold commands
            keys![Space],     // leader key
            keys![Space 'f'], // find prefix
            keys![Space 'b'], // buffer prefix
            keys![Space 'w'], // window prefix
            keys!['d'],       // delete operator
            keys!['y'],       // yank operator
            keys!['c'],       // change operator
        ];

        for prefix in common_prefixes {
            let mut full_keys = prefix.clone();
            full_keys.push(Keystroke::char('?'));
            ctx.keymap_mut().bind_scoped(
                editor_normal.clone(),
                full_keys,
                CommandRef::Inline(WhichKeyTrigger::arc(prefix)),
            );
        }

        // Define which-key interactor scope for keybindings
        let which_key_interactor = KeymapScope::SubMode(SubModeKind::Interactor(COMPONENT_ID));

        // Bind Escape to close the which-key panel (in interactor mode)
        ctx.keymap_mut().bind_scoped(
            which_key_interactor.clone(),
            keys![Escape],
            CommandRef::Registered(CommandId::new("which_key_close")),
        );

        // Bind Backspace to remove last filter key (in interactor mode)
        ctx.keymap_mut().bind_scoped(
            which_key_interactor,
            keys![Backspace],
            CommandRef::Registered(CommandId::new("which_key_backspace")),
        );
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Create ArcSwap cache (saturator spawned in boot() with event_tx)
        let cache = Arc::new(ArcSwap::new(Arc::new(state::WhichKeyCache::default())));

        // Register cache for boot() to access
        registry.register(WhichKeyCacheHolder {
            cache: cache.clone(),
        });
        registry.register_plugin_window(Arc::new(WhichKeyPluginWindow { cache }));
    }

    fn boot(
        &self,
        _bus: &EventBus,
        state: Arc<PluginStateRegistry>,
        event_tx: Option<mpsc::Sender<InnerEvent>>,
    ) {
        tracing::info!("WhichKeyPlugin: boot() called");
        // Get cache from init_state
        let cache = state
            .with::<WhichKeyCacheHolder, _, _>(|h| h.cache.clone())
            .expect("WhichKeyCacheHolder must be registered");

        // Spawn background saturator with event_tx
        let Some(tx) = event_tx else {
            tracing::warn!("Which-key plugin requires event_tx but none was provided");
            return;
        };

        // Get KeyMap and CommandRegistry from state
        let Some(keymap) = state.keymap() else {
            tracing::warn!("Which-key plugin requires KeyMap but none was registered");
            return;
        };
        let Some(command_registry) = state.command_registry() else {
            tracing::warn!("Which-key plugin requires CommandRegistry but none was registered");
            return;
        };

        tracing::info!("WhichKeyPlugin: spawning saturator");
        let saturator = spawn_whichkey_saturator(cache.clone(), tx, keymap, command_registry);

        // Register full state with saturator handle
        state.register(WhichKeyState { cache, saturator });
        tracing::info!("WhichKeyPlugin: WhichKeyState registered");
    }

    #[allow(clippy::too_many_lines)]
    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        // Subscribe to WhichKeyOpen
        let state_clone = Arc::clone(&state);
        bus.subscribe::<WhichKeyOpen, _>(100, move |event, ctx| {
            tracing::info!(
                "WhichKeyPlugin: received WhichKeyOpen event, prefix={:?}",
                event.prefix
            );
            let prefix = event.prefix.clone();
            let scope = KeymapScope::editor_normal();

            // Update cache with open state
            state_clone.with::<WhichKeyState, _, _>(|wk| {
                let mut cache = (*wk.cache.load_full()).clone();
                cache.open(prefix.clone(), scope.clone());
                wk.cache.store(Arc::new(cache));

                // Send initial filter request to saturator
                tracing::info!(
                    "WhichKeyPlugin: sending request to saturator with prefix={:?}",
                    prefix
                );
                match wk.saturator.tx.try_send(saturator::WhichKeyRequest {
                    pending_keys: prefix,
                    scope,
                }) {
                    Ok(()) => tracing::info!("WhichKeyPlugin: request sent to saturator"),
                    Err(e) => tracing::warn!("WhichKeyPlugin: failed to send to saturator: {}", e),
                }
            });

            // Request mode change to which-key interactor sub-mode
            // This ensures keys are routed to this plugin via PluginTextInput
            let mode = ModeState::with_interactor_id_sub_mode(
                ComponentId::EDITOR,
                EditMode::Normal,
                SubMode::Interactor(COMPONENT_ID),
            );
            ctx.emit(RequestModeChange { mode });

            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to PluginTextInput - handle keys typed while panel is visible
        let state_clone = Arc::clone(&state);
        bus.subscribe::<PluginTextInput, _>(100, move |event, ctx| {
            // Only handle if we're the target
            if event.target != COMPONENT_ID {
                return EventResult::NotHandled;
            }

            tracing::info!("WhichKeyPlugin: received PluginTextInput c={}", event.c);

            // Convert char to keystroke and update filter
            let keystroke = Keystroke::char(event.c);
            state_clone.with::<WhichKeyState, _, _>(|wk| {
                let mut cache = (*wk.cache.load_full()).clone();
                cache.push_key(keystroke);

                // Send updated filter to saturator
                let filter = cache.current_filter.clone();
                let scope = cache.scope.clone();
                wk.cache.store(Arc::new(cache));

                tracing::info!("WhichKeyPlugin: updated filter to {:?}", filter);
                let _ = wk.saturator.tx.try_send(saturator::WhichKeyRequest {
                    pending_keys: filter,
                    scope,
                });
            });

            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to PluginBackspace - same as WhichKeyBackspace
        let state_clone = Arc::clone(&state);
        bus.subscribe::<PluginBackspace, _>(100, move |event, ctx| {
            // Only handle if we're the target
            if event.target != COMPONENT_ID {
                return EventResult::NotHandled;
            }

            tracing::info!("WhichKeyPlugin: received PluginBackspace");

            state_clone.with::<WhichKeyState, _, _>(|wk| {
                let mut cache = (*wk.cache.load_full()).clone();
                if cache.pop_key() {
                    // Key was removed, update filter
                    let filter = cache.current_filter.clone();
                    let scope = cache.scope.clone();
                    wk.cache.store(Arc::new(cache));

                    tracing::info!("WhichKeyPlugin: updated filter to {:?}", filter);
                    let _ = wk.saturator.tx.try_send(saturator::WhichKeyRequest {
                        pending_keys: filter,
                        scope,
                    });
                }
            });

            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to WhichKeyBackspace
        let state_clone = Arc::clone(&state);
        bus.subscribe::<WhichKeyBackspace, _>(100, move |_event, ctx| {
            tracing::info!("WhichKeyPlugin: received WhichKeyBackspace");

            state_clone.with::<WhichKeyState, _, _>(|wk| {
                let mut cache = (*wk.cache.load_full()).clone();
                if cache.pop_key() {
                    // Key was removed, update filter
                    let filter = cache.current_filter.clone();
                    let scope = cache.scope.clone();
                    wk.cache.store(Arc::new(cache));

                    tracing::info!("WhichKeyPlugin: updated filter to {:?}", filter);
                    let _ = wk.saturator.tx.try_send(saturator::WhichKeyRequest {
                        pending_keys: filter,
                        scope,
                    });
                }
            });

            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to WhichKeyClose
        let state_clone = Arc::clone(&state);
        bus.subscribe::<WhichKeyClose, _>(100, move |_event, ctx| {
            tracing::info!("WhichKeyPlugin: received WhichKeyClose");

            state_clone.with::<WhichKeyState, _, _>(|wk| {
                let mut cache = (*wk.cache.load_full()).clone();
                cache.close();
                wk.cache.store(Arc::new(cache));
            });

            // Return to normal editor mode
            let mode = ModeState::normal();
            ctx.emit(RequestModeChange { mode });

            ctx.request_render();
            EventResult::Handled
        });
    }
}

/// Plugin window for rendering the which-key panel
pub struct WhichKeyPluginWindow {
    cache: Arc<ArcSwap<state::WhichKeyCache>>,
}

impl PluginWindow for WhichKeyPluginWindow {
    fn window_config(
        &self,
        _state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        // Lock-free read from ArcSwap cache
        let wk_cache = self.cache.load();
        if !wk_cache.visible {
            return None;
        }

        let panel_height = 6;
        let (x, y, w, h) = ctx.side_panel(PanelPosition::Bottom, panel_height);

        Some(WindowConfig {
            bounds: Rect::new(x, y, w, h),
            z_order: 600, // Panel level
            visible: true,
        })
    }

    fn render(
        &self,
        _state: &Arc<PluginStateRegistry>,
        _ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        bounds: Rect,
        theme: &Theme,
    ) {
        // Lock-free read from ArcSwap cache
        let wk_cache = self.cache.load();

        // Render border
        let border_style = &theme.popup.border;

        // Top border
        buffer.put_char(bounds.x, bounds.y, '╭', border_style);
        for x in (bounds.x + 1)..(bounds.x + bounds.width - 1) {
            buffer.put_char(x, bounds.y, '─', border_style);
        }
        buffer.put_char(bounds.x + bounds.width - 1, bounds.y, '╮', border_style);

        // Title
        let title = " Which Key ";
        #[allow(clippy::cast_possible_truncation)] // title is short
        let title_x = bounds.x + (bounds.width.saturating_sub(title.len() as u16)) / 2;
        buffer.write_str(title_x, bounds.y, title, border_style);

        // Side borders
        for y in (bounds.y + 1)..(bounds.y + bounds.height - 1) {
            buffer.put_char(bounds.x, y, '│', border_style);
            buffer.put_char(bounds.x + bounds.width - 1, y, '│', border_style);
        }

        // Bottom border
        buffer.put_char(bounds.x, bounds.y + bounds.height - 1, '╰', border_style);
        for x in (bounds.x + 1)..(bounds.x + bounds.width - 1) {
            buffer.put_char(x, bounds.y + bounds.height - 1, '─', border_style);
        }
        buffer.put_char(
            bounds.x + bounds.width - 1,
            bounds.y + bounds.height - 1,
            '╯',
            border_style,
        );

        // Render bindings
        let content_x = bounds.x + 2;
        let content_y = bounds.y + 1;
        let content_width = bounds.width.saturating_sub(4);
        let content_height = bounds.height.saturating_sub(2);

        let normal_style = &theme.popup.normal;

        // Fill content area with background to clear any artifacts
        for y in content_y..(content_y + content_height) {
            // Fill entire inner width (between borders)
            for x in (bounds.x + 1)..(bounds.x + 1 + content_width + 2) {
                buffer.put_char(x, y, ' ', normal_style);
            }
        }

        // For now, just show a placeholder message if no bindings
        if wk_cache.bindings.is_empty() {
            let msg = "Press ? after a prefix key to see bindings";
            buffer.write_str(content_x, content_y, msg, normal_style);
        } else {
            // Render bindings in columns
            for (row, entry) in wk_cache.bindings.iter().enumerate() {
                if row >= content_height as usize {
                    break;
                }

                #[allow(clippy::cast_possible_truncation)] // row limited by content_height
                let y = content_y + row as u16;
                let key_str = entry
                    .suffix
                    .render(reovim_core::keystroke::KeyNotationFormat::Vim);
                let desc = &entry.description;

                // Key in highlight color
                let key_width = buffer.write_str(content_x, y, &key_str, &theme.popup.selected);

                // Description in normal color
                buffer.write_str(content_x + key_width + 2, y, desc, normal_style);
            }
        }
    }
}
