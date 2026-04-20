#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Format module for reovim.
//!
//! Provides format-on-save via `BufferWillSave` subscription, `:Format` and
//! `:FormatRange` commands, and per-filetype formatter resolution
//! (external > LSP > no-op).

pub mod commands;
mod hook;
pub mod ids;
pub mod lsp_formatter;
pub mod resolver;

use std::sync::Arc;

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_kernel::api::v1::{
        EventResult, Module, ModuleContext, ModuleError, ModuleId, OptionSpec, OptionValue,
        ProbeResult, Subscription, Version,
        events::kernel::{BufferWillSave, priority},
    },
};

/// Format module.
///
/// Subscribes to `BufferWillSave` for format-on-save and registers
/// `:Format` and `:FormatRange` commands.
pub struct FormatModule {
    subscriptions: Vec<Subscription>,
}

impl FormatModule {
    /// Create a new format module instance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
        }
    }
}

impl Default for FormatModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for FormatModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "Format"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::FORMATTER_PROVIDER]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register autoformat option
        if let Err(e) = ctx.kernel.options.register(OptionSpec::new(
            "autoformat",
            "Enable format-on-save",
            OptionValue::bool(true),
        )) {
            return ProbeResult::Failed(ModuleError::InitFailed(format!(
                "Failed to register autoformat option: {e}"
            )));
        }

        // Register command handlers
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::command_handlers() {
            command_store.add(handler);
        }

        // Subscribe to BufferWillSave for format-on-save
        let options = Arc::clone(&ctx.kernel.options);
        let services = Arc::clone(&ctx.services);

        let sub =
            ctx.kernel
                .event_bus
                .subscribe::<BufferWillSave, _>(priority::NORMAL, move |event| {
                    hook::format_on_save(event, &services, &options);
                    EventResult::Handled
                });
        // Detach so handler survives module drop (bootstrap drops modules after init).
        sub.detach();
        self.subscriptions.push(sub);

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        let count = self.subscriptions.len();
        self.subscriptions.clear();
        tracing::debug!("Format module exiting, cleared {count} subscriptions");
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(FormatModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
