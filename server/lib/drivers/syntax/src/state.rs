//! Per-session syntax driver storage.
//!
//! Stores per-buffer syntax drivers using the `ExtensionMap` pattern.
//! This maintains "per-buffer" semantics while respecting kernel purity
//! (kernel depends only on arch, not on syntax drivers).
//!
//! # Architecture
//!
//! ```text
//! Session
//!   └─ ExtensionMap
//!        └─ SyntaxSessionState
//!             └─ HashMap<BufferId, Box<dyn SyntaxDriver>>
//! ```
//!
//! # Usage from Modules
//!
//! Modules access syntax drivers via `ExtensionMap`:
//! ```ignore
//! let syntax = runtime.ext::<SyntaxSessionState>();
//! if let Some(driver) = syntax.get(buffer_id) {
//!     let highlights = driver.highlights(0..1000);
//!     let folds = driver.folds();
//! }
//! ```

use std::{collections::HashMap, sync::Arc};

use {reovim_driver_session::SessionExtension, reovim_kernel::api::v1::BufferId};

use crate::{LanguageRegistry, SyntaxDriver, SyntaxDriverFactory};

/// Per-session syntax state stored in `ExtensionMap`.
///
/// Maps buffer IDs to their syntax drivers. Each buffer can have
/// at most one syntax driver (language-specific highlighting).
///
/// # Design Rationale
///
/// Originally the plan called for storing syntax drivers in the kernel's
/// `Buffer` struct. However, the kernel has a strict rule that it "depends
/// only on arch". Storing drivers here in the session layer:
///
/// - Preserves kernel purity (no syntax dependency in kernel)
/// - Maintains per-buffer semantics (each buffer has its own driver)
/// - Uses existing `ExtensionMap` pattern (consistent with `VimSessionState`)
///
/// # Thread Safety
///
/// Access should be synchronized at the session level via `with_state_mut()`.
#[derive(Default)]
pub struct SyntaxSessionState {
    /// Drivers per buffer (`BufferId.as_usize()` -> `SyntaxDriver`).
    drivers: HashMap<usize, Box<dyn SyntaxDriver>>,
    /// Optional factory for creating new drivers.
    /// Uses `Arc` for shared ownership (populated from `SyntaxFactoryStore`).
    factory: Option<Arc<dyn SyntaxDriverFactory>>,
    /// Optional language registry for detecting language from file paths.
    registry: Option<Arc<dyn LanguageRegistry>>,
}

impl SessionExtension for SyntaxSessionState {
    fn create() -> Self {
        Self::default()
    }
}

impl SyntaxSessionState {
    /// Create a new empty syntax state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the factory used to create new syntax drivers.
    ///
    /// Accepts `Arc` for shared ownership (populated from `SyntaxFactoryStore`).
    pub fn set_factory(&mut self, factory: Arc<dyn SyntaxDriverFactory>) {
        self.factory = Some(factory);
    }

    /// Get the factory (if set).
    #[must_use]
    pub fn factory(&self) -> Option<&dyn SyntaxDriverFactory> {
        self.factory.as_deref()
    }

    /// Set the language registry used for language detection.
    pub fn set_registry(&mut self, registry: Arc<dyn LanguageRegistry>) {
        self.registry = Some(registry);
    }

    /// Get the language registry (if set).
    #[must_use]
    pub fn registry(&self) -> Option<&dyn LanguageRegistry> {
        self.registry.as_deref()
    }

    /// Detect language from a file path using the registry.
    ///
    /// Returns `None` if no registry is set or the language is not recognized.
    #[must_use]
    pub fn detect_language(&self, path: &str) -> Option<String> {
        self.registry.as_ref()?.detect_from_path(path)
    }

    /// Ensure a driver exists for a buffer by detecting language from file path.
    ///
    /// Combines language detection (via registry) and driver creation (via factory).
    /// Returns `true` if a driver exists after the call.
    pub fn ensure_driver_from_path(
        &mut self,
        buffer_id: BufferId,
        path: &str,
        content: &str,
    ) -> bool {
        // If driver already exists, done
        if self.drivers.contains_key(&buffer_id.as_usize()) {
            return true;
        }

        // Detect language from path
        let Some(language_id) = self.detect_language(path) else {
            return false;
        };

        // Delegate to ensure_driver
        self.ensure_driver(buffer_id, &language_id, content)
    }

    /// Get a reference to the driver for a buffer.
    #[must_use]
    pub fn get(&self, buffer_id: BufferId) -> Option<&dyn SyntaxDriver> {
        self.drivers
            .get(&buffer_id.as_usize())
            .map(|d| &**d as &dyn SyntaxDriver)
    }

    /// Get a mutable reference to the driver for a buffer.
    pub fn get_mut(&mut self, buffer_id: BufferId) -> Option<&mut dyn SyntaxDriver> {
        self.drivers
            .get_mut(&buffer_id.as_usize())
            .map(|d| &mut **d as &mut dyn SyntaxDriver)
    }

    /// Set the driver for a buffer.
    ///
    /// Replaces any existing driver for this buffer.
    pub fn set(&mut self, buffer_id: BufferId, driver: Box<dyn SyntaxDriver>) {
        self.drivers.insert(buffer_id.as_usize(), driver);
    }

    /// Remove the driver for a buffer.
    ///
    /// Call this when a buffer is closed to clean up resources.
    pub fn remove(&mut self, buffer_id: BufferId) -> Option<Box<dyn SyntaxDriver>> {
        self.drivers.remove(&buffer_id.as_usize())
    }

    /// Check if a buffer has a syntax driver.
    #[must_use]
    pub fn has_driver(&self, buffer_id: BufferId) -> bool {
        self.drivers.contains_key(&buffer_id.as_usize())
    }

    /// Get or create a driver for a buffer.
    ///
    /// If no driver exists and a factory is set, attempts to create one
    /// for the given language. Returns `false` if:
    /// - No driver exists AND no factory is set
    /// - No driver exists AND factory doesn't support the language
    ///
    /// After calling this, use `get_mut()` to access the driver.
    pub fn ensure_driver(&mut self, buffer_id: BufferId, language_id: &str, content: &str) -> bool {
        // If driver already exists, done
        if self.drivers.contains_key(&buffer_id.as_usize()) {
            return true;
        }

        // Try to create via factory
        if let Some(factory) = &self.factory
            && let Some(mut driver) = factory.create(language_id)
        {
            // Parse initial content
            driver.parse(content);
            self.drivers.insert(buffer_id.as_usize(), driver);
            return true;
        }

        false
    }

    /// Get the number of buffers with syntax drivers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.drivers.len()
    }

    /// Check if no buffers have syntax drivers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.drivers.is_empty()
    }

    /// Clear all syntax drivers.
    pub fn clear(&mut self) {
        self.drivers.clear();
    }
}

impl std::fmt::Debug for SyntaxSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyntaxSessionState")
            .field("buffer_count", &self.drivers.len())
            .field("has_factory", &self.factory.is_some())
            .field("has_registry", &self.registry.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{ops::Range, sync::Arc};

    use crate::{Annotation, HighlightCategory, SyntaxEdit};

    /// A minimal test driver for unit tests.
    struct TestDriver {
        language: String,
        parsed: bool,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl TestDriver {
        fn new(language: &str) -> Self {
            Self {
                language: language.to_string(),
                parsed: false,
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl SyntaxDriver for TestDriver {
        fn language(&self) -> &str {
            &self.language
        }

        fn parse(&mut self, _content: &str) {
            self.parsed = true;
        }

        fn update(&mut self, _content: &str, _edit: &SyntaxEdit) {
            // No-op
        }

        fn highlights(&self, byte_range: Range<usize>) -> Vec<Annotation> {
            if self.parsed {
                vec![Annotation::new(
                    byte_range.start,
                    byte_range.end,
                    HighlightCategory::new("comment"),
                )]
            } else {
                Vec::new()
            }
        }

        fn is_parsed(&self) -> bool {
            self.parsed
        }
    }

    /// A minimal test factory.
    struct TestFactory;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl SyntaxDriverFactory for TestFactory {
        fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
            if language_id == "rust" {
                Some(Box::new(TestDriver::new("rust")))
            } else {
                None
            }
        }

        fn supported_languages(&self) -> Vec<&str> {
            vec!["rust"]
        }

        fn supports(&self, language_id: &str) -> bool {
            language_id == "rust"
        }
    }

    fn buffer_id(n: usize) -> BufferId {
        BufferId::from_raw(n)
    }

    #[test]
    fn test_syntax_session_state_new() {
        let state = SyntaxSessionState::new();
        assert!(state.is_empty());
        assert!(state.factory().is_none());
    }

    #[test]
    fn test_syntax_session_state_session_extension() {
        let state = SyntaxSessionState::create();
        assert!(state.is_empty());
        assert!(state.factory().is_none());
    }

    #[test]
    fn test_set_and_get() {
        let mut state = SyntaxSessionState::new();
        let id = buffer_id(1);

        assert!(state.get(id).is_none());

        state.set(id, Box::new(TestDriver::new("rust")));

        assert!(state.get(id).is_some());
        assert_eq!(state.get(id).unwrap().language(), "rust");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_get_mut() {
        let mut state = SyntaxSessionState::new();
        let id = buffer_id(1);

        state.set(id, Box::new(TestDriver::new("rust")));

        // Parse via mutable reference
        if let Some(driver) = state.get_mut(id) {
            assert!(!driver.is_parsed());
            driver.parse("fn main() {}");
            assert!(driver.is_parsed());
        }

        // Verify parse persisted
        assert!(state.get(id).unwrap().is_parsed());
    }

    #[test]
    fn test_remove() {
        let mut state = SyntaxSessionState::new();
        let id = buffer_id(1);

        state.set(id, Box::new(TestDriver::new("rust")));
        assert!(state.has_driver(id));

        let removed = state.remove(id);
        assert!(removed.is_some());
        assert!(!state.has_driver(id));
        assert!(state.get(id).is_none());
    }

    #[test]
    fn test_set_factory() {
        let mut state = SyntaxSessionState::new();
        assert!(state.factory().is_none());

        state.set_factory(Arc::new(TestFactory));
        assert!(state.factory().is_some());
        assert!(state.factory().unwrap().supports("rust"));
    }

    #[test]
    fn test_ensure_driver_with_factory() {
        let mut state = SyntaxSessionState::new();
        let id = buffer_id(1);

        state.set_factory(Arc::new(TestFactory));

        // Should create driver via factory
        assert!(state.ensure_driver(id, "rust", "fn main() {}"));
        assert!(state.get(id).unwrap().is_parsed());

        // Should reuse existing driver
        assert!(state.ensure_driver(id, "rust", ""));
    }

    #[test]
    fn test_ensure_driver_unsupported_language() {
        let mut state = SyntaxSessionState::new();
        let id = buffer_id(1);

        state.set_factory(Arc::new(TestFactory));

        // Factory doesn't support python
        assert!(!state.ensure_driver(id, "python", "def foo(): pass"));
    }

    #[test]
    fn test_ensure_driver_no_factory() {
        let mut state = SyntaxSessionState::new();
        let id = buffer_id(1);

        // No factory set
        assert!(!state.ensure_driver(id, "rust", "fn main() {}"));
    }

    #[test]
    fn test_multiple_buffers() {
        let mut state = SyntaxSessionState::new();
        let id1 = buffer_id(1);
        let id2 = buffer_id(2);

        state.set(id1, Box::new(TestDriver::new("rust")));
        state.set(id2, Box::new(TestDriver::new("python")));

        assert_eq!(state.len(), 2);
        assert_eq!(state.get(id1).unwrap().language(), "rust");
        assert_eq!(state.get(id2).unwrap().language(), "python");
    }

    #[test]
    fn test_clear() {
        let mut state = SyntaxSessionState::new();
        state.set(buffer_id(1), Box::new(TestDriver::new("rust")));
        state.set(buffer_id(2), Box::new(TestDriver::new("python")));

        assert_eq!(state.len(), 2);
        state.clear();
        assert!(state.is_empty());
    }

    #[test]
    fn test_debug_impl() {
        let mut state = SyntaxSessionState::new();
        state.set(buffer_id(1), Box::new(TestDriver::new("rust")));

        let debug = format!("{state:?}");
        assert!(debug.contains("SyntaxSessionState"));
        assert!(debug.contains("buffer_count"));
    }

    #[test]
    fn test_get_folds_via_driver() {
        let mut state = SyntaxSessionState::new();
        let id = buffer_id(1);
        state.set(id, Box::new(TestDriver::new("rust")));

        // TestDriver returns empty folds (default), but access works
        let folds = state.get(id).map_or_else(Vec::new, SyntaxDriver::folds);
        assert!(folds.is_empty());
    }

    #[test]
    fn test_get_folds_no_driver() {
        let state = SyntaxSessionState::new();
        let id = buffer_id(1);

        // No driver: get() returns None, graceful
        let folds = state.get(id).map_or_else(Vec::new, SyntaxDriver::folds);
        assert!(folds.is_empty());
    }

    // ========================================================================
    // Registry and ensure_driver_from_path Tests
    // ========================================================================

    fn make_test_registry() -> Arc<dyn crate::LanguageRegistry> {
        Arc::new(crate::DefaultLanguageRegistry::new(vec![
            crate::LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
            crate::LanguageInfo::new("markdown", "Markdown").with_extensions(["md"]),
        ]))
    }

    #[test]
    fn test_set_and_get_registry() {
        let mut state = SyntaxSessionState::new();
        assert!(state.registry().is_none());

        state.set_registry(make_test_registry());
        assert!(state.registry().is_some());
    }

    #[test]
    fn test_detect_language_with_registry() {
        let mut state = SyntaxSessionState::new();
        state.set_registry(make_test_registry());

        assert_eq!(state.detect_language("main.rs"), Some("rust".to_string()));
        assert_eq!(state.detect_language("README.md"), Some("markdown".to_string()));
        assert_eq!(state.detect_language("file.txt"), None);
    }

    #[test]
    fn test_detect_language_without_registry() {
        let state = SyntaxSessionState::new();
        assert_eq!(state.detect_language("main.rs"), None);
    }

    #[test]
    fn test_ensure_driver_from_path_creates_driver() {
        let mut state = SyntaxSessionState::new();
        state.set_factory(Arc::new(TestFactory));
        state.set_registry(make_test_registry());

        let id = buffer_id(1);
        assert!(state.ensure_driver_from_path(id, "main.rs", "fn main() {}"));
        assert!(state.has_driver(id));
        assert_eq!(state.get(id).unwrap().language(), "rust");
    }

    #[test]
    fn test_ensure_driver_from_path_existing_driver() {
        let mut state = SyntaxSessionState::new();
        state.set_factory(Arc::new(TestFactory));
        state.set_registry(make_test_registry());

        let id = buffer_id(1);
        state.set(id, Box::new(TestDriver::new("rust")));

        // Should return true (driver already exists)
        assert!(state.ensure_driver_from_path(id, "main.rs", ""));
    }

    #[test]
    fn test_ensure_driver_from_path_unknown_extension() {
        let mut state = SyntaxSessionState::new();
        state.set_factory(Arc::new(TestFactory));
        state.set_registry(make_test_registry());

        let id = buffer_id(1);
        assert!(!state.ensure_driver_from_path(id, "file.txt", "hello"));
        assert!(!state.has_driver(id));
    }

    #[test]
    fn test_ensure_driver_from_path_no_registry() {
        let mut state = SyntaxSessionState::new();
        state.set_factory(Arc::new(TestFactory));

        let id = buffer_id(1);
        assert!(!state.ensure_driver_from_path(id, "main.rs", "fn main() {}"));
    }

    #[test]
    fn test_ensure_driver_from_path_no_factory() {
        let mut state = SyntaxSessionState::new();
        state.set_registry(make_test_registry());

        let id = buffer_id(1);
        assert!(!state.ensure_driver_from_path(id, "main.rs", "fn main() {}"));
    }

    #[test]
    fn test_ensure_driver_from_path_unsupported_language() {
        let mut state = SyntaxSessionState::new();
        state.set_factory(Arc::new(TestFactory)); // TestFactory only supports "rust"
        state.set_registry(make_test_registry());

        let id = buffer_id(1);
        // Markdown detected but factory doesn't support it
        assert!(!state.ensure_driver_from_path(id, "README.md", "# Hello"));
    }

    #[test]
    fn test_debug_with_registry() {
        let mut state = SyntaxSessionState::new();
        state.set_registry(make_test_registry());

        let debug = format!("{state:?}");
        assert!(debug.contains("has_registry"));
    }
}
