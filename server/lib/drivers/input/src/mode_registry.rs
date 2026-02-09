//! Mode provider registry.
//!
//! Type alias for the mode provider registry, keyed by purpose.

use reovim_kernel::api::v1::MultiServiceRegistry;

use crate::{DefaultModeProvider, mode_key::ModeProviderKey};

/// Registry for mode providers, keyed by purpose.
///
/// This is a type alias for `MultiServiceRegistry<ModeProviderKey, dyn DefaultModeProvider>`.
/// Currently only `Entry` key is supported for the startup mode provider.
///
/// # Architecture
///
/// Following the VFS pattern (mechanism/policy separation):
/// - **Mechanism (driver)**: This registry type + `DefaultModeProvider` trait
/// - **Policy (module)**: `VimDefaultModeProvider` in `server/modules/vim`
///
/// # Example
///
/// ```ignore
/// use reovim_driver_input::{ModeProviderKey, ModeProviderRegistry, DefaultModeProvider};
/// use std::sync::Arc;
///
/// // Create registry (typically done by runner)
/// let registry = ModeProviderRegistry::new();
///
/// // Modules register their providers during init
/// registry.register(ModeProviderKey::Entry, Arc::new(vim_mode_provider));
///
/// // Runner queries with typed key
/// let provider = registry.get(&ModeProviderKey::Entry);
/// ```
pub type ModeProviderRegistry = MultiServiceRegistry<ModeProviderKey, dyn DefaultModeProvider>;

#[cfg(test)]
mod tests {
    use std::sync::{Arc, LazyLock};

    use reovim_kernel::api::v1::ModeId;

    use {super::*, crate::ProviderPriority};

    // Mock provider for testing
    struct MockModeProvider;

    impl DefaultModeProvider for MockModeProvider {
        fn provider_id(&self) -> &reovim_kernel::api::v1::ModuleId {
            static ID: reovim_kernel::api::v1::ModuleId =
                reovim_kernel::api::v1::ModuleId::new("mock");
            &ID
        }

        fn priority(&self) -> ProviderPriority {
            ProviderPriority::Default
        }

        fn entry_mode(&self) -> &ModeId {
            static MODE: LazyLock<ModeId> = LazyLock::new(|| {
                ModeId::new(reovim_kernel::api::v1::ModuleId::new("mock"), "normal")
            });
            &MODE
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let registry = ModeProviderRegistry::new();

        let provider = Arc::new(MockModeProvider);
        registry.register(ModeProviderKey::Entry, provider);

        let retrieved = registry.get(&ModeProviderKey::Entry);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_registry_keys() {
        let registry = ModeProviderRegistry::new();

        registry.register(ModeProviderKey::Entry, Arc::new(MockModeProvider));

        let keys = registry.keys();
        assert_eq!(keys.len(), 1);
        assert!(keys.contains(&ModeProviderKey::Entry));
    }

    #[test]
    fn test_registry_get_nonexistent() {
        let registry = ModeProviderRegistry::new();
        assert!(registry.get(&ModeProviderKey::Entry).is_none());
    }

    #[test]
    fn test_registry_provider_methods() {
        let registry = ModeProviderRegistry::new();
        registry.register(ModeProviderKey::Entry, Arc::new(MockModeProvider));

        let provider = registry.get(&ModeProviderKey::Entry).unwrap();
        assert_eq!(provider.provider_id().as_str(), "mock");
        assert_eq!(provider.priority(), ProviderPriority::Default);
        assert_eq!(provider.entry_mode().name(), "normal");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_registry_replace_provider() {
        struct AltProvider;

        impl DefaultModeProvider for AltProvider {
            fn provider_id(&self) -> &reovim_kernel::api::v1::ModuleId {
                static ID: reovim_kernel::api::v1::ModuleId =
                    reovim_kernel::api::v1::ModuleId::new("alt");
                &ID
            }

            fn entry_mode(&self) -> &ModeId {
                static MODE: LazyLock<ModeId> = LazyLock::new(|| {
                    ModeId::new(reovim_kernel::api::v1::ModuleId::new("alt"), "insert")
                });
                &MODE
            }
        }

        let registry = ModeProviderRegistry::new();
        registry.register(ModeProviderKey::Entry, Arc::new(MockModeProvider));
        registry.register(ModeProviderKey::Entry, Arc::new(AltProvider));

        let provider = registry.get(&ModeProviderKey::Entry).unwrap();
        assert_eq!(provider.provider_id().as_str(), "alt");
        assert_eq!(provider.entry_mode().name(), "insert");
    }

    #[test]
    fn test_registry_new_is_empty() {
        let registry = ModeProviderRegistry::new();
        assert!(registry.keys().is_empty());
    }
}
