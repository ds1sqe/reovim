#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Bufferline module for reovim.
//!
//! Provides a tab/buffer bar showing open buffers with active highlight,
//! modified markers, filetype metadata, and per-buffer diagnostic counts.
//! Follows the extension-state-bridge pattern with `ExtensionScope::Shared`.

pub mod bridge;
pub mod commands;
pub mod ids;
pub mod service;
pub mod state;

use std::sync::Arc;

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_input::KeybindingStore,
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{
        EventResult, KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId,
        ProbeResult, Subscription, Version,
        events::kernel::{BufferClosed, BufferCreated, BufferModified, BufferSaved, priority},
    },
};

use service::{BufferEntrySnapshot, BufferListService};

const KIND: &str = "bufferline";

/// Bufferline module.
///
/// Maintains a buffer list service populated via `EventBus` subscriptions
/// and a bridge that serializes the buffer list to JSON for TUI/Web clients.
pub struct BufferlineModule {
    /// Active `EventBus` subscriptions (RAII — drop unsubscribes).
    subscriptions: Vec<Subscription>,
}

impl BufferlineModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
        }
    }
}

impl Default for BufferlineModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for BufferlineModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "Bufferline"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register bridge.
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(bridge::BufferlineBridge);

        // Ensure BufferListService exists.
        let _svc = ctx.services.get_or_create::<BufferListService>();

        // Register command handlers.
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::command_handlers() {
            command_store.add(handler);
        }

        // Register keybindings.
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        // Subscribe to buffer lifecycle events.
        let bus = Arc::clone(&ctx.kernel.event_bus);
        let services = Arc::clone(&ctx.services);

        // BufferCreated: add entry to the buffer list.
        let svc_ref = Arc::clone(&services);
        let sub_created =
            bus.subscribe_with_context::<BufferCreated, _>(priority::NORMAL, move |event, _ctx| {
                if let Some(svc) = svc_ref.get::<BufferListService>() {
                    svc.add(BufferEntrySnapshot {
                        id: event.buffer_id,
                        name: String::from("[No Name]"),
                        path: None,
                        modified: false,
                        filetype: None,
                    });
                }
                EventResult::Handled
            });
        self.subscriptions.push(sub_created);

        // BufferClosed: remove entry.
        let svc_ref = Arc::clone(&services);
        let sub_closed =
            bus.subscribe_with_context::<BufferClosed, _>(priority::NORMAL, move |event, _ctx| {
                if let Some(svc) = svc_ref.get::<BufferListService>() {
                    svc.remove(event.buffer_id);
                }
                EventResult::Handled
            });
        self.subscriptions.push(sub_closed);

        // BufferModified: mark as modified.
        let svc_ref = Arc::clone(&services);
        let sub_modified = bus.subscribe_with_context::<BufferModified, _>(
            priority::NORMAL,
            move |event, _ctx| {
                if let Some(svc) = svc_ref.get::<BufferListService>() {
                    svc.set_modified(event.buffer_id, true);
                }
                EventResult::Handled
            },
        );
        self.subscriptions.push(sub_modified);

        // BufferSaved: update path, filetype, and clear modified.
        let svc_ref = Arc::clone(&services);
        let sub_saved =
            bus.subscribe_with_context::<BufferSaved, _>(priority::NORMAL, move |event, _ctx| {
                if let Some(svc) = svc_ref.get::<BufferListService>() {
                    svc.set_path(event.buffer_id, event.path.clone());
                    svc.set_modified(event.buffer_id, false);
                }
                EventResult::Handled
            });
        self.subscriptions.push(sub_saved);

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        self.subscriptions.clear();
        Ok(())
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND]
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            KeybindingRegistration::new("<leader>bp", ids::PIN_BUFFER)
                .with_modes(&["normal"])
                .with_description("Toggle buffer pin"),
            KeybindingRegistration::new("<leader>bu", ids::UNPIN_BUFFER)
                .with_modes(&["normal"])
                .with_description("Unpin buffer"),
            KeybindingRegistration::new("<leader>bc", ids::CLOSE_BUFFER)
                .with_modes(&["normal"])
                .with_description("Close buffer"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(BufferlineModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
