use std::sync::Arc;

use reovim_kernel::api::v1::{ModeId, ModuleId};

use crate::{DefaultModeProvider, DefaultModeProviderModule, ProviderPriority};

struct TestProvider;

impl DefaultModeProvider for TestProvider {
    fn provider_id(&self) -> &ModuleId {
        static ID: ModuleId = ModuleId::new("test");
        &ID
    }

    fn priority(&self) -> ProviderPriority {
        ProviderPriority::Override
    }

    fn entry_mode(&self) -> &ModeId {
        static MODE: std::sync::LazyLock<ModeId> =
            std::sync::LazyLock::new(|| ModeId::new(ModuleId::new("test"), "insert"));
        &MODE
    }
}

struct TestModule(bool);

impl DefaultModeProviderModule for TestModule {
    fn default_mode_provider(&self) -> Option<Arc<dyn DefaultModeProvider>> {
        self.0
            .then(|| Arc::new(TestProvider) as Arc<dyn DefaultModeProvider>)
    }
}

#[test]
fn test_default_mode_provider_module_returns_provider() {
    let module = TestModule(true);
    let provider = module.default_mode_provider().unwrap();
    assert_eq!(provider.provider_id().as_str(), "test");
    assert_eq!(provider.entry_mode().name(), "insert");
    assert_eq!(provider.priority(), ProviderPriority::Override);
}

#[test]
fn test_default_mode_provider_module_returns_none() {
    let module = TestModule(false);
    assert!(module.default_mode_provider().is_none());
}
