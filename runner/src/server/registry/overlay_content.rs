//! Overlay content storage for runner.
//!
//! Provides a concrete implementation of `OverlayContentProvider` for
//! modules to store and retrieve overlay window content.
//!
//! # Architecture
//!
//! This follows the mechanism/policy separation:
//! - **Mechanism** (display driver): `OverlayContentProvider` trait
//! - **Policy** (this runner): `RunnerOverlayContent` storage implementation
//!
//! Modules register their content via `set_content()` when showing popups,
//! and the screen handler queries via `content_for()` when rendering.

use std::{collections::HashMap, sync::RwLock};

use reovim_driver_display::{OverlayContentProvider, WindowId};

/// Runner-side storage for overlay window content.
///
/// Thread-safe storage of rendered content for overlay windows.
/// Modules call `set_content()` to register content when showing a popup,
/// and `remove()` to clean up when hiding.
///
/// # Thread Safety
///
/// Uses `RwLock` for safe concurrent access:
/// - Multiple readers can query content simultaneously
/// - Single writer for updates (set/remove)
#[derive(Debug, Default)]
pub struct RunnerOverlayContent {
    content: RwLock<HashMap<WindowId, Vec<String>>>,
}

impl RunnerOverlayContent {
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
    /// Panics if the `RwLock` is poisoned.
    pub fn set_content(&self, window: WindowId, lines: Vec<String>) {
        self.content.write().unwrap().insert(window, lines);
    }

    /// Remove content for an overlay window.
    ///
    /// Called when the overlay is hidden to clean up.
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned.
    pub fn remove(&self, window: WindowId) {
        self.content.write().unwrap().remove(&window);
    }

    /// Check if content exists for a window.
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned.
    #[must_use]
    pub fn has_content(&self, window: WindowId) -> bool {
        self.content.read().unwrap().contains_key(&window)
    }
}

impl OverlayContentProvider for RunnerOverlayContent {
    fn content_for(&self, window: WindowId) -> Option<Vec<String>> {
        self.content.read().unwrap().get(&window).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get_content() {
        let storage = RunnerOverlayContent::new();
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
    fn test_content_for_unknown_window() {
        let storage = RunnerOverlayContent::new();
        let window_id = WindowId::from_raw(99);

        assert!(storage.content_for(window_id).is_none());
    }

    #[test]
    fn test_remove_content() {
        let storage = RunnerOverlayContent::new();
        let window_id = WindowId::from_raw(1);

        storage.set_content(window_id, vec!["content".into()]);
        assert!(storage.content_for(window_id).is_some());

        storage.remove(window_id);
        assert!(storage.content_for(window_id).is_none());
    }

    #[test]
    fn test_replace_content() {
        let storage = RunnerOverlayContent::new();
        let window_id = WindowId::from_raw(1);

        storage.set_content(window_id, vec!["old".into()]);
        storage.set_content(window_id, vec!["new".into()]);

        let content = storage.content_for(window_id).unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0], "new");
    }

    #[test]
    fn test_has_content() {
        let storage = RunnerOverlayContent::new();
        let window_id = WindowId::from_raw(1);

        assert!(!storage.has_content(window_id));

        storage.set_content(window_id, vec!["content".into()]);
        assert!(storage.has_content(window_id));

        storage.remove(window_id);
        assert!(!storage.has_content(window_id));
    }

    #[test]
    fn test_multiple_windows() {
        let storage = RunnerOverlayContent::new();
        let window1 = WindowId::from_raw(1);
        let window2 = WindowId::from_raw(2);

        storage.set_content(window1, vec!["window 1".into()]);
        storage.set_content(window2, vec!["window 2".into()]);

        assert_eq!(storage.content_for(window1).unwrap()[0], "window 1");
        assert_eq!(storage.content_for(window2).unwrap()[0], "window 2");
    }
}
