#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Bufferline module for reovim.
//!
//! Provides pin/unpin/close commands for the buffer tab bar. The buffer
//! list itself is fetched client-side via `list_buffers()` gRPC — this
//! module only manages server-owned pin state and a thin bridge that
//! emits pin changes to clients.

pub mod bridge;
pub mod commands;
pub mod ids;
pub mod state;

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_input::KeybindingStore,
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
};

const KIND: &str = "bufferline";

/// Bufferline module.
///
/// Registers pin commands, keybindings, and a thin pin-state bridge.
/// Buffer list data flows client-side (no server-side aggregation).
pub struct BufferlineModule;

impl BufferlineModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
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
        Version::new(0, 2, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register pin-state bridge.
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(bridge::PinBridge);

        // Register command handlers.
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::command_handlers() {
            command_store.add(handler);
        }

        // Register keybindings.
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
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
