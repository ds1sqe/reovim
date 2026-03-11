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

        // Verify that DiagnosticPathIndex was created
        let path_index = services.get::<diagnostic_bridge::DiagnosticPathIndex>();
        assert!(path_index.is_some(), "DiagnosticPathIndex should be registered");

        // Verify that DiagnosticBridge was registered via BridgeProvider
        let bridge_provider = services.get::<BridgeProvider>();
        assert!(bridge_provider.is_some(), "BridgeProvider should be registered");
        let bridges = bridge_provider.unwrap().take_bridges();
        assert!(
            bridges.iter().any(|b| b.kind() == "diagnostics"),
            "DiagnosticBridge should be registered"
        );
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
        assert!(module.file_opened_sub.is_none());

        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);

        // Verify subscriptions were stored
        assert!(module.buffer_saved_sub.is_some());
        assert!(module.file_opened_sub.is_some());
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

    #[test]
    fn test_buffer_saved_inactive_provider_skipped() {
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

        struct InactiveMockProvider {
            send_called: Arc<AtomicBool>,
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        #[allow(clippy::unnecessary_literal_bound)]
        impl LspProvider for InactiveMockProvider {
            fn is_active(&self) -> bool {
                false
            }

            fn send_request(&self, _request: LspRequest) -> bool {
                self.send_called.store(true, Ordering::SeqCst);
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

        let send_called = Arc::new(AtomicBool::new(false));
        let registry = services.get_or_create::<LspProviderRegistry>();
        registry.register(
            LspKey::Language("rust".to_owned()),
            Arc::new(InactiveMockProvider {
                send_called: Arc::clone(&send_called),
            }),
        );

        kernel.event_bus.emit(BufferSaved {
            buffer_id: 1,
            path: "/tmp/test.rs".to_string(),
        });

        assert!(
            !send_called.load(Ordering::SeqCst),
            "DidSave should NOT be sent to inactive provider"
        );
    }

    // ========================================================================
    // FileOpened event handler tests (#564)
    // ========================================================================

    /// Helper mock `LspProvider` for `FileOpened` tests.
    struct FileOpenedMockProvider {
        did_open_flag: Arc<std::sync::atomic::AtomicBool>,
        active: bool,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[allow(clippy::unnecessary_literal_bound)]
    impl reovim_driver_lsp::LspProvider for FileOpenedMockProvider {
        fn is_active(&self) -> bool {
            self.active
        }

        fn send_request(&self, request: reovim_driver_lsp::LspRequest) -> bool {
            if matches!(request, reovim_driver_lsp::LspRequest::DidOpen { .. }) {
                self.did_open_flag
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            }
            true
        }

        fn capabilities(&self) -> Option<Arc<lsp_types::ServerCapabilities>> {
            None
        }

        fn diagnostics(&self) -> &reovim_driver_lsp::DiagnosticCache {
            static CACHE: std::sync::OnceLock<reovim_driver_lsp::DiagnosticCache> =
                std::sync::OnceLock::new();
            CACHE.get_or_init(reovim_driver_lsp::DiagnosticCache::new)
        }

        fn root_path(&self) -> &std::path::Path {
            std::path::Path::new("/mock")
        }

        fn language_id(&self) -> &'static str {
            "rust"
        }

        fn server_info(&self) -> Option<&lsp_types::ServerInfo> {
            None
        }
    }

    #[test]
    fn test_file_opened_unknown_extension_not_handled() {
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

        // Emit FileOpened with unknown extension - should not panic
        kernel.event_bus.emit(FileOpened {
            buffer_id: 1,
            path: "/tmp/file.xyz".to_string(),
        });
    }

    #[test]
    fn test_file_opened_active_provider_sends_did_open() {
        use {
            reovim_driver_lsp::LspKey,
            reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
            std::{
                path::PathBuf,
                sync::{
                    Arc,
                    atomic::{AtomicBool, Ordering},
                },
            },
        };

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

        // Register an active mock provider for "rust"
        let did_open_called = Arc::new(AtomicBool::new(false));
        let registry = services.get_or_create::<LspProviderRegistry>();
        registry.register(
            LspKey::Language("rust".to_owned()),
            Arc::new(FileOpenedMockProvider {
                did_open_flag: Arc::clone(&did_open_called),
                active: true,
            }),
        );

        kernel.event_bus.emit(FileOpened {
            buffer_id: 1,
            path: "/tmp/test.rs".to_string(),
        });

        assert!(
            did_open_called.load(Ordering::SeqCst),
            "DidOpen should be sent to active provider"
        );
    }

    #[test]
    fn test_file_opened_inactive_provider_tries_auto_start() {
        use {
            reovim_driver_lsp::LspKey,
            reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
            std::{
                path::PathBuf,
                sync::{
                    Arc,
                    atomic::{AtomicBool, Ordering},
                },
            },
        };

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

        // Register an inactive provider — should fall through to auto-start path
        let did_open_called = Arc::new(AtomicBool::new(false));
        let registry = services.get_or_create::<LspProviderRegistry>();
        registry.register(
            LspKey::Language("rust".to_owned()),
            Arc::new(FileOpenedMockProvider {
                did_open_flag: Arc::clone(&did_open_called),
                active: false,
            }),
        );

        // Path doesn't have a project root, so auto-start will fail gracefully
        kernel.event_bus.emit(FileOpened {
            buffer_id: 1,
            path: "/nonexistent/path/test.rs".to_string(),
        });

        assert!(
            !did_open_called.load(Ordering::SeqCst),
            "DidOpen should NOT be sent to inactive provider"
        );
    }

    #[test]
    fn test_file_opened_no_provider_no_project_root() {
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

        // File with supported extension but no project root — should not panic
        kernel.event_bus.emit(FileOpened {
            buffer_id: 1,
            path: "/nonexistent/deep/nested/test.rs".to_string(),
        });
    }

    #[test]
    fn test_file_opened_no_config_for_language() {
        use {
            reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
            std::{path::PathBuf, sync::Arc},
        };

        let kernel = KernelContext::default();
        let services = Arc::new(ServiceRegistry::new());

        // Create a temp dir with Cargo.toml and a .zig file so
        // find_project_root exercises the is_file() branch.
        let tmp = PathBuf::from("/tmp/reovim-test-564-config");
        let _ = std::fs::create_dir_all(&tmp);
        let _ = std::fs::write(tmp.join("Cargo.toml"), "[package]\nname = \"test\"");
        let _ = std::fs::write(tmp.join("test.zig"), "pub fn main() {}");

        let ctx = ModuleContext::new(
            kernel.clone(),
            services,
            PathBuf::from("/tmp/test-data"),
            PathBuf::from("/tmp/test-cache"),
        );

        let mut module = LspModule::new();
        module.init(&ctx);

        // .zig has a language ID but no config_for_language mapping
        kernel.event_bus.emit(FileOpened {
            buffer_id: 1,
            path: format!("{}/test.zig", tmp.display()),
        });

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_file_opened_auto_start_with_lifecycle() {
        use {
            reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
            std::{path::PathBuf, sync::Arc},
        };

        let kernel = KernelContext::default();
        let services = Arc::new(ServiceRegistry::new());

        // Create a temp dir with Cargo.toml and a .rs file for project root detection.
        // The .rs file must exist so find_project_root exercises the is_file() branch.
        let tmp = PathBuf::from("/tmp/reovim-test-564-lifecycle");
        let _ = std::fs::create_dir_all(&tmp);
        let _ = std::fs::write(tmp.join("Cargo.toml"), "[package]\nname = \"test\"");
        let _ = std::fs::write(tmp.join("main.rs"), "fn main() {}");

        let ctx = ModuleContext::new(
            kernel.clone(),
            services,
            PathBuf::from("/tmp/test-data"),
            PathBuf::from("/tmp/test-cache"),
        );

        let mut module = LspModule::new();
        module.init(&ctx);

        // .rs file in a project with Cargo.toml → exercises the auto-start path.
        // LspAutoStarter.auto_start() calls tokio::runtime::Handle::try_current()
        // which returns Err in non-async test context, so the spawn is skipped —
        // but the closure code path up to lifecycle.auto_start() is fully exercised.
        kernel.event_bus.emit(FileOpened {
            buffer_id: 1,
            path: format!("{}/main.rs", tmp.display()),
        });

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_file_opened_no_extension() {
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

        // File with no extension (e.g., Makefile) → no language ID → NotHandled
        kernel.event_bus.emit(FileOpened {
            buffer_id: 1,
            path: "/tmp/Makefile".to_string(),
        });
    }
}
