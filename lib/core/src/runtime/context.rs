//! Runtime context trait for plugin access
//!
//! This module provides the `RuntimeContext` trait which defines
//! the minimal interface plugins need to interact with the runtime.
//!
//! This abstraction allows plugins to be tested in isolation and
//! decouples them from the concrete `Runtime` implementation.

use std::sync::Arc;

use crate::{buffer::Buffer, event_bus::EventBus, modd::ModeState, plugin::PluginStateRegistry};

/// Minimal interface for plugins to access runtime functionality
///
/// This trait abstracts over the concrete `Runtime` type, allowing
/// plugins to be tested and used without direct coupling to the
/// full runtime implementation.
///
/// # Usage
///
/// Plugins should receive `&dyn RuntimeContext` or `&mut dyn RuntimeContext`
/// instead of `&Runtime` when they need to access runtime state or emit events.
pub trait RuntimeContext {
    /// Get the plugin state registry for type-erased state access
    fn plugin_state(&self) -> &Arc<PluginStateRegistry>;

    /// Get the event bus for emitting events
    fn event_bus(&self) -> &Arc<EventBus>;

    /// Get a buffer by ID
    fn buffer(&self, id: usize) -> Option<&Buffer>;

    /// Get a mutable buffer by ID
    fn buffer_mut(&mut self, id: usize) -> Option<&mut Buffer>;

    /// Get the currently active buffer ID
    fn active_buffer_id(&self) -> usize;

    /// Get the current mode state
    fn mode(&self) -> &ModeState;

    /// Set the mode state
    fn set_mode(&mut self, mode: ModeState);

    /// Get screen dimensions (width, height)
    fn screen_size(&self) -> (u16, u16);
}

/// Extension trait for common operations on `RuntimeContext`
pub trait RuntimeContextExt: RuntimeContext {
    /// Get the active buffer
    fn active_buffer(&self) -> Option<&Buffer> {
        self.buffer(self.active_buffer_id())
    }

    /// Get the active buffer mutably
    fn active_buffer_mut(&mut self) -> Option<&mut Buffer> {
        let id = self.active_buffer_id();
        self.buffer_mut(id)
    }

    /// Emit an event to the event bus
    fn emit<E: crate::event_bus::Event + 'static>(&self, event: E) {
        self.event_bus().emit(event);
    }

    /// Access typed plugin state via closure
    ///
    /// Returns `None` if state of this type is not registered.
    fn with_state<T: 'static, F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        self.plugin_state().with::<T, F, R>(f)
    }

    /// Access typed plugin state mutably via closure
    ///
    /// Returns `None` if state of this type is not registered.
    fn with_state_mut<T: 'static, F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        self.plugin_state().with_mut::<T, F, R>(f)
    }
}

// Blanket implementation for all types implementing RuntimeContext
impl<T: RuntimeContext + ?Sized> RuntimeContextExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock implementation for testing
    struct MockRuntimeContext {
        plugin_state: Arc<PluginStateRegistry>,
        event_bus: Arc<EventBus>,
        mode: ModeState,
    }

    impl MockRuntimeContext {
        fn new() -> Self {
            Self {
                plugin_state: Arc::new(PluginStateRegistry::new()),
                event_bus: Arc::new(EventBus::new(100)),
                mode: ModeState::new(),
            }
        }
    }

    impl RuntimeContext for MockRuntimeContext {
        fn plugin_state(&self) -> &Arc<PluginStateRegistry> {
            &self.plugin_state
        }

        fn event_bus(&self) -> &Arc<EventBus> {
            &self.event_bus
        }

        fn buffer(&self, _id: usize) -> Option<&Buffer> {
            None
        }

        fn buffer_mut(&mut self, _id: usize) -> Option<&mut Buffer> {
            None
        }

        fn active_buffer_id(&self) -> usize {
            0
        }

        fn mode(&self) -> &ModeState {
            &self.mode
        }

        fn set_mode(&mut self, mode: ModeState) {
            self.mode = mode;
        }

        fn screen_size(&self) -> (u16, u16) {
            (80, 24)
        }
    }

    #[test]
    fn test_mock_context() {
        let ctx = MockRuntimeContext::new();
        assert_eq!(ctx.active_buffer_id(), 0);
        assert_eq!(ctx.screen_size(), (80, 24));
    }

    #[test]
    fn test_extension_methods() {
        let ctx = MockRuntimeContext::new();
        assert!(ctx.active_buffer().is_none());
    }
}
