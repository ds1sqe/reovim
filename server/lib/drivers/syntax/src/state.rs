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
            // Configure injection support: pass the factory so the driver
            // can create child drivers for embedded languages.
            driver.set_injection_factory(factory.clone());

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
#[path = "state_tests.rs"]
mod tests;
