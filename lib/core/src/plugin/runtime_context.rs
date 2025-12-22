//! Runtime context for plugins
//!
//! This module provides the `RuntimeContext` that plugins receive during
//! event handling and other runtime operations. It provides controlled
//! access to editor state and services.

#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::doc_markdown)]

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::{
    event::InnerEvent,
    event_bus::{DynEvent, Event, EventSender},
    modd::ModeState,
};

use super::{PluginId, PluginStateRegistry};

/// Context provided to plugins during runtime operations
///
/// This provides plugins with controlled access to:
/// - Event emission (both legacy InnerEvent and new plugin events)
/// - Read-only mode state observation
/// - Plugin state registry access
/// - Render requests
pub struct RuntimeContext {
    /// Plugin ID for event attribution
    plugin_id: PluginId,
    /// Sender for legacy InnerEvent system
    inner_event_tx: mpsc::Sender<InnerEvent>,
    /// Sender for new event bus system
    event_bus_sender: EventSender,
    /// Current mode state (snapshot)
    mode_state: ModeState,
    /// Shared plugin state registry
    state_registry: Arc<PluginStateRegistry>,
    /// Active buffer ID
    active_buffer_id: usize,
    /// Whether a render has been requested
    render_requested: bool,
}

impl RuntimeContext {
    /// Create a new runtime context for a plugin
    #[must_use]
    pub fn new(
        plugin_id: PluginId,
        inner_event_tx: mpsc::Sender<InnerEvent>,
        event_bus_sender: EventSender,
        mode_state: ModeState,
        state_registry: Arc<PluginStateRegistry>,
        active_buffer_id: usize,
    ) -> Self {
        Self {
            plugin_id,
            inner_event_tx,
            event_bus_sender,
            mode_state,
            state_registry,
            active_buffer_id,
            render_requested: false,
        }
    }

    /// Get the plugin ID this context belongs to
    #[must_use]
    pub fn plugin_id(&self) -> &PluginId {
        &self.plugin_id
    }

    /// Get the current mode state
    #[must_use]
    pub fn mode_state(&self) -> &ModeState {
        &self.mode_state
    }

    /// Get the active buffer ID
    #[must_use]
    pub fn active_buffer_id(&self) -> usize {
        self.active_buffer_id
    }

    /// Get access to the plugin state registry
    #[must_use]
    pub fn state_registry(&self) -> &Arc<PluginStateRegistry> {
        &self.state_registry
    }

    /// Check if a render has been requested
    #[must_use]
    pub fn render_requested(&self) -> bool {
        self.render_requested
    }

    // =========================================================================
    // Event Emission
    // =========================================================================

    /// Emit a plugin event to the new event bus
    ///
    /// This is the preferred way for plugins to emit events.
    pub fn emit<E: Event>(&self, event: E) {
        self.event_bus_sender.try_send(event);
    }

    /// Emit a legacy InnerEvent
    ///
    /// Used during migration when plugins need to interact with
    /// the existing event system.
    pub fn emit_inner(&self, event: InnerEvent) {
        let _ = self.inner_event_tx.try_send(event);
    }

    /// Emit a plugin event wrapped in InnerEvent::PluginEvent
    ///
    /// This routes the event through the legacy system but marks it
    /// as a plugin event for proper dispatch.
    pub fn emit_plugin_event<E: Event>(&self, event: E) {
        let dyn_event = DynEvent::new(event);
        let inner = InnerEvent::PluginEvent {
            plugin_id: self.plugin_id.clone(),
            event: dyn_event,
        };
        let _ = self.inner_event_tx.try_send(inner);
    }

    /// Request a render after this event is processed
    pub fn request_render(&mut self) {
        self.render_requested = true;
        let _ = self.inner_event_tx.try_send(InnerEvent::RenderSignal);
    }

    // =========================================================================
    // State Access Helpers
    // =========================================================================

    /// Access plugin state immutably via closure
    ///
    /// Returns None if state of this type is not registered.
    pub fn with_state<S: 'static, F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&S) -> R,
    {
        self.state_registry.with(f)
    }

    /// Access plugin state mutably via closure
    ///
    /// Returns None if state of this type is not registered.
    pub fn with_state_mut<S: 'static, F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut S) -> R,
    {
        self.state_registry.with_mut(f)
    }
}

/// Builder for creating RuntimeContext instances
///
/// Used by the Runtime when dispatching events to plugins.
pub struct RuntimeContextBuilder {
    plugin_id: Option<PluginId>,
    inner_event_tx: Option<mpsc::Sender<InnerEvent>>,
    event_bus_sender: Option<EventSender>,
    mode_state: Option<ModeState>,
    state_registry: Option<Arc<PluginStateRegistry>>,
    active_buffer_id: usize,
}

impl RuntimeContextBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            plugin_id: None,
            inner_event_tx: None,
            event_bus_sender: None,
            mode_state: None,
            state_registry: None,
            active_buffer_id: 0,
        }
    }

    /// Set the plugin ID
    #[must_use]
    pub fn plugin_id(mut self, id: PluginId) -> Self {
        self.plugin_id = Some(id);
        self
    }

    /// Set the inner event sender
    #[must_use]
    pub fn inner_event_tx(mut self, tx: mpsc::Sender<InnerEvent>) -> Self {
        self.inner_event_tx = Some(tx);
        self
    }

    /// Set the event bus sender
    #[must_use]
    pub fn event_bus_sender(mut self, sender: EventSender) -> Self {
        self.event_bus_sender = Some(sender);
        self
    }

    /// Set the mode state
    #[must_use]
    pub fn mode_state(mut self, state: ModeState) -> Self {
        self.mode_state = Some(state);
        self
    }

    /// Set the state registry
    #[must_use]
    pub fn state_registry(mut self, registry: Arc<PluginStateRegistry>) -> Self {
        self.state_registry = Some(registry);
        self
    }

    /// Set the active buffer ID
    #[must_use]
    pub fn active_buffer_id(mut self, id: usize) -> Self {
        self.active_buffer_id = id;
        self
    }

    /// Build the RuntimeContext
    ///
    /// # Panics
    ///
    /// Panics if any required field is not set.
    #[must_use]
    pub fn build(self) -> RuntimeContext {
        RuntimeContext::new(
            self.plugin_id.expect("plugin_id is required"),
            self.inner_event_tx.expect("inner_event_tx is required"),
            self.event_bus_sender.expect("event_bus_sender is required"),
            self.mode_state.expect("mode_state is required"),
            self.state_registry.expect("state_registry is required"),
            self.active_buffer_id,
        )
    }
}

impl Default for RuntimeContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::event_bus::EventBus};

    #[test]
    fn test_runtime_context_creation() {
        let (tx, _rx) = mpsc::channel(16);
        let bus = EventBus::new(16);
        let registry = Arc::new(PluginStateRegistry::new());

        let ctx = RuntimeContext::new(
            PluginId::new("test:plugin"),
            tx,
            bus.sender(),
            ModeState::new(),
            registry,
            0,
        );

        assert_eq!(ctx.plugin_id().as_str(), "test:plugin");
        assert_eq!(ctx.active_buffer_id(), 0);
        assert!(!ctx.render_requested());
    }

    #[test]
    fn test_state_access() {
        let (tx, _rx) = mpsc::channel(16);
        let bus = EventBus::new(16);
        let registry = Arc::new(PluginStateRegistry::new());

        // Register some state
        registry.register(42i32);

        let ctx = RuntimeContext::new(
            PluginId::new("test:plugin"),
            tx,
            bus.sender(),
            ModeState::new(),
            registry,
            0,
        );

        let value = ctx.with_state::<i32, _, _>(|v| *v);
        assert_eq!(value, Some(42));
    }
}
