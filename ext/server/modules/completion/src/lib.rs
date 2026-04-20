#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Completion module - POLICY layer.
//!
//! Orchestrates the completion popup: registers completion sources, provides
//! session state management, and bridges state to clients.
//!
//! # Architecture (#521)
//!
//! This module owns the per-client `CompletionState` stored in `ExtensionMap`.
//! The bridge serializes state to JSON consumed by both TUI and Web extensions.
//! Completion sources are registered in the `CompletionSourceRegistry` service.

pub mod bridge;
pub mod buffer_words;
pub mod commands;
pub mod ids;
mod keybinding;
pub mod lsp_source;
pub mod notification_queue;
pub mod state;

pub use {bridge::CompletionBridge, state::CompletionState};

const KIND: &str = "completion";

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_completion::CompletionSourceRegistry,
    reovim_driver_text_input::KeybindingStore,
    reovim_driver_text_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, OptionConstraint,
        OptionSpec, OptionValue, ProbeResult, Version,
    },
    reovim_subsys_module_config::ModuleConfigStore,
    std::sync::Arc,
};

/// Completion module.
///
/// Registers [`CompletionBridge`], creates the [`CompletionSourceRegistry`]
/// service, and registers the [`BufferWordsSource`](buffer_words::BufferWordsSource)
/// during `init()`.
pub struct CompletionModule;

impl CompletionModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CompletionModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CompletionModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "Completion"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register CompletionBridge via BridgeProvider.
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(CompletionBridge);

        // Register built-in completion sources.
        let registry = ctx.services.get_or_create::<CompletionSourceRegistry>();
        registry.register(Arc::new(buffer_words::BufferWordsSource));

        // Register LspCompletionSource in both registries:
        // - CompletionSourceRegistry: so the engine calls complete() on it
        // - ServiceRegistry: so fire_lsp_completion() can update_cache() on it
        let lsp_source = Arc::new(lsp_source::LspCompletionSource::new());
        registry.register(
            Arc::clone(&lsp_source) as Arc<dyn reovim_driver_completion::CompletionSource>
        );
        ctx.services.register(lsp_source);

        // Register pending notification queue for background thread → UI bridge.
        let _ = ctx
            .services
            .get_or_create::<notification_queue::PendingNotificationQueue>();

        // Register command handlers.
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::command_handlers() {
            command_store.add(handler);
        }

        // Epic #570: Register completion options (#574)
        for spec in completion_option_specs() {
            if let Err(e) = ctx.kernel.options.register(spec) {
                return ProbeResult::Failed(ModuleError::InitFailed(format!(
                    "Failed to register completion option: {e}"
                )));
            }
        }

        // #610: Apply user config overrides from modules.toml
        apply_completion_config(ctx);

        // #700: Register keybindings from personality adapters
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::COMPLETION_PROVIDER]
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        keybinding::all()
    }
}

/// Completion option specifications.
///
/// Popup menu height and width for the completion UI.
/// Registered during `CompletionModule::init()`.
#[cfg_attr(coverage_nightly, coverage(off))]
fn completion_option_specs() -> Vec<OptionSpec> {
    vec![
        OptionSpec::new(
            "pumheight",
            "Maximum number of items in completion popup",
            OptionValue::int(10),
        )
        .with_short("ph")
        .with_constraint(OptionConstraint::range(1, 50))
        .with_owner(ids::MODULE),
        OptionSpec::new("pumwidth", "Minimum width of completion popup", OptionValue::int(15))
            .with_short("pw")
            .with_constraint(OptionConstraint::range(5, 80))
            .with_owner(ids::MODULE),
    ]
}

/// Apply user config overrides from `modules.toml` to completion options.
///
/// Reads `pumheight` and `pumwidth` from the module's config section and
/// overrides the option defaults via `set_global()`. Type mismatches are
/// logged as warnings but do not fail init.
#[cfg_attr(coverage_nightly, coverage(off))]
fn apply_completion_config(ctx: &ModuleContext) {
    let Some(store) = ctx.services.get::<ModuleConfigStore>() else {
        return;
    };

    if let Ok(Some(height)) = store.get_int("completion", "pumheight") {
        if let Err(e) = ctx
            .kernel
            .options
            .set_global("pumheight", OptionValue::int(height))
        {
            tracing::warn!("completion config: failed to set pumheight: {e}");
        }
    } else if let Err(e) = store.get_int("completion", "pumheight") {
        tracing::warn!("completion config: {e}");
    }

    if let Ok(Some(width)) = store.get_int("completion", "pumwidth") {
        if let Err(e) = ctx
            .kernel
            .options
            .set_global("pumwidth", OptionValue::int(width))
        {
            tracing::warn!("completion config: failed to set pumwidth: {e}");
        }
    } else if let Err(e) = store.get_int("completion", "pumwidth") {
        tracing::warn!("completion config: {e}");
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CompletionModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
