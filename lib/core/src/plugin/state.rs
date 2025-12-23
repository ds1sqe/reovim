//! Plugin state management
//!
//! This module provides a type-erased registry for plugin state. Each plugin
//! can register its own state type, which is stored and accessed by `TypeId`.
//!
//! # Example
//!
//! ```ignore
//! // Define plugin state
//! struct MyPluginState {
//!     counter: i32,
//! }
//!
//! // Register state
//! registry.register(MyPluginState { counter: 0 });
//!
//! // Access state
//! registry.with_mut::<MyPluginState, _, _>(|state| {
//!     state.counter += 1;
//! });
//! ```

#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    render::{RenderStage, RenderStageRegistry},
    syntax::SharedSyntaxFactory,
    textobject::SharedSemanticTextObjectSource,
    visibility::{BufferVisibilitySource, NoOpBufferVisibility},
};

use super::PluginWindow;

/// Type-erased plugin state container
///
/// Stores plugin state instances indexed by their TypeId. Thread-safe
/// access is provided through RwLock.
pub struct PluginStateRegistry {
    states: RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
    /// Visibility source for fold/hide state (trait-based for decoupling)
    visibility_source: RwLock<Option<Arc<dyn BufferVisibilitySource>>>,
    /// Semantic text object source (provided by treesitter plugin)
    text_object_source: RwLock<Option<SharedSemanticTextObjectSource>>,
    /// Syntax factory for creating syntax providers (provided by treesitter plugin)
    syntax_factory: RwLock<Option<SharedSyntaxFactory>>,
    /// Render stage registry for delayed stage registration from init_state()
    render_stages: RwLock<Option<Arc<RwLock<RenderStageRegistry>>>>,
    /// Plugin windows for unified rendering
    plugin_windows: RwLock<Vec<Arc<dyn PluginWindow>>>,
    /// Left panel width (set by left-side plugins like explorer)
    left_panel_width: RwLock<u16>,
    /// Right panel width (set by right-side plugins like outline)
    right_panel_width: RwLock<u16>,
}

impl std::fmt::Debug for PluginStateRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.states.read().map_or(0, |s| s.len());
        let has_visibility = self.visibility_source.read().is_ok_and(|s| s.is_some());
        let has_text_object = self.text_object_source.read().is_ok_and(|s| s.is_some());
        let has_syntax_factory = self.syntax_factory.read().is_ok_and(|s| s.is_some());
        let has_render_stages = self.render_stages.read().is_ok_and(|s| s.is_some());
        let plugin_windows_count = self.plugin_windows.read().map_or(0, |p| p.len());
        let left_panel = self.left_panel_width.read().map_or(0, |w| *w);
        let right_panel = self.right_panel_width.read().map_or(0, |w| *w);
        f.debug_struct("PluginStateRegistry")
            .field("state_count", &count)
            .field("has_visibility_source", &has_visibility)
            .field("has_text_object_source", &has_text_object)
            .field("has_syntax_factory", &has_syntax_factory)
            .field("has_render_stages", &has_render_stages)
            .field("plugin_windows_count", &plugin_windows_count)
            .field("left_panel_width", &left_panel)
            .field("right_panel_width", &right_panel)
            .finish()
    }
}

impl PluginStateRegistry {
    /// Create a new empty state registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            states: RwLock::new(HashMap::new()),
            visibility_source: RwLock::new(None),
            text_object_source: RwLock::new(None),
            syntax_factory: RwLock::new(None),
            render_stages: RwLock::new(None),
            plugin_windows: RwLock::new(Vec::new()),
            left_panel_width: RwLock::new(0),
            right_panel_width: RwLock::new(0),
        }
    }

    /// Set the visibility source (used by fold plugin)
    ///
    /// This allows plugins to provide visibility information without
    /// core needing to know the concrete type.
    pub fn set_visibility_source(&self, source: Arc<dyn BufferVisibilitySource>) {
        *self.visibility_source.write().unwrap() = Some(source);
    }

    /// Get the visibility source
    ///
    /// Returns the registered visibility source, or a no-op if none is registered.
    #[must_use]
    pub fn visibility_source(&self) -> Arc<dyn BufferVisibilitySource> {
        self.visibility_source
            .read()
            .unwrap()
            .clone()
            .unwrap_or_else(|| Arc::new(NoOpBufferVisibility))
    }

    /// Set the semantic text object source (used by treesitter plugin)
    ///
    /// This allows the treesitter plugin to provide semantic text object
    /// resolution without the runtime needing to know about treesitter.
    pub fn set_text_object_source(&self, source: SharedSemanticTextObjectSource) {
        *self.text_object_source.write().unwrap() = Some(source);
    }

    /// Get the semantic text object source
    ///
    /// Returns the registered text object source, or None if none is registered.
    #[must_use]
    pub fn text_object_source(&self) -> Option<SharedSemanticTextObjectSource> {
        self.text_object_source.read().unwrap().clone()
    }

    /// Set the syntax factory (used by treesitter plugin)
    ///
    /// This allows the treesitter plugin to provide syntax highlighting
    /// for files without the runtime needing to know about treesitter.
    pub fn set_syntax_factory(&self, factory: SharedSyntaxFactory) {
        *self.syntax_factory.write().unwrap() = Some(factory);
    }

    /// Get the syntax factory
    ///
    /// Returns the registered syntax factory, or None if none is registered.
    #[must_use]
    pub fn syntax_factory(&self) -> Option<SharedSyntaxFactory> {
        self.syntax_factory.read().unwrap().clone()
    }

    /// Set the render stage registry reference (called by Runtime)
    ///
    /// This allows plugins to register render stages from init_state()
    /// where they have access to their state.
    pub fn set_render_stages(&self, registry: Arc<RwLock<RenderStageRegistry>>) {
        *self.render_stages.write().unwrap() = Some(registry);
    }

    /// Register a render stage (called by plugins from init_state())
    ///
    /// This allows plugins to register render stages after they have
    /// initialized their state in init_state().
    pub fn register_render_stage(&self, stage: Arc<dyn RenderStage>) {
        if let Some(registry) = self.render_stages.read().unwrap().as_ref() {
            registry.write().unwrap().register(stage);
        } else {
            tracing::warn!(
                stage_name = stage.name(),
                "Attempted to register render stage before registry was set"
            );
        }
    }

    /// Register a plugin window
    ///
    /// Plugin windows provide both window configuration and rendering
    /// through a single unified trait.
    pub fn register_plugin_window(&self, window: Arc<dyn PluginWindow>) {
        self.plugin_windows.write().unwrap().push(window);
        tracing::debug!("Registered plugin window");
    }

    /// Get all registered plugin windows
    ///
    /// Returns a vector of all plugin windows that have been registered.
    #[must_use]
    pub fn plugin_windows(&self) -> Vec<Arc<dyn PluginWindow>> {
        self.plugin_windows.read().unwrap().clone()
    }

    /// Set the left panel width (used by left-side plugins like explorer)
    ///
    /// This allows plugins to reserve horizontal space on the left side
    /// of the editor area. The editor windows will be offset accordingly.
    pub fn set_left_panel_width(&self, width: u16) {
        *self.left_panel_width.write().unwrap() = width;
    }

    /// Get the left panel width
    ///
    /// Returns the width reserved by left-side panels (e.g., explorer).
    #[must_use]
    pub fn left_panel_width(&self) -> u16 {
        *self.left_panel_width.read().unwrap()
    }

    /// Set the right panel width (used by right-side plugins like outline)
    ///
    /// This allows plugins to reserve horizontal space on the right side
    /// of the editor area. The editor windows will be offset accordingly.
    pub fn set_right_panel_width(&self, width: u16) {
        *self.right_panel_width.write().unwrap() = width;
    }

    /// Get the right panel width
    ///
    /// Returns the width reserved by right-side panels (e.g., outline).
    #[must_use]
    pub fn right_panel_width(&self) -> u16 {
        *self.right_panel_width.read().unwrap()
    }

    /// Register a new plugin state
    ///
    /// If state of this type already exists, it will be replaced.
    pub fn register<S: Send + Sync + 'static>(&self, state: S) {
        let mut states = self.states.write().unwrap();
        states.insert(TypeId::of::<S>(), Box::new(state));
    }

    /// Check if state of a given type is registered
    #[must_use]
    pub fn contains<S: 'static>(&self) -> bool {
        let states = self.states.read().unwrap();
        states.contains_key(&TypeId::of::<S>())
    }

    /// Get immutable access to state
    ///
    /// Returns None if state of this type is not registered.
    #[must_use]
    pub fn get<S: 'static>(&self) -> Option<StateRef<'_, S>> {
        let states = self.states.read().unwrap();
        if states.contains_key(&TypeId::of::<S>()) {
            Some(StateRef {
                _guard: states,
                _phantom: std::marker::PhantomData,
            })
        } else {
            None
        }
    }

    /// Get mutable access to state via closure
    ///
    /// This pattern avoids lifetime issues with returning mutable references
    /// while holding the lock.
    ///
    /// Returns None if state of this type is not registered.
    pub fn with_mut<S: 'static, F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut S) -> R,
    {
        let mut states = self.states.write().unwrap();
        states
            .get_mut(&TypeId::of::<S>())
            .and_then(|b| b.downcast_mut())
            .map(f)
    }

    /// Get immutable access to state via closure
    pub fn with<S: 'static, F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&S) -> R,
    {
        let states = self.states.read().unwrap();
        states
            .get(&TypeId::of::<S>())
            .and_then(|b| b.downcast_ref())
            .map(f)
    }

    /// Remove state of a given type
    ///
    /// Returns the removed state if it existed.
    pub fn remove<S: 'static>(&self) -> Option<S> {
        let mut states = self.states.write().unwrap();
        states
            .remove(&TypeId::of::<S>())
            .and_then(|b| b.downcast().ok())
            .map(|b| *b)
    }

    /// Get the number of registered state types
    #[must_use]
    pub fn len(&self) -> usize {
        self.states.read().unwrap().len()
    }

    /// Check if the registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.read().unwrap().is_empty()
    }

    /// Clear all registered states
    pub fn clear(&self) {
        self.states.write().unwrap().clear();
    }
}

impl Default for PluginStateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard for immutable state access
///
/// This is a placeholder for future optimization where we might want
/// to return a guard that holds the lock.
pub struct StateRef<'a, S> {
    _guard: std::sync::RwLockReadGuard<'a, HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
    _phantom: std::marker::PhantomData<S>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestState {
        value: i32,
    }

    #[derive(Debug, PartialEq)]
    struct OtherState {
        name: String,
    }

    #[test]
    fn test_register_and_access() {
        let registry = PluginStateRegistry::new();

        registry.register(TestState { value: 42 });

        let result = registry.with::<TestState, _, _>(|state| state.value);
        assert_eq!(result, Some(42));
    }

    #[test]
    fn test_with_mut() {
        let registry = PluginStateRegistry::new();

        registry.register(TestState { value: 0 });

        registry.with_mut::<TestState, _, _>(|state| {
            state.value += 10;
        });

        let result = registry.with::<TestState, _, _>(|state| state.value);
        assert_eq!(result, Some(10));
    }

    #[test]
    fn test_multiple_states() {
        let registry = PluginStateRegistry::new();

        registry.register(TestState { value: 42 });
        registry.register(OtherState {
            name: "test".into(),
        });

        assert!(registry.contains::<TestState>());
        assert!(registry.contains::<OtherState>());
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_missing_state() {
        let registry = PluginStateRegistry::new();

        let result = registry.with::<TestState, _, _>(|state| state.value);
        assert_eq!(result, None);
    }

    #[test]
    fn test_remove_state() {
        let registry = PluginStateRegistry::new();

        registry.register(TestState { value: 42 });
        assert!(registry.contains::<TestState>());

        let removed = registry.remove::<TestState>();
        assert_eq!(removed, Some(TestState { value: 42 }));
        assert!(!registry.contains::<TestState>());
    }

    #[test]
    fn test_replace_state() {
        let registry = PluginStateRegistry::new();

        registry.register(TestState { value: 1 });
        registry.register(TestState { value: 2 });

        let result = registry.with::<TestState, _, _>(|state| state.value);
        assert_eq!(result, Some(2));
    }
}
