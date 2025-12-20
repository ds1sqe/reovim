//! Component registry for managing UI components
//!
//! The `ComponentRegistry` manages all registered UI components and tracks
//! which component is currently active (focused).

use std::collections::HashMap;

use super::{ComponentId, UIComponent};
use crate::component::RenderContext;

/// Registry of UI components
///
/// Manages all registered components and tracks the currently active one.
/// This replaces `InteractorRegistry` with a unified component system.
#[derive(Debug)]
pub struct ComponentRegistry {
    components: HashMap<ComponentId, Box<dyn UIComponent>>,
    active: ComponentId,
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentRegistry {
    /// Create a new empty component registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
            active: ComponentId::EDITOR,
        }
    }

    /// Register a component
    ///
    /// If a component with the same ID already exists, it will be replaced.
    pub fn register(&mut self, component: Box<dyn UIComponent>) {
        let id = component.id();
        self.components.insert(id, component);
    }

    /// Unregister a component by ID
    ///
    /// Returns the component if it was registered.
    pub fn unregister(&mut self, id: ComponentId) -> Option<Box<dyn UIComponent>> {
        self.components.remove(&id)
    }

    /// Set the active (focused) component
    ///
    /// Returns `true` if the focus was changed, `false` if the component ID
    /// is not registered or not focusable.
    pub fn set_active(&mut self, id: ComponentId) -> bool {
        if let Some(component) = self.components.get(&id)
            && component.is_focusable()
        {
            self.active = id;
            return true;
        }
        false
    }

    /// Get the active component
    ///
    /// # Panics
    ///
    /// Panics if the active component is not registered (should never happen
    /// in normal use).
    #[must_use]
    pub fn active(&self) -> &dyn UIComponent {
        self.components
            .get(&self.active)
            .map(AsRef::as_ref)
            .expect("active component should always be registered")
    }

    /// Get the active component mutably
    ///
    /// # Panics
    ///
    /// Panics if the active component is not registered.
    pub fn active_mut(&mut self) -> &mut Box<dyn UIComponent> {
        self.components
            .get_mut(&self.active)
            .expect("active component should always be registered")
    }

    /// Get the active component ID
    #[must_use]
    pub const fn active_id(&self) -> ComponentId {
        self.active
    }

    /// Get a component by ID
    #[must_use]
    pub fn get(&self, id: ComponentId) -> Option<&dyn UIComponent> {
        self.components.get(&id).map(AsRef::as_ref)
    }

    /// Get a component by ID mutably
    pub fn get_mut(&mut self, id: ComponentId) -> Option<&mut Box<dyn UIComponent>> {
        self.components.get_mut(&id)
    }

    /// Check if a component is registered
    #[must_use]
    pub fn contains(&self, id: ComponentId) -> bool {
        self.components.contains_key(&id)
    }

    /// Get the number of registered components
    #[must_use]
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Check if the registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    /// Iterate over all registered component IDs
    pub fn ids(&self) -> impl Iterator<Item = &ComponentId> {
        self.components.keys()
    }

    /// Iterate over all registered components
    pub fn iter(&self) -> impl Iterator<Item = (&ComponentId, &Box<dyn UIComponent>)> {
        self.components.iter()
    }

    /// Iterate over all focusable components
    pub fn focusable(&self) -> impl Iterator<Item = &dyn UIComponent> {
        self.components
            .values()
            .filter(|c| c.is_focusable())
            .map(AsRef::as_ref)
    }

    /// Iterate over visible components in z-order
    ///
    /// Returns components sorted by `z_order` (lowest first, for bottom-to-top rendering).
    pub fn visible_sorted<'a>(
        &'a self,
        ctx: &'a RenderContext<'_>,
    ) -> impl Iterator<Item = &'a dyn UIComponent> {
        let mut visible: Vec<_> = self
            .components
            .values()
            .filter(|c| c.is_visible(ctx))
            .collect();
        visible.sort_by_key(|c| c.z_order());
        visible.into_iter().map(AsRef::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{frame::FrameBuffer, screen::LayerBounds};

    // Test component for unit tests
    #[derive(Debug)]
    struct TestComponent {
        id: ComponentId,
        focusable: bool,
        visible: bool,
        z_order: u8,
    }

    impl TestComponent {
        fn new(id: ComponentId, focusable: bool, visible: bool, z_order: u8) -> Self {
            Self {
                id,
                focusable,
                visible,
                z_order,
            }
        }
    }

    impl UIComponent for TestComponent {
        fn id(&self) -> ComponentId {
            self.id
        }

        fn display_name(&self) -> &'static str {
            "TEST"
        }

        fn z_order(&self) -> u8 {
            self.z_order
        }

        fn is_visible(&self, _ctx: &RenderContext<'_>) -> bool {
            self.visible
        }

        fn bounds(&self, _ctx: &RenderContext<'_>) -> LayerBounds {
            LayerBounds::default()
        }

        fn render_to_frame(&self, _buffer: &mut FrameBuffer, _ctx: &RenderContext<'_>) {}

        fn is_focusable(&self) -> bool {
            self.focusable
        }
    }

    #[test]
    fn test_registry_register() {
        let mut registry = ComponentRegistry::new();
        assert!(registry.is_empty());

        registry.register(Box::new(TestComponent::new(
            ComponentId::EDITOR,
            true,
            true,
            1,
        )));
        assert_eq!(registry.len(), 1);
        assert!(registry.contains(ComponentId::EDITOR));
    }

    #[test]
    fn test_registry_set_active() {
        let mut registry = ComponentRegistry::new();

        // Register focusable component
        registry.register(Box::new(TestComponent::new(
            ComponentId::EDITOR,
            true,
            true,
            1,
        )));

        // Register non-focusable component
        registry.register(Box::new(TestComponent::new(
            ComponentId::STATUS_LINE,
            false,
            true,
            0,
        )));

        // Can set active to focusable component
        assert!(registry.set_active(ComponentId::EDITOR));
        assert_eq!(registry.active_id(), ComponentId::EDITOR);

        // Cannot set active to non-focusable component
        assert!(!registry.set_active(ComponentId::STATUS_LINE));
        assert_eq!(registry.active_id(), ComponentId::EDITOR);

        // Cannot set active to unregistered component
        assert!(!registry.set_active(ComponentId::TELESCOPE));
        assert_eq!(registry.active_id(), ComponentId::EDITOR);
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry = ComponentRegistry::new();
        registry.register(Box::new(TestComponent::new(
            ComponentId::EDITOR,
            true,
            true,
            1,
        )));

        assert!(registry.contains(ComponentId::EDITOR));
        let removed = registry.unregister(ComponentId::EDITOR);
        assert!(removed.is_some());
        assert!(!registry.contains(ComponentId::EDITOR));
    }
}
