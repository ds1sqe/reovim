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

use tokio::sync::mpsc;

use crate::{
    animation::{AnimationHandle, AnimationState},
    bind::KeyMap,
    command::CommandRegistry,
    completion::SharedCompletionFactory,
    decoration::SharedDecorationFactory,
    event::InnerEvent,
    render::{RenderStage, RenderStageRegistry},
    syntax::SharedSyntaxFactory,
    textobject::SharedSemanticTextObjectSource,
    visibility::{BufferVisibilitySource, NoOpBufferVisibility},
};

use super::statusline::SharedStatuslineSectionProvider;

use tokio::sync::RwLock as TokioRwLock;

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
    /// Decoration factory for creating decoration providers (provided by language plugins)
    decoration_factory: RwLock<Option<SharedDecorationFactory>>,
    /// Completion factory for providing auto-completion (provided by completion plugin)
    completion_factory: RwLock<Option<SharedCompletionFactory>>,
    /// Render stage registry for delayed stage registration from init_state()
    render_stages: RwLock<Option<Arc<RwLock<RenderStageRegistry>>>>,
    /// Plugin windows for unified rendering
    plugin_windows: RwLock<Vec<Arc<dyn PluginWindow>>>,
    /// Left panel width (set by left-side plugins like explorer)
    left_panel_width: RwLock<u16>,
    /// Right panel width (set by right-side plugins like outline)
    right_panel_width: RwLock<u16>,
    /// Animation handle for starting/stopping effects
    animation_handle: RwLock<Option<AnimationHandle>>,
    /// Shared animation state for querying active effects during render
    animation_state: RwLock<Option<Arc<TokioRwLock<AnimationState>>>>,
    /// Inner event sender for plugins that need to send InnerEvent (e.g., completion saturator)
    inner_event_tx: RwLock<Option<mpsc::Sender<InnerEvent>>>,
    /// Keymap reference for plugins that need to access keybindings
    keymap: RwLock<Option<Arc<KeyMap>>>,
    /// Command registry reference for plugins that need command metadata
    command_registry: RwLock<Option<Arc<CommandRegistry>>>,
    /// Current pending keys display string (updated by runtime on PendingKeysEvent)
    pending_keys: RwLock<String>,
    /// Statusline section provider (used by statusline plugin)
    statusline_provider: RwLock<Option<SharedStatuslineSectionProvider>>,
}

impl std::fmt::Debug for PluginStateRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.states.read().map_or(0, |s| s.len());
        let has_visibility = self.visibility_source.read().is_ok_and(|s| s.is_some());
        let has_text_object = self.text_object_source.read().is_ok_and(|s| s.is_some());
        let has_syntax_factory = self.syntax_factory.read().is_ok_and(|s| s.is_some());
        let has_decoration_factory = self.decoration_factory.read().is_ok_and(|s| s.is_some());
        let has_completion_factory = self.completion_factory.read().is_ok_and(|s| s.is_some());
        let has_render_stages = self.render_stages.read().is_ok_and(|s| s.is_some());
        let has_animation = self.animation_handle.read().is_ok_and(|s| s.is_some());
        let has_animation_state = self.animation_state.read().is_ok_and(|s| s.is_some());
        let plugin_windows_count = self.plugin_windows.read().map_or(0, |p| p.len());
        let left_panel = self.left_panel_width.read().map_or(0, |w| *w);
        let right_panel = self.right_panel_width.read().map_or(0, |w| *w);
        let has_inner_event_tx = self.inner_event_tx.read().is_ok_and(|s| s.is_some());
        let has_keymap = self.keymap.read().is_ok_and(|k| k.is_some());
        let has_command_registry = self.command_registry.read().is_ok_and(|r| r.is_some());
        let pending_keys_len = self.pending_keys.read().map_or(0, |k| k.len());
        let has_statusline_provider = self.statusline_provider.read().is_ok_and(|p| p.is_some());
        f.debug_struct("PluginStateRegistry")
            .field("state_count", &count)
            .field("has_visibility_source", &has_visibility)
            .field("has_text_object_source", &has_text_object)
            .field("has_syntax_factory", &has_syntax_factory)
            .field("has_decoration_factory", &has_decoration_factory)
            .field("has_completion_factory", &has_completion_factory)
            .field("has_render_stages", &has_render_stages)
            .field("has_animation_handle", &has_animation)
            .field("has_animation_state", &has_animation_state)
            .field("plugin_windows_count", &plugin_windows_count)
            .field("left_panel_width", &left_panel)
            .field("right_panel_width", &right_panel)
            .field("has_inner_event_tx", &has_inner_event_tx)
            .field("has_keymap", &has_keymap)
            .field("has_command_registry", &has_command_registry)
            .field("pending_keys_len", &pending_keys_len)
            .field("has_statusline_provider", &has_statusline_provider)
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
            decoration_factory: RwLock::new(None),
            completion_factory: RwLock::new(None),
            render_stages: RwLock::new(None),
            plugin_windows: RwLock::new(Vec::new()),
            left_panel_width: RwLock::new(0),
            right_panel_width: RwLock::new(0),
            animation_handle: RwLock::new(None),
            animation_state: RwLock::new(None),
            inner_event_tx: RwLock::new(None),
            keymap: RwLock::new(None),
            command_registry: RwLock::new(None),
            pending_keys: RwLock::new(String::new()),
            statusline_provider: RwLock::new(None),
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

    /// Set the decoration factory (used by language plugins like markdown)
    ///
    /// This allows language plugins to provide visual decorations (conceals,
    /// backgrounds, inline styles) without the runtime needing to know specifics.
    pub fn set_decoration_factory(&self, factory: SharedDecorationFactory) {
        *self.decoration_factory.write().unwrap() = Some(factory);
    }

    /// Get the decoration factory
    ///
    /// Returns the registered decoration factory, or None if none is registered.
    #[must_use]
    pub fn decoration_factory(&self) -> Option<SharedDecorationFactory> {
        self.decoration_factory.read().unwrap().clone()
    }

    /// Set the completion factory (used by completion plugin)
    ///
    /// This allows the completion plugin to provide auto-completion
    /// without the runtime needing to know about completion internals.
    pub fn set_completion_factory(&self, factory: SharedCompletionFactory) {
        *self.completion_factory.write().unwrap() = Some(factory);
    }

    /// Get the completion factory
    ///
    /// Returns the registered completion factory, or None if none is registered.
    #[must_use]
    pub fn completion_factory(&self) -> Option<SharedCompletionFactory> {
        self.completion_factory.read().unwrap().clone()
    }

    /// Set the inner event sender (called by Runtime during startup)
    ///
    /// This allows plugins like completion to spawn background tasks that
    /// can send InnerEvent (e.g., RenderSignal) back to the runtime.
    pub fn set_inner_event_tx(&self, tx: mpsc::Sender<InnerEvent>) {
        *self.inner_event_tx.write().unwrap() = Some(tx);
    }

    /// Get the inner event sender
    ///
    /// Returns the inner event sender for plugins that need to send InnerEvent
    /// from background tasks (e.g., completion saturator sending RenderSignal).
    #[must_use]
    pub fn inner_event_tx(&self) -> Option<mpsc::Sender<InnerEvent>> {
        self.inner_event_tx.read().unwrap().clone()
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

    /// Set the keymap reference (called by Runtime after plugin loading)
    ///
    /// This allows plugins to access the full keymap for features like which-key.
    pub fn set_keymap(&self, keymap: Arc<KeyMap>) {
        *self.keymap.write().unwrap() = Some(keymap);
    }

    /// Get the keymap reference
    ///
    /// Returns the keymap for plugins that need to query keybindings.
    #[must_use]
    pub fn keymap(&self) -> Option<Arc<KeyMap>> {
        self.keymap.read().unwrap().clone()
    }

    /// Set the command registry reference (called by Runtime after plugin loading)
    ///
    /// This allows plugins to access command metadata (descriptions, etc.).
    pub fn set_command_registry(&self, registry: Arc<CommandRegistry>) {
        *self.command_registry.write().unwrap() = Some(registry);
    }

    /// Get the command registry reference
    ///
    /// Returns the command registry for plugins that need command metadata.
    #[must_use]
    pub fn command_registry(&self) -> Option<Arc<CommandRegistry>> {
        self.command_registry.read().unwrap().clone()
    }

    /// Set the current pending keys display string
    ///
    /// Called by Runtime when pending keys change (on `PendingKeysEvent`).
    pub fn set_pending_keys(&self, keys: String) {
        *self.pending_keys.write().unwrap() = keys;
    }

    /// Get the current pending keys display string
    ///
    /// Returns the current pending keys for plugins that need to track key sequences.
    #[must_use]
    pub fn pending_keys(&self) -> String {
        self.pending_keys.read().unwrap().clone()
    }

    /// Set the statusline section provider (used by statusline plugin)
    ///
    /// This allows plugins to contribute sections to the status line
    /// without core needing to know about specific plugin implementations.
    pub fn set_statusline_provider(&self, provider: SharedStatuslineSectionProvider) {
        *self.statusline_provider.write().unwrap() = Some(provider);
    }

    /// Get the statusline section provider
    ///
    /// Returns the registered statusline provider, or None if none is registered.
    #[must_use]
    pub fn statusline_provider(&self) -> Option<SharedStatuslineSectionProvider> {
        self.statusline_provider.read().unwrap().clone()
    }

    /// Set the animation handle (used by Runtime during initialization)
    ///
    /// This allows the animation system to be accessed from mode change handlers
    /// and other runtime components.
    pub fn set_animation_handle(&self, handle: AnimationHandle) {
        *self.animation_handle.write().unwrap() = Some(handle);
    }

    /// Get the animation handle
    ///
    /// Returns the animation handle for starting/stopping effects.
    #[must_use]
    pub fn animation_handle(&self) -> Option<AnimationHandle> {
        self.animation_handle.read().unwrap().clone()
    }

    /// Set the animation state (used by Runtime during initialization)
    ///
    /// This allows the animation state to be queried during rendering
    /// to check for active effects.
    pub fn set_animation_state(&self, state: Arc<TokioRwLock<AnimationState>>) {
        *self.animation_state.write().unwrap() = Some(state);
    }

    /// Get the animation state for querying active effects
    ///
    /// Returns the shared animation state.
    #[must_use]
    pub fn animation_state(&self) -> Option<Arc<TokioRwLock<AnimationState>>> {
        self.animation_state.read().unwrap().clone()
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
