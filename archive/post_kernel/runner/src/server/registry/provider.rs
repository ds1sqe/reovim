//! Provider registry for default mode initialization.
//!
//! This registry collects mode providers from modules during boot and resolves
//! them by priority. It implements the "single initialization path" principle.
//!
//! # Design
//!
//! - **Priority-based resolution**: Higher priority providers win
//! - **Panic fast**: Missing essential providers cause immediate panic
//! - **Single source of truth**: All defaults come from provider registries
//!
//! # Note
//!
//! VFS providers now use `ServiceRegistry` with typed keys (Epic #417).
//! See `reovim_driver_vfs::VfsProviderRegistry`.

use std::sync::Arc;

use {reovim_driver_input::DefaultModeProvider, reovim_kernel::api::v1::ModeId};

/// Registry for default mode providers.
///
/// Collects mode providers during initialization and resolves them
/// by priority to determine the entry mode.
///
/// # Panic Fast
///
/// Call `validate()` after all providers are registered. It will panic
/// if no mode provider is registered.
pub struct DefaultModeProviderRegistry {
    providers: Vec<Arc<dyn DefaultModeProvider>>,
}

impl DefaultModeProviderRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a default mode provider.
    ///
    /// Providers are automatically sorted by priority (highest first).
    pub fn register(&mut self, provider: Arc<dyn DefaultModeProvider>) {
        self.providers.push(provider);
        self.providers
            .sort_by_key(|p| std::cmp::Reverse(p.priority()));
    }

    /// Get the entry mode from the highest priority provider.
    ///
    /// # Returns
    ///
    /// The mode ID to use as the initial mode, or `None` if no
    /// provider is registered.
    #[must_use]
    pub fn entry_mode(&self) -> Option<&ModeId> {
        self.providers.first().map(|p| p.entry_mode())
    }

    /// Validate that essential providers are registered.
    ///
    /// # Panics
    ///
    /// Panics if no mode provider is registered.
    /// This is the "panic fast" behavior for essential providers.
    pub fn validate(&self) {
        assert!(
            self.entry_mode().is_some(),
            "FATAL: No default mode provider. Load vim module or another mode module."
        );
    }

    /// Number of registered providers (for testing).
    #[cfg(test)]
    #[must_use]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.providers.len()
    }
}

impl Default for DefaultModeProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use {
        reovim_driver_input::ProviderPriority as ModeProviderPriority,
        reovim_kernel::api::v1::ModuleId,
    };

    use super::*;

    // Mock mode provider for testing
    struct MockModeProvider {
        id: ModuleId,
        priority: ModeProviderPriority,
        mode: ModeId,
    }

    impl DefaultModeProvider for MockModeProvider {
        fn provider_id(&self) -> &ModuleId {
            &self.id
        }

        fn priority(&self) -> ModeProviderPriority {
            self.priority
        }

        fn entry_mode(&self) -> &ModeId {
            &self.mode
        }
    }

    #[test]
    fn test_mode_registry_empty() {
        let registry = DefaultModeProviderRegistry::new();
        assert!(registry.entry_mode().is_none());
    }

    #[test]
    fn test_mode_registry_single_provider() {
        let mut registry = DefaultModeProviderRegistry::new();

        let provider = Arc::new(MockModeProvider {
            id: ModuleId::new("vim"),
            priority: ModeProviderPriority::Default,
            mode: ModeId::new(ModuleId::new("vim"), "normal"),
        });

        registry.register(provider);

        let mode = registry.entry_mode().unwrap();
        assert_eq!(mode.module().as_str(), "vim");
        assert_eq!(mode.name(), "normal");
    }

    #[test]
    fn test_mode_registry_priority_ordering() {
        let mut registry = DefaultModeProviderRegistry::new();

        // Register low priority first
        let low = Arc::new(MockModeProvider {
            id: ModuleId::new("editor"),
            priority: ModeProviderPriority::Default,
            mode: ModeId::new(ModuleId::new("editor"), "normal"),
        });
        registry.register(low);

        // Register high priority second
        let high = Arc::new(MockModeProvider {
            id: ModuleId::new("vim"),
            priority: ModeProviderPriority::Override,
            mode: ModeId::new(ModuleId::new("vim"), "normal"),
        });
        registry.register(high);

        // High priority should win
        let mode = registry.entry_mode().unwrap();
        assert_eq!(mode.module().as_str(), "vim");
    }

    #[test]
    #[should_panic(expected = "FATAL: No default mode provider")]
    fn test_mode_registry_validate_panics_when_empty() {
        let registry = DefaultModeProviderRegistry::new();
        registry.validate();
    }
}
