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
//! # `TextInputSink`
//!
//! Extensions that accept text input (like command-line input) implement
//! [`TextInputSink`]. The resolver specifies the target via [`InputTarget`],
//! and the runner routes characters accordingly.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::{SessionExtension, ExtensionMap, TextInputSink};
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
//! // Command-line state that accepts text input
//! #[derive(Default)]
//! pub struct CmdlineState {
//!     pub buffer: String,
//! }
//!
//! impl SessionExtension for CmdlineState {
//!     fn create() -> Self { Self::default() }
//!
//!     fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
//!         Some(self)
//!     }
//! }
//!
//! impl TextInputSink for CmdlineState {
//!     fn insert_char(&mut self, ch: char) {
//!         self.buffer.push(ch);
//!     }
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

// ============================================================================
// TextInputSink - Trait for extensions that accept text input (#482)
// ============================================================================

/// Trait for extensions that can receive text input.
///
/// Implement this trait for session extensions that accept character input,
/// such as command-line input, search input, or any other text entry mode.
///
/// # Architecture (#482)
///
/// This trait enables generic input routing without string-based mode detection:
/// - **Resolver** specifies target via `ResolveResult::insert_char_to::<T>()`
/// - **Runner** routes to extension via `InputTarget::Extension(TypeId)`
/// - **Extension** receives character via this trait
///
/// # Example
///
/// ```ignore
/// use reovim_driver_session::{SessionExtension, TextInputSink};
///
/// #[derive(Default)]
/// pub struct CmdlineState {
///     pub buffer: String,
///     pub cursor: usize,
/// }
///
/// impl TextInputSink for CmdlineState {
///     fn insert_char(&mut self, ch: char) {
///         self.buffer.insert(self.cursor, ch);
///         self.cursor += ch.len_utf8();
///     }
/// }
/// ```
pub trait TextInputSink {
    /// Insert a character at the current position.
    fn insert_char(&mut self, ch: char);
}

// ============================================================================
// SessionExtension - Trait for module-provided per-session state
// ============================================================================

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

    /// Return self as a [`TextInputSink`] if this extension accepts text input.
    ///
    /// Override this method in extensions that implement `TextInputSink` to
    /// enable input routing via `InputTarget::Extension`.
    ///
    /// # Default Implementation
    ///
    /// Returns `None` - most extensions don't accept text input.
    ///
    /// # Example
    ///
    /// ```ignore
    /// impl SessionExtension for CmdlineState {
    ///     fn create() -> Self { Self::default() }
    ///
    ///     fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
    ///         Some(self)
    ///     }
    /// }
    /// ```
    fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
        None
    }
}

// ============================================================================
// SessionExtensionDyn - Object-safe wrapper for runtime access (#482)
// ============================================================================

/// Object-safe trait for runtime access to session extensions.
///
/// This trait provides a dyn-compatible interface to `SessionExtension` methods.
/// It's automatically implemented for all `SessionExtension` types via blanket impl.
///
/// # Why This Exists
///
/// `SessionExtension::create()` has `where Self: Sized`, making the trait not
/// object-safe. This wrapper provides object-safe access to:
/// - `Any` downcasting (for type-safe retrieval)
/// - `TextInputSink` access (for input routing)
pub trait SessionExtensionDyn: Send + Sync + 'static {
    /// Get as `&dyn Any` for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Get as `&mut dyn Any` for mutable downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Get as `TextInputSink` if this extension accepts text input.
    fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink>;
}

/// Blanket implementation of `SessionExtensionDyn` for all `SessionExtension` types.
impl<T: SessionExtension> SessionExtensionDyn for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
        SessionExtension::as_text_input_sink(self)
    }
}

// ============================================================================
// ExtensionMap - Type-erased extension storage
// ============================================================================

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
///
/// # `TextInputSink` Support (#482)
///
/// Extensions that implement `TextInputSink` can be accessed via
/// `get_text_input_sink_by_id()` for input routing without knowing
/// the concrete type at compile time.
#[derive(Default)]
pub struct ExtensionMap {
    /// Type-erased storage. Key is `TypeId` of the concrete extension type.
    map: HashMap<TypeId, Box<dyn SessionExtensionDyn>>,
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
            .and_then(|boxed| (**boxed).as_any().downcast_ref())
    }

    /// Get extension by type (mutable).
    ///
    /// Returns `None` if the extension hasn't been inserted yet.
    pub fn get_mut<T: SessionExtension>(&mut self) -> Option<&mut T> {
        self.map
            .get_mut(&TypeId::of::<T>())
            .and_then(|boxed| (**boxed).as_any_mut().downcast_mut())
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
        (**self
            .map
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(T::create())))
        .as_any_mut()
        .downcast_mut()
        .expect("ExtensionMap type mismatch - this is a bug")
    }

    /// Get extension as [`TextInputSink`] by type ID.
    ///
    /// This enables routing input to extensions without knowing the concrete
    /// type at compile time. Used by the runner to handle `InputTarget::Extension`.
    ///
    /// # Arguments
    ///
    /// * `type_id` - The `TypeId` of the extension (from `InputTarget::Extension`)
    ///
    /// # Returns
    ///
    /// * `Some(&mut dyn TextInputSink)` - Extension exists and implements `TextInputSink`
    /// * `None` - Extension doesn't exist or doesn't implement `TextInputSink`
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::any::TypeId;
    /// use reovim_driver_session::ExtensionMap;
    ///
    /// let mut extensions = ExtensionMap::new();
    /// let type_id = TypeId::of::<CmdlineState>();
    ///
    /// // First, ensure the extension exists
    /// extensions.get_or_insert::<CmdlineState>();
    ///
    /// // Then route input by TypeId
    /// if let Some(sink) = extensions.get_text_input_sink_by_id(type_id) {
    ///     sink.insert_char('x');
    /// }
    /// ```
    pub fn get_text_input_sink_by_id(&mut self, type_id: TypeId) -> Option<&mut dyn TextInputSink> {
        (**self.map.get_mut(&type_id)?).as_text_input_sink()
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_debug_impl() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<TestExtension>();

        let debug = format!("{map:?}");
        assert!(debug.contains("ExtensionMap"));
        assert!(debug.contains("count"));
    }

    // ========================================================================
    // TextInputSink tests (#482)
    // ========================================================================

    #[test]
    fn test_get_text_input_sink_by_id_nonexistent() {
        // Test that get_text_input_sink_by_id returns None for non-existent extension
        let mut map = ExtensionMap::new();
        let fake_type_id = std::any::TypeId::of::<String>();

        let result = map.get_text_input_sink_by_id(fake_type_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_text_input_sink_by_id_no_sink_impl() {
        // Test that get_text_input_sink_by_id returns None for extension
        // that exists but doesn't implement TextInputSink
        let mut map = ExtensionMap::new();
        map.get_or_insert::<TestExtension>(); // TestExtension doesn't impl TextInputSink

        let type_id = std::any::TypeId::of::<TestExtension>();
        let result = map.get_text_input_sink_by_id(type_id);
        assert!(result.is_none()); // Should return None (default as_text_input_sink returns None)
    }

    // ========================================================================
    // TextInputSink full flow tests (#482)
    // ========================================================================

    /// Extension that implements `TextInputSink`
    #[derive(Debug, Default)]
    struct SinkExtension {
        buffer: String,
    }

    impl SessionExtension for SinkExtension {
        fn create() -> Self {
            Self {
                buffer: String::new(),
            }
        }

        fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
            Some(self)
        }
    }

    impl TextInputSink for SinkExtension {
        fn insert_char(&mut self, ch: char) {
            self.buffer.push(ch);
        }
    }

    #[test]
    fn test_text_input_sink_via_extension_map() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<SinkExtension>();

        let type_id = std::any::TypeId::of::<SinkExtension>();
        let sink = map.get_text_input_sink_by_id(type_id);
        assert!(sink.is_some());

        // Insert characters via sink
        let sink = sink.unwrap();
        sink.insert_char('h');
        sink.insert_char('i');

        // Verify via get
        let ext = map.get::<SinkExtension>().unwrap();
        assert_eq!(ext.buffer, "hi");
    }

    #[test]
    fn test_extension_default_as_text_input_sink_returns_none() {
        let mut ext = TestExtension { value: 42 };
        assert!(SessionExtension::as_text_input_sink(&mut ext).is_none());
    }

    #[test]
    fn test_extension_dyn_as_any() {
        let ext = TestExtension { value: 42 };
        let boxed: Box<dyn SessionExtensionDyn> = Box::new(ext);

        // Should be able to downcast
        let any = boxed.as_any();
        assert!(any.downcast_ref::<TestExtension>().is_some());
    }

    #[test]
    fn test_extension_dyn_as_any_mut() {
        let ext = TestExtension { value: 42 };
        let mut boxed: Box<dyn SessionExtensionDyn> = Box::new(ext);

        let any = boxed.as_any_mut();
        let concrete = any.downcast_mut::<TestExtension>().unwrap();
        concrete.value = 100;
        assert_eq!(concrete.value, 100);
    }

    #[test]
    fn test_extension_dyn_as_text_input_sink_none() {
        let ext = TestExtension { value: 42 };
        let mut boxed: Box<dyn SessionExtensionDyn> = Box::new(ext);

        assert!(boxed.as_text_input_sink().is_none());
    }

    #[test]
    fn test_extension_dyn_as_text_input_sink_some() {
        let ext = SinkExtension::create();
        let mut boxed: Box<dyn SessionExtensionDyn> = Box::new(ext);

        let sink = boxed.as_text_input_sink();
        assert!(sink.is_some());
    }

    #[test]
    fn test_extension_map_default() {
        let map = ExtensionMap::default();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_get_mut_nonexistent() {
        let mut map = ExtensionMap::new();
        assert!(map.get_mut::<TestExtension>().is_none());
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut map = ExtensionMap::new();
        assert!(!map.remove::<TestExtension>());
    }

    #[test]
    fn test_extension_independence() {
        // Verify different extension types don't interfere
        let mut map = ExtensionMap::new();

        map.get_or_insert::<TestExtension>().value = 10;
        map.get_or_insert::<AnotherExtension>().name = "test".to_string();

        // Removing one doesn't affect the other
        assert!(map.remove::<TestExtension>());
        assert!(map.contains::<AnotherExtension>());
        assert_eq!(map.get::<AnotherExtension>().unwrap().name, "test");
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_clear_then_reinsert() {
        let mut map = ExtensionMap::new();
        map.get_or_insert::<TestExtension>().value = 99;
        map.clear();

        // After clear, get_or_insert should create fresh
        let ext = map.get_or_insert::<TestExtension>();
        assert_eq!(ext.value, 42); // Default from create()
    }
}
