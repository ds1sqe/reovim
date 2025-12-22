//! Code completion plugin for reovim
//!
//! This plugin provides auto-completion functionality:
//! - Trigger completion popup
//! - Navigate suggestions with next/prev
//! - Confirm selection or dismiss
//!
//! # Architecture
//!
//! This plugin is fully self-contained:
//! - Defines its own command IDs
//! - Manages its own state via `PluginStateRegistry`
//! - Renders via `OverlayRenderer` trait
//! - Communicates via `EventBus` events

mod commands;
mod completion;

use std::any::TypeId;

use {
    commands::{
        CompletionConfirmCommand, CompletionDismissCommand, CompletionNextCommand,
        CompletionPrevCommand, CompletionTriggerCommand,
    },
    completion::CompletionState,
    reovim_core::{
        bind::CommandRef,
        command::id::CommandId,
        component::RenderContext,
        frame::FrameBuffer,
        keys,
        overlay::{OverlayBounds, OverlayRenderer},
        plugin::{Plugin, PluginContext, PluginId},
    },
};

// Re-export key types for external use
pub use completion::{
    CompletionConfirmEvent, CompletionDismissEvent, CompletionEngine, CompletionItem,
    CompletionSelectNextEvent, CompletionSelectPrevEvent, CompletionSource, CompletionTriggerEvent,
};

/// Plugin-local command IDs
pub mod command_id {
    use super::CommandId;

    pub const COMPLETION_TRIGGER: CommandId = CommandId::new("completion_trigger");
    pub const COMPLETION_NEXT: CommandId = CommandId::new("completion_next");
    pub const COMPLETION_PREV: CommandId = CommandId::new("completion_prev");
    pub const COMPLETION_CONFIRM: CommandId = CommandId::new("completion_confirm");
    pub const COMPLETION_DISMISS: CommandId = CommandId::new("completion_dismiss");
}

/// Completion popup overlay
///
/// Stateless overlay that accesses `CompletionState` through `RenderContext`.
pub struct CompletionOverlay;

impl OverlayRenderer for CompletionOverlay {
    fn id(&self) -> &'static str {
        "completion"
    }

    fn z_order(&self) -> u16 {
        200
    }

    fn is_visible(&self, ctx: &RenderContext<'_>) -> bool {
        ctx.state()
            .and_then(|s| {
                s.plugin_state
                    .with::<CompletionState, _, _>(|cs| cs.active && !cs.items.is_empty())
            })
            .unwrap_or(false)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(&self, buffer: &mut FrameBuffer, ctx: &RenderContext<'_>) {
        let Some(state) = ctx.state() else { return };
        let Some(completion) = state
            .plugin_state
            .with::<CompletionState, _, _>(Clone::clone)
        else {
            return;
        };
        let theme = ctx.theme;

        let items = &completion.items;
        if items.is_empty() {
            return;
        }

        let cursor_x = completion.start_col;
        let cursor_y = completion.start_row;

        let max_items = 10.min(items.len());
        let max_label_width = items
            .iter()
            .take(max_items)
            .map(|i| i.label.len())
            .max()
            .unwrap_or(10);
        let popup_width = (max_label_width + 2).min(40) as u16;

        let prefix_len = completion.prefix.len() as u16;
        let popup_x = cursor_x
            .saturating_sub(prefix_len)
            .min(ctx.screen_width.saturating_sub(popup_width));
        let popup_y = cursor_y + 1;

        for (idx, item) in items.iter().take(max_items).enumerate() {
            let is_selected = idx == completion.selected_index;
            let style = if is_selected {
                &theme.popup.selected
            } else {
                &theme.popup.normal
            };

            let row = popup_y + idx as u16;
            if row >= ctx.screen_height.saturating_sub(1) {
                break;
            }

            buffer.put_char(popup_x, row, ' ', style);
            let label_chars: Vec<char> = item.label.chars().collect();
            for (i, &ch) in label_chars
                .iter()
                .take(popup_width as usize - 2)
                .enumerate()
            {
                buffer.put_char(popup_x + 1 + i as u16, row, ch, style);
            }
            for i in label_chars.len().min(popup_width as usize - 2)..popup_width as usize - 1 {
                buffer.put_char(popup_x + 1 + i as u16, row, ' ', style);
            }
            buffer.put_char(popup_x + popup_width - 1, row, ' ', style);
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn bounds(&self, ctx: &RenderContext<'_>) -> OverlayBounds {
        ctx.state()
            .and_then(|s| {
                s.plugin_state.with::<CompletionState, _, _>(|completion| {
                    if completion.items.is_empty() {
                        return OverlayBounds::default();
                    }

                    let cursor_x = completion.start_col;
                    let cursor_y = completion.start_row;
                    let max_items = 10.min(completion.items.len());
                    let popup_width = completion
                        .items
                        .iter()
                        .take(max_items)
                        .map(|i| i.label.len())
                        .max()
                        .map_or(12, |w| (w + 2).min(40))
                        as u16;
                    let prefix_len = completion.prefix.len() as u16;
                    let popup_x = cursor_x.saturating_sub(prefix_len);
                    let popup_y = cursor_y + 1;

                    OverlayBounds::new(popup_x, popup_y, popup_width, max_items as u16)
                })
            })
            .unwrap_or_default()
    }

    fn captures_input(&self, _ctx: &RenderContext<'_>) -> bool {
        false
    }
}

/// Code completion plugin
///
/// Provides auto-completion:
/// - Trigger completion popup
/// - Navigate suggestions
/// - Confirm/dismiss
pub struct CompletionPlugin;

impl Plugin for CompletionPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:completion")
    }

    fn name(&self) -> &'static str {
        "Completion"
    }

    fn description(&self) -> &'static str {
        "Auto-completion popup"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register commands
        let _ = ctx.register_command(CompletionTriggerCommand);
        let _ = ctx.register_command(CompletionNextCommand);
        let _ = ctx.register_command(CompletionPrevCommand);
        let _ = ctx.register_command(CompletionConfirmCommand);
        let _ = ctx.register_command(CompletionDismissCommand);

        // Register overlay
        ctx.register_overlay(CompletionOverlay);

        // Register keybindings
        // Ctrl-Space to trigger completion in insert mode
        use reovim_core::bind::KeymapScope;
        let insert_mode = KeymapScope::editor_insert();

        ctx.bind_key_scoped(
            insert_mode.clone(),
            keys![(Ctrl Space)],
            CommandRef::Registered(command_id::COMPLETION_TRIGGER),
        );

        // Completion navigation keybindings (when completion is active)
        // Note: These are typically handled contextually when completion is visible
        // The runtime checks if completion is active before dispatching these commands
    }
}
