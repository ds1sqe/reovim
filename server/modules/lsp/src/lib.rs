#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! LSP module for reovim.
//!
//! Provides Language Server Protocol integration following the
//! mechanism/policy separation:
//! - **Mechanism**: `LspProvider` trait, `Client`, transport (in `reovim-driver-lsp`)
//! - **Policy**: `LspModule` (this module) manages server lifecycle and events

mod saturator;

pub use saturator::{LspSaturator, LspSaturatorHandle};

use std::sync::Arc;

use {
    reovim_driver_lsp::{
        LspLifecycleRegistry, LspProviderRegistry, LspRequest, config_for_language,
        find_project_root, language_id_from_path, uri_from_path,
    },
    reovim_driver_session::{TickSchedulerHandle, bridges::BridgeProvider},
    reovim_kernel::api::v1::{
        BufferId, EventResult, Module, ModuleContext, ModuleError, ModuleId, ProbeResult,
        Subscription, Version,
        events::kernel::{BufferSaved, FileOpened},
        pr_info,
    },
    tracing::debug,
};

mod auto_starter;
pub mod diagnostic_bridge;
pub mod diagnostic_state;

/// LSP module instance.
///
/// Registers the `LspProviderRegistry` in `ServiceRegistry` during init.
/// Language servers are started on-demand when files with supported
/// languages are opened.
///
/// Subscribes to `BufferSaved` events and forwards `DidSave` notifications
/// to active LSP servers.
pub struct LspModule {
    /// Subscription handle for `BufferSaved` events (RAII).
    buffer_saved_sub: Option<Subscription>,
    /// Subscription handle for `FileOpened` events (RAII, #564).
    file_opened_sub: Option<Subscription>,
}

impl LspModule {
    /// Create a new LSP module.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buffer_saved_sub: None,
            file_opened_sub: None,
        }
    }
}

impl Default for LspModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for LspModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("lsp")
    }

    fn name(&self) -> &'static str {
        "LSP"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Create the LSP provider registry (empty initially).
        // Providers are registered on-demand when language servers are spawned.
        let _ = ctx.services.get_or_create::<LspProviderRegistry>();

        // Create the diagnostic path index for URI-to-BufferId resolution.
        let path_index = ctx
            .services
            .get_or_create::<diagnostic_bridge::DiagnosticPathIndex>();

        // Register the diagnostic bridge (stateless unit struct, #555).
        let bridge_provider = ctx.services.get_or_create::<BridgeProvider>();
        bridge_provider.register(diagnostic_bridge::DiagnosticBridge);

        // Ensure TickSchedulerHandle exists for diagnostic tick (#564).
        let _ = ctx.services.get_or_create::<TickSchedulerHandle>();

        // Register LspLifecycle implementation (#542: decouple completion from module-lsp).
        let lifecycle: Arc<dyn reovim_driver_lsp::LspLifecycle> =
            Arc::new(auto_starter::LspAutoStarter);
        let lifecycle_registry = ctx.services.get_or_create::<LspLifecycleRegistry>();
        lifecycle_registry.register(Arc::clone(&lifecycle));

        // Subscribe to FileOpened events → auto-start LSP server (#564).
        {
            let services = Arc::clone(&ctx.services);
            let buffers = Arc::clone(&ctx.kernel.buffers);
            let file_opened_sub =
                ctx.kernel
                    .event_bus
                    .subscribe::<FileOpened, _>(0, move |event| {
                        let Some(lang) = language_id_from_path(&event.path) else {
                            return EventResult::NotHandled;
                        };

                        #[allow(clippy::cast_possible_truncation)]
                        let buf_id = BufferId::from_raw(event.buffer_id as usize);

                        // Check if an LSP provider is already active for this language.
                        let registry = services.get_or_create::<LspProviderRegistry>();
                        if let Some(provider) =
                            registry.get(&reovim_driver_lsp::LspKey::Language(lang.clone()))
                            && provider.is_active()
                        {
                            // Provider already running — send DidOpen for the new file.
                            let uri = uri_from_path(std::path::Path::new(&event.path));
                            let content = buffers
                                .get(buf_id)
                                .map(|b| b.read().content())
                                .unwrap_or_default();
                            provider.send_request(LspRequest::DidOpen {
                                uri,
                                language_id: lang,
                                version: 1,
                                content,
                            });
                            return EventResult::Handled;
                        }

                        // No active provider — try to auto-start.
                        let Some(root) = find_project_root(std::path::Path::new(&event.path))
                        else {
                            return EventResult::NotHandled;
                        };
                        let Some(config) = config_for_language(&lang, &root) else {
                            return EventResult::NotHandled;
                        };

                        let content = buffers
                            .get(buf_id)
                            .map(|b| b.read().content())
                            .unwrap_or_default();

                        lifecycle.auto_start(
                            &services,
                            config,
                            lang,
                            event.path.clone(),
                            content,
                            event.buffer_id,
                        );
                        EventResult::Handled
                    });
            self.file_opened_sub = Some(file_opened_sub);
        }

        // Subscribe to BufferSaved events → send DidSave to active LSP servers.
        let services = Arc::clone(&ctx.services);
        let path_index_clone = Arc::clone(&path_index);
        let sub = ctx
            .kernel
            .event_bus
            .subscribe::<BufferSaved, _>(0, move |event| {
                let registry = services.get_or_create::<LspProviderRegistry>();

                let path = std::path::Path::new(&event.path);
                let uri = uri_from_path(path);

                // Populate diagnostic path index with buffer_id ↔ URI mapping.
                path_index_clone.insert(uri.as_str().to_string(), event.buffer_id);

                // Send DidSave to all active providers (typically one).
                for provider in registry.values() {
                    if provider.is_active() {
                        debug!(path = %event.path, "Sending DidSave notification");
                        provider.send_request(LspRequest::DidSave {
                            uri: uri.clone(),
                            text: None,
                        });
                    }
                }

                EventResult::Handled
            });
        self.buffer_saved_sub = Some(sub);

        pr_info!("LSP module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("LSP module exiting");
        Ok(())
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[reovim_extension_kinds::DIAGNOSTICS]
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(LspModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
