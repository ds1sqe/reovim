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

