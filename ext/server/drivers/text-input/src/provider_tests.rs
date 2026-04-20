use {
    crate::{DefaultModeProvider, ProviderPriority},
    reovim_kernel::api::v1::{ModeId, ModuleId},
};

#[test]
fn test_provider_priority_ordering() {
    assert!(ProviderPriority::Override > ProviderPriority::Default);
}

#[test]
fn test_provider_priority_default() {
    assert_eq!(ProviderPriority::default(), ProviderPriority::Default);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_provider_priority_debug() {
    let debug = format!("{:?}", ProviderPriority::Default);
    assert_eq!(debug, "Default");
    let debug = format!("{:?}", ProviderPriority::Override);
    assert_eq!(debug, "Override");
}

#[test]
fn test_provider_priority_clone() {
    let p = ProviderPriority::Override;
    let cloned = p;
    assert_eq!(p, cloned);
}

#[test]
fn test_provider_priority_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(ProviderPriority::Default);
    set.insert(ProviderPriority::Override);
    assert_eq!(set.len(), 2);
    set.insert(ProviderPriority::Default);
    assert_eq!(set.len(), 2);
}

struct TestProvider {
    id: ModuleId,
    mode: ModeId,
}

impl TestProvider {
    fn new() -> Self {
        let id = ModuleId::new("test");
        let mode = ModeId::new(id.clone(), "normal");
        Self { id, mode }
    }
}

impl DefaultModeProvider for TestProvider {
    fn provider_id(&self) -> &ModuleId {
        &self.id
    }

    fn entry_mode(&self) -> &ModeId {
        &self.mode
    }
}

#[test]
fn test_default_mode_provider_default_priority() {
    let provider = TestProvider::new();
    assert_eq!(provider.priority(), ProviderPriority::Default);
}

#[test]
fn test_default_mode_provider_entry_mode() {
    let provider = TestProvider::new();
    let mode = provider.entry_mode();
    assert_eq!(mode.name(), "normal");
}

#[test]
fn test_default_mode_provider_id() {
    let provider = TestProvider::new();
    assert_eq!(provider.provider_id().as_str(), "test");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_default_mode_provider_is_object_safe() {
    fn _accepts_ref(_: &dyn DefaultModeProvider) {}
    fn _accepts_box(_: Box<dyn DefaultModeProvider>) {}
    fn _accepts_arc(_: std::sync::Arc<dyn DefaultModeProvider>) {}
}

struct OverrideProvider;

impl DefaultModeProvider for OverrideProvider {
    fn provider_id(&self) -> &ModuleId {
        static ID: ModuleId = ModuleId::new("override");
        &ID
    }

    fn priority(&self) -> ProviderPriority {
        ProviderPriority::Override
    }

    fn entry_mode(&self) -> &ModeId {
        static MODE: std::sync::LazyLock<ModeId> =
            std::sync::LazyLock::new(|| ModeId::new(ModuleId::new("override"), "custom"));
        &MODE
    }
}

#[test]
fn test_override_provider_has_higher_priority() {
    let default_provider = TestProvider::new();
    let override_provider = OverrideProvider;
    assert!(override_provider.priority() > default_provider.priority());
}

#[test]
fn test_provider_priority_partial_ord() {
    assert!(ProviderPriority::Override > ProviderPriority::Default);
    assert!(ProviderPriority::Default < ProviderPriority::Override);
    assert!(ProviderPriority::Default == ProviderPriority::Default);
}

#[test]
fn test_provider_priority_ord() {
    let mut priorities = vec![ProviderPriority::Override, ProviderPriority::Default];
    priorities.sort();
    assert_eq!(priorities, vec![ProviderPriority::Default, ProviderPriority::Override]);
}

#[test]
fn test_provider_priority_repr_values() {
    assert_eq!(ProviderPriority::Default as u8, 50);
    assert_eq!(ProviderPriority::Override as u8, 100);
}

#[test]
fn test_override_provider_entry_mode() {
    let provider = OverrideProvider;
    assert_eq!(provider.entry_mode().name(), "custom");
}

#[test]
fn test_override_provider_priority() {
    let provider = OverrideProvider;
    assert_eq!(provider.priority(), ProviderPriority::Override);
}

#[test]
fn test_override_provider_id() {
    let provider = OverrideProvider;
    assert_eq!(provider.provider_id().as_str(), "override");
}

#[test]
fn test_default_mode_provider_as_dyn() {
    let provider: &dyn DefaultModeProvider = &TestProvider::new();
    assert_eq!(provider.provider_id().as_str(), "test");
    assert_eq!(provider.priority(), ProviderPriority::Default);
}
