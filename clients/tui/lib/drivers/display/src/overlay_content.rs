//! Overlay content provider trait.
//!
//! Mechanism for overlay windows to display custom content instead
//! of buffer content. Modules register content via the `ServiceRegistry`.
//!
//! # Architecture
//!
//! This follows the mechanism/policy separation:
//! - **Mechanism** (this driver): Registry and key types
//! - **Policy** (modules): Actual provider implementations
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_display::overlay_content::{
//!     OverlayContentKey, OverlayContentRegistry,
//! };
//!
//! // Module registers provider during init
//! let registry = OverlayContentRegistry::new();
//! let key = OverlayContentKey::new("which-key");
//! registry.register(key, Arc::new(my_provider));
//!
//! // Runner iterates all providers generically
//! for key in registry.keys() {
//!     if let Some(provider) = registry.get(&key) {
//!         if let Some(lines) = provider.content_for(window_id) {
//!             render_overlay_lines(&mut screen, placement, &lines);
//!         }
//!     }
//! }
//! ```

use {
    crate::WindowId,
    reovim_kernel::api::v1::{MultiServiceRegistry, Service, ServiceKey},
};

/// Key for overlay content providers in `ServiceRegistry`.
///
/// Each module registers with a unique string key (e.g., "which-key", "diagnostics").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OverlayContentKey(String);

impl OverlayContentKey {
    /// Create a new overlay content key.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the key name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl ServiceKey for OverlayContentKey {
    fn service_name() -> &'static str {
        "OverlayContent"
    }
}

impl std::fmt::Display for OverlayContentKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OverlayContentKey({})", self.0)
    }
}

/// Trait for providing overlay window content.
///
/// Implementations return the text lines to display in an overlay window.
/// The runner queries all registered providers when rendering overlay windows.
pub trait OverlayContentProvider: Send + Sync {
    /// Get content for a specific overlay window.
    ///
    /// Returns `None` if this provider doesn't have content for the window.
    /// Returns `Some(lines)` where each line is a pre-formatted string.
    fn content_for(&self, window: WindowId) -> Option<Vec<String>>;
}

/// Registry for overlay content providers.
///
/// Modules register their content providers during init using string keys.
/// The runner queries providers generically without knowing about specific modules.
pub type OverlayContentRegistry =
    MultiServiceRegistry<OverlayContentKey, dyn OverlayContentProvider>;

// ============================================================================
// Reference Implementation: HashMap-based Storage
// ============================================================================

use std::{collections::HashMap, sync::RwLock};

/// Reference implementation of overlay content storage.
///
/// A simple thread-safe `HashMap` storage for overlay window content.
/// Modules call `set_content()` to register content when showing a popup,
/// and the screen handler queries via `content_for()` when rendering.
///
/// # Thread Safety
///
/// Uses `RwLock` for safe concurrent access:
/// - Multiple readers can query content simultaneously
/// - Single writer for updates (set/remove)
#[derive(Debug, Default)]
pub struct OverlayContentStorage {
    content: RwLock<HashMap<WindowId, Vec<String>>>,
}

impl OverlayContentStorage {
    /// Create a new empty overlay content storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            content: RwLock::new(HashMap::new()),
        }
    }

    /// Set content for an overlay window.
    ///
    /// Replaces any existing content for the window.
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned (another thread panicked while holding it).
    pub fn set_content(&self, window: WindowId, lines: Vec<String>) {
        self.content.write().unwrap().insert(window, lines);
    }

    /// Remove content for an overlay window.
    ///
    /// Called when the overlay is hidden to clean up.
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned (another thread panicked while holding it).
    pub fn remove(&self, window: WindowId) {
        self.content.write().unwrap().remove(&window);
    }

    /// Check if content exists for a window.
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned (another thread panicked while holding it).
    #[must_use]
    pub fn has_content(&self, window: WindowId) -> bool {
        self.content.read().unwrap().contains_key(&window)
    }
}

impl OverlayContentProvider for OverlayContentStorage {
    fn content_for(&self, window: WindowId) -> Option<Vec<String>> {
        self.content.read().unwrap().get(&window).cloned()
    }
}

// Allow standalone registration in ServiceRegistry
impl Service for OverlayContentStorage {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_content_key() {
        let key = OverlayContentKey::new("which-key");
        assert_eq!(key.name(), "which-key");
    }

    #[test]
    fn test_overlay_content_key_equality() {
        let key1 = OverlayContentKey::new("which-key");
        let key2 = OverlayContentKey::new("which-key");
        let key3 = OverlayContentKey::new("diagnostics");

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_overlay_content_key_display() {
        let key = OverlayContentKey::new("which-key");
        assert_eq!(format!("{key}"), "OverlayContentKey(which-key)");
    }

    #[test]
    fn test_overlay_content_key_service_name() {
        assert_eq!(OverlayContentKey::service_name(), "OverlayContent");
    }

    // Storage tests

    #[test]
    fn test_storage_set_and_get() {
        let storage = OverlayContentStorage::new();
        let window_id = WindowId::from_raw(1);

        storage.set_content(window_id, vec!["line 1".into(), "line 2".into()]);

        let content = storage.content_for(window_id);
        assert!(content.is_some());
        let lines = content.unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "line 1");
        assert_eq!(lines[1], "line 2");
    }

    #[test]
    fn test_storage_unknown_window() {
        let storage = OverlayContentStorage::new();
        let window_id = WindowId::from_raw(99);

        assert!(storage.content_for(window_id).is_none());
    }

    #[test]
    fn test_storage_remove() {
        let storage = OverlayContentStorage::new();
        let window_id = WindowId::from_raw(1);

        storage.set_content(window_id, vec!["content".into()]);
        assert!(storage.content_for(window_id).is_some());

        storage.remove(window_id);
        assert!(storage.content_for(window_id).is_none());
    }

    #[test]
    fn test_storage_replace() {
        let storage = OverlayContentStorage::new();
        let window_id = WindowId::from_raw(1);

        storage.set_content(window_id, vec!["old".into()]);
        storage.set_content(window_id, vec!["new".into()]);

        let content = storage.content_for(window_id).unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0], "new");
    }

    #[test]
    fn test_storage_has_content() {
        let storage = OverlayContentStorage::new();
        let window_id = WindowId::from_raw(1);

        assert!(!storage.has_content(window_id));

        storage.set_content(window_id, vec!["content".into()]);
        assert!(storage.has_content(window_id));

        storage.remove(window_id);
        assert!(!storage.has_content(window_id));
    }

    #[test]
    fn test_storage_multiple_windows() {
        let storage = OverlayContentStorage::new();
        let window1 = WindowId::from_raw(1);
        let window2 = WindowId::from_raw(2);

        storage.set_content(window1, vec!["window 1".into()]);
        storage.set_content(window2, vec!["window 2".into()]);

        assert_eq!(storage.content_for(window1).unwrap()[0], "window 1");
        assert_eq!(storage.content_for(window2).unwrap()[0], "window 2");
    }
}
