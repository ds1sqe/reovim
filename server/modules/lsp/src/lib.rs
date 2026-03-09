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
    reovim_driver_lsp::{LspLifecycleRegistry, LspProviderRegistry, LspRequest, uri_from_path},
    reovim_kernel::api::v1::{
        EventResult, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Subscription,
        Version, events::kernel::BufferSaved, pr_info,
    },
    tracing::debug,
};

mod auto_starter;

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
}

impl LspModule {
    /// Create a new LSP module.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buffer_saved_sub: None,
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
        let _registry = ctx.services.get_or_create::<LspProviderRegistry>();

        // Register LspLifecycle implementation (#542: decouple completion from module-lsp).
        let lifecycle_registry = ctx.services.get_or_create::<LspLifecycleRegistry>();
        lifecycle_registry.register(Arc::new(auto_starter::LspAutoStarter));

        // Subscribe to BufferSaved events → send DidSave to active LSP servers.
        let services = Arc::clone(&ctx.services);
        let sub = ctx
            .kernel
            .event_bus
            .subscribe::<BufferSaved, _>(0, move |event| {
                let Some(registry) = services.get::<LspProviderRegistry>() else {
                    return EventResult::NotHandled;
                };

                let path = std::path::Path::new(&event.path);
                let uri = uri_from_path(path);

                // Send DidSave to all active providers (typically one).
                for key in registry.keys() {
                    if let Some(provider) = registry.get(&key)
                        && provider.is_active()
                    {
                        debug!(path = %event.path, key = ?key, "Sending DidSave notification");
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
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(LspModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = LspModule::new();
        assert_eq!(module.id().as_str(), "lsp");
    }

    #[test]
    fn test_module_name() {
        let module = LspModule::new();
        assert_eq!(module.name(), "LSP");
    }

    #[test]
    fn test_module_version() {
        let module = LspModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_module_default() {
        fn create_default<T: Default>() -> T {
            T::default()
        }
        let from_default: LspModule = create_default();
        let from_new = LspModule::new();
        assert_eq!(from_new.id(), from_default.id());
        assert_eq!(from_new.version(), from_default.version());
    }

    #[test]
    fn test_exit_succeeds() {
        let mut module = LspModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn test_dependencies_default_empty() {
        let module = LspModule::new();
        assert!(module.dependencies().is_empty());
    }

    #[test]
    fn test_init_registers_provider_registry() {
        use {
            reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
            std::{path::PathBuf, sync::Arc},
        };

        let kernel = KernelContext::default();
        let services = Arc::new(ServiceRegistry::new());
        let ctx = ModuleContext::new(
            kernel,
            services.clone(),
            PathBuf::from("/tmp/test-data"),
            PathBuf::from("/tmp/test-cache"),
        );

        let mut module = LspModule::new();
        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);

        // Verify that LspProviderRegistry was created in services
        let registry = services.get::<LspProviderRegistry>();
        assert!(registry.is_some(), "LspProviderRegistry should be registered in services");
    }

    #[test]
    fn test_init_subscribes_to_buffer_saved() {
        use {
            reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
            std::{path::PathBuf, sync::Arc},
        };

        let kernel = KernelContext::default();
        let services = Arc::new(ServiceRegistry::new());
        let ctx = ModuleContext::new(
            kernel,
            services,
            PathBuf::from("/tmp/test-data"),
            PathBuf::from("/tmp/test-cache"),
        );

        let mut module = LspModule::new();
        assert!(module.buffer_saved_sub.is_none());

        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);

        // Verify subscription was stored
        assert!(module.buffer_saved_sub.is_some());
    }

    #[test]
    fn test_buffer_saved_event_sends_did_save() {
        use {
            reovim_driver_lsp::{DiagnosticCache, LspKey, LspProvider, LspRequest},
            reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
            std::{
                path::{Path, PathBuf},
                sync::{
                    Arc, OnceLock,
                    atomic::{AtomicBool, Ordering},
                },
            },
        };

        struct MockLspProvider {
            did_save_flag: Arc<AtomicBool>,
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        #[allow(clippy::unnecessary_literal_bound)]
        impl LspProvider for MockLspProvider {
            fn is_active(&self) -> bool {
                true
            }

            fn send_request(&self, request: LspRequest) -> bool {
                if matches!(request, LspRequest::DidSave { .. }) {
                    self.did_save_flag.store(true, Ordering::SeqCst);
                }
                true
            }

            fn capabilities(&self) -> Option<Arc<lsp_types::ServerCapabilities>> {
                None
            }

            fn diagnostics(&self) -> &DiagnosticCache {
                static CACHE: OnceLock<DiagnosticCache> = OnceLock::new();
                CACHE.get_or_init(DiagnosticCache::new)
            }

            fn root_path(&self) -> &Path {
                Path::new("/mock")
            }

            fn language_id(&self) -> &'static str {
                "rust"
            }

            fn server_info(&self) -> Option<&lsp_types::ServerInfo> {
                None
            }
        }

        let kernel = KernelContext::default();
        let services = Arc::new(ServiceRegistry::new());
        let ctx = ModuleContext::new(
            kernel.clone(),
            services.clone(),
            PathBuf::from("/tmp/test-data"),
            PathBuf::from("/tmp/test-cache"),
        );

        let mut module = LspModule::new();
        module.init(&ctx);

        // Register a mock provider
        let did_save_called = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&did_save_called);

        let registry = services.get_or_create::<LspProviderRegistry>();
        registry.register(
            LspKey::Language("rust".to_owned()),
            Arc::new(MockLspProvider {
                did_save_flag: flag,
            }),
        );

        // Emit BufferSaved event
        kernel.event_bus.emit(BufferSaved {
            buffer_id: 1,
            path: "/tmp/test.rs".to_string(),
        });

        assert!(
            did_save_called.load(Ordering::SeqCst),
            "DidSave should have been sent to the active provider"
        );
    }

    #[test]
    fn test_buffer_saved_no_providers_no_panic() {
        use {
            reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
            std::{path::PathBuf, sync::Arc},
        };

        let kernel = KernelContext::default();
        let services = Arc::new(ServiceRegistry::new());
        let ctx = ModuleContext::new(
            kernel.clone(),
            services,
            PathBuf::from("/tmp/test-data"),
            PathBuf::from("/tmp/test-cache"),
        );

        let mut module = LspModule::new();
        module.init(&ctx);

        // Emit BufferSaved with no providers - should not panic
        kernel.event_bus.emit(BufferSaved {
            buffer_id: 1,
            path: "/tmp/test.rs".to_string(),
        });
    }
}
