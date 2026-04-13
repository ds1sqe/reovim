//! Mode provider key - typed key for mode provider lookup.
//!
//! Defines the typed key enum for mode provider discovery.

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for mode provider lookup.
///
/// This enum defines the purposes for which mode providers can be registered.
/// Currently only `Entry` is supported (the startup mode), but this can be
/// extended for other mode-related services.
///
/// # Compile-Time Safety
///
/// Using typed keys instead of strings ensures:
/// - Typos are caught at compile time (`ModeProviderKey::Etnry` → error)
/// - Exhaustive matching in `match` statements
/// - Self-documenting API
///
/// # Example
///
/// ```ignore
/// use reovim_subsys_input::{ModeProviderKey, ModeProviderRegistry, DefaultModeProvider};
/// use std::sync::Arc;
///
/// let registry = ModeProviderRegistry::new();
/// registry.register(ModeProviderKey::Entry, Arc::new(VimModeProvider::new()));
///
/// // Lookup with typed key
/// let provider = registry.get(&ModeProviderKey::Entry);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModeProviderKey {
    /// Entry/startup mode provider.
    ///
    /// Provides the mode to use when a new session starts (e.g., Normal mode in Vim).
    /// This is queried by the runner during session initialization.
    Entry,
}

impl ServiceKey for ModeProviderKey {
    fn service_name() -> &'static str {
        "Mode"
    }
}
