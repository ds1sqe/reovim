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
