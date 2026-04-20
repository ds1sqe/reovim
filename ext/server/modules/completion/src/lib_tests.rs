use super::*;

#[test]
fn module_id() {
    let module = CompletionModule::new();
    assert_eq!(module.id().as_str(), "completion");
}

#[test]
fn module_name() {
    let module = CompletionModule::new();
    assert_eq!(module.name(), "Completion");
}

#[test]
fn module_version() {
    let module = CompletionModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn module_default() {
    let module = CompletionModule::default();
    assert_eq!(module.id().as_str(), "completion");
}

#[test]
fn module_exit() {
    let mut module = CompletionModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn module_init_registers_bridge_and_registry() {
    use reovim_kernel::api::v1::ServiceRegistry;

    let services = Arc::new(ServiceRegistry::new());
    let ctx = test_module_context(services.clone());

    let mut module = CompletionModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    // Verify bridge was registered.
    let provider = services.get::<BridgeProvider>().unwrap();
    let bridges = provider.take_bridges();
    assert_eq!(bridges.len(), 1);
    assert_eq!(bridges[0].kind(), "completion");

    // Verify CompletionSourceRegistry was created with built-in sources.
    let registry = services.get::<CompletionSourceRegistry>();
    assert!(registry.is_some());
    let reg = registry.unwrap();
    assert_eq!(reg.len(), 2);
    assert!(reg.get("buffer").is_some());
    assert!(reg.get("lsp").is_some());

    // Verify LspCompletionSource is also in ServiceRegistry (for cache updates).
    let lsp_source = services.get::<lsp_source::LspCompletionSource>();
    assert!(lsp_source.is_some());

    // Verify PendingNotificationQueue was registered.
    let queue = services.get::<notification_queue::PendingNotificationQueue>();
    assert!(queue.is_some());

    // Verify commands were registered.
    let command_store = services.get::<CommandHandlerStore>();
    assert!(command_store.is_some());
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_context(
    services: std::sync::Arc<reovim_kernel::api::v1::ServiceRegistry>,
) -> ModuleContext {
    ModuleContext::new(
        reovim_kernel::api::v1::KernelContext::default(),
        services,
        std::path::PathBuf::from("/tmp"),
        std::path::PathBuf::from("/tmp"),
    )
}

#[test]
fn test_extension_kinds() {
    let module = CompletionModule::new();
    assert_eq!(module.extension_kinds(), &["completion"]);
}

// ============================================================================
// Config consumer (#610)
// ============================================================================

#[test]
fn apply_config_no_store_is_noop() {
    let services = Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
    let ctx = test_module_context(services);

    let mut module = CompletionModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));
    // pumheight should still have its default (10)
    let val = ctx
        .kernel
        .options
        .get("pumheight", reovim_kernel::api::v1::OptionScopeId::Global)
        .unwrap();
    assert_eq!(val.as_int(), Some(10));
}

#[test]
fn apply_config_with_pumheight_override() {
    use std::collections::HashMap;

    let services = Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());

    // Register a ModuleConfigStore with pumheight=25
    let settings: toml::Value = toml::from_str("pumheight = 25").unwrap();
    let mut configs = HashMap::new();
    configs.insert("completion".to_string(), settings);
    services.register(Arc::new(ModuleConfigStore::new(configs)));

    let ctx = test_module_context(services);
    let mut module = CompletionModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    let val = ctx
        .kernel
        .options
        .get("pumheight", reovim_kernel::api::v1::OptionScopeId::Global)
        .unwrap();
    assert_eq!(val.as_int(), Some(25));
}

#[test]
fn apply_config_with_pumwidth_override() {
    use std::collections::HashMap;

    let services = Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());

    let settings: toml::Value = toml::from_str("pumwidth = 30").unwrap();
    let mut configs = HashMap::new();
    configs.insert("completion".to_string(), settings);
    services.register(Arc::new(ModuleConfigStore::new(configs)));

    let ctx = test_module_context(services);
    let mut module = CompletionModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    let val = ctx
        .kernel
        .options
        .get("pumwidth", reovim_kernel::api::v1::OptionScopeId::Global)
        .unwrap();
    assert_eq!(val.as_int(), Some(30));
}

#[test]
fn apply_config_wrong_type_logs_warning_continues() {
    use std::collections::HashMap;

    let services = Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());

    // pumheight is a string instead of int — should log warning, keep default
    let settings: toml::Value = toml::from_str(r#"pumheight = "big""#).unwrap();
    let mut configs = HashMap::new();
    configs.insert("completion".to_string(), settings);
    services.register(Arc::new(ModuleConfigStore::new(configs)));

    let ctx = test_module_context(services);
    let mut module = CompletionModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    // Default should remain
    let val = ctx
        .kernel
        .options
        .get("pumheight", reovim_kernel::api::v1::OptionScopeId::Global)
        .unwrap();
    assert_eq!(val.as_int(), Some(10));
}

#[test]
fn apply_config_missing_fields_uses_defaults() {
    use std::collections::HashMap;

    let services = Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());

    // Settings present but without pumheight/pumwidth
    let settings: toml::Value = toml::from_str("some_other = true").unwrap();
    let mut configs = HashMap::new();
    configs.insert("completion".to_string(), settings);
    services.register(Arc::new(ModuleConfigStore::new(configs)));

    let ctx = test_module_context(services);
    let mut module = CompletionModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    let val = ctx
        .kernel
        .options
        .get("pumheight", reovim_kernel::api::v1::OptionScopeId::Global)
        .unwrap();
    assert_eq!(val.as_int(), Some(10));
    let val = ctx
        .kernel
        .options
        .get("pumwidth", reovim_kernel::api::v1::OptionScopeId::Global)
        .unwrap();
    assert_eq!(val.as_int(), Some(15));
}
