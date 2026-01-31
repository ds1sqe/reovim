//! Session extension mechanism for module-provided per-session state.
//!
//! This module provides the [`SessionExtension`] trait that modules implement
//! to store per-session policy state. The runner manages an [`ExtensionMap`]
//! for each session, allowing type-safe access to module extensions.
//!
//! # Design
//!
//! - **Mechanism (Session Driver)**: Type-erased storage via `TypeId`
//! - **Policy (Modules)**: What state to store (e.g., `VimSessionState`)
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::{SessionExtension, ExtensionMap};
//!
//! // Module defines its per-session state
//! #[derive(Default)]
//! pub struct VimSessionState {
//!     pub pending_count: Option<usize>,
//!     pub pending_register: Option<char>,
//! }
//!
//! impl SessionExtension for VimSessionState {
//!     fn create() -> Self { Self::default() }
//! }
//!
//! // Access in resolvers/commands
//! let mut extensions = ExtensionMap::new();
//! let vim = extensions.get_or_insert::<VimSessionState>();
//! vim.pending_count = Some(5);
//! ```

use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

/// Trait for module-provided per-session state.
///
/// Modules implement this trait to store policy state that varies per client
/// session. The session driver provides type-safe storage via [`ExtensionMap`].
///
/// # Requirements
///
/// - `Send + Sync`: Extensions must be thread-safe
/// - `'static`: No borrowed references (owned data only)
///
/// # Example
///
/// ```ignore
/// use reovim_driver_session::SessionExtension;
///
/// #[derive(Default)]
/// pub struct MyModuleState {
///     pub counter: usize,
/// }
///
/// impl SessionExtension for MyModuleState {
///     fn create() -> Self {
///         Self::default()
///     }
/// }
/// ```
pub trait SessionExtension: Send + Sync + 'static {
    /// Create default state for a new session.
    ///
    /// Called when the extension is first accessed for a session.
    fn create() -> Self
    where
        Self: Sized;
}

/// Type-erased extension storage using `TypeId`.
///
/// Each session has its own `ExtensionMap`. Modules access their state
/// via the generic `get` and `get_mut` methods, which use `TypeId` for
/// type-safe lookup.
///
/// # Thread Safety
///
/// `ExtensionMap` itself is not `Sync`, but the stored extensions are
/// `Send + Sync`. Access should be synchronized at the session level.
#[derive(Default)]
pub struct ExtensionMap {
    /// Type-erased storage. Key is `TypeId` of the concrete extension type.
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl ExtensionMap {
    /// Create a new empty extension map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Get extension by type (immutable).
    ///
    /// Returns `None` if the extension hasn't been inserted yet.
    #[must_use]
    pub fn get<T: SessionExtension>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref())
    }

    /// Get extension by type (mutable).
    ///
    /// Returns `None` if the extension hasn't been inserted yet.
    pub fn get_mut<T: SessionExtension>(&mut self) -> Option<&mut T> {
        self.map
            .get_mut(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_mut())
    }

    /// Get or create extension (lazy initialization).
    ///
    /// If the extension doesn't exist, creates it using `T::create()`.
    /// This is the primary way modules access their state.
    ///
    /// # Panics
    ///
    /// Panics if the stored type doesn't match `T`. This should never
    /// happen in correct code since `TypeId` is used as the key.
    pub fn get_or_insert<T: SessionExtension>(&mut self) -> &mut T {
        self.map
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(T::create()))
            .downcast_mut()
            .expect("ExtensionMap type mismatch - this is a bug")
    }

    /// Check if an extension exists.
    #[must_use]
    pub fn contains<T: SessionExtension>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }

    /// Remove an extension.
    ///
    /// Returns `true` if the extension was present.
    pub fn remove<T: SessionExtension>(&mut self) -> bool {
        self.map.remove(&TypeId::of::<T>()).is_some()
    }

    /// Get the number of extensions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Clear all extensions.
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

impl std::fmt::Debug for ExtensionMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionMap")
            .field("count", &self.map.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test extension type
    #[derive(Debug, Default, PartialEq)]
    struct TestExtension {
        value: i32,
    }

    impl SessionExtension for TestExtension {
        fn create() -> Self {
            Self { value: 42 }
        }
    }

    // Another test extension
    #[derive(Debug, Default)]
    struct AnotherExtension {
        name: String,
    }

    impl SessionExtension for AnotherExtension {
        fn create() -> Self {
            Self {
                name: "default".to_string(),
            }
        }
    }

    #[test]
    fn test_extension_map_new() {
        let map = ExtensionMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_get_nonexistent() {
        let map = ExtensionMap::new();
        assert!(map.get::<TestExtension>().is_none());
    }

    #[test]
    fn test_get_or_insert_creates() {
        let mut map = ExtensionMap::new();
        let ext = map.get_or_insert::<TestExtension>();
        assert_eq!(ext.value, 42); // Default from create()
    }

    #[test]
    fn test_get_or_insert_returns_existing() {
        let mut map = ExtensionMap::new();

        // First access creates with default
        map.get_or_insert::<TestExtension>().value = 100;

        // Second access returns existing
        let ext = map.get_or_insert::<TestExtension>();
        assert_eq!(ext.value, 100);
    }

    #[test]
    fn test_get_after_insert() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<TestExtension>().value = 99;

        let ext = map.get::<TestExtension>();
        assert!(ext.is_some());
        assert_eq!(ext.unwrap().value, 99);
    }

    #[test]
    fn test_get_mut() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<TestExtension>();

        if let Some(ext) = map.get_mut::<TestExtension>() {
            ext.value = 200;
        }

        assert_eq!(map.get::<TestExtension>().unwrap().value, 200);
    }

    #[test]
    fn test_multiple_extensions() {
        let mut map = ExtensionMap::new();

        map.get_or_insert::<TestExtension>().value = 1;
        map.get_or_insert::<AnotherExtension>().name = "hello".to_string();

        assert_eq!(map.len(), 2);
        assert_eq!(map.get::<TestExtension>().unwrap().value, 1);
        assert_eq!(map.get::<AnotherExtension>().unwrap().name, "hello");
    }

    #[test]
    fn test_contains() {
        let mut map = ExtensionMap::new();
        assert!(!map.contains::<TestExtension>());

        map.get_or_insert::<TestExtension>();
        assert!(map.contains::<TestExtension>());
    }

    #[test]
    fn test_remove() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<TestExtension>();

        assert!(map.remove::<TestExtension>());
        assert!(!map.contains::<TestExtension>());
        assert!(!map.remove::<TestExtension>()); // Already removed
    }

    #[test]
    fn test_clear() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<TestExtension>();
        map.get_or_insert::<AnotherExtension>();

        assert_eq!(map.len(), 2);
        map.clear();
        assert!(map.is_empty());
    }

    #[test]
    fn test_debug_impl() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<TestExtension>();

        let debug = format!("{map:?}");
        assert!(debug.contains("ExtensionMap"));
        assert!(debug.contains("count"));
    }
}
