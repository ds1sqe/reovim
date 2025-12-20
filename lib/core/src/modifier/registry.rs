//! Modifier registry for managing and evaluating modifiers

use std::collections::HashMap;

use super::{
    behavior::{BehaviorModifiers, KeyBindingAction, WindowBehaviorState},
    context::ModifierContext,
    style::{StyleModifiers, WindowStyleState},
    traits::{Modifier, ModifierId},
};

/// Registry for managing modifiers
pub struct ModifierRegistry {
    /// Registered modifiers sorted by priority
    modifiers: Vec<Box<dyn Modifier>>,
    /// Cache of computed styles per window
    window_style_cache: HashMap<usize, WindowStyleState>,
    /// Cache of computed behaviors per window
    window_behavior_cache: HashMap<usize, WindowBehaviorState>,
    /// Whether the cache is valid
    cache_valid: bool,
}

impl Default for ModifierRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModifierRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            modifiers: Vec::new(),
            window_style_cache: HashMap::new(),
            window_behavior_cache: HashMap::new(),
            cache_valid: false,
        }
    }

    /// Register a modifier
    pub fn register(&mut self, modifier: Box<dyn Modifier>) {
        self.modifiers.push(modifier);
        self.sort_by_priority();
        self.invalidate_cache();
    }

    /// Unregister a modifier by ID
    pub fn unregister(&mut self, id: ModifierId) {
        self.modifiers.retain(|m| m.id() != id);
        self.invalidate_cache();
    }

    /// Sort modifiers by priority (lower priority first)
    fn sort_by_priority(&mut self) {
        self.modifiers.sort_by_key(|m| m.priority());
    }

    /// Invalidate the cache
    pub fn invalidate_cache(&mut self) {
        self.cache_valid = false;
        self.window_style_cache.clear();
        self.window_behavior_cache.clear();
    }

    /// Evaluate modifiers for a context and return computed style
    #[must_use]
    pub fn evaluate(&self, ctx: &ModifierContext<'_>) -> WindowStyleState {
        let mut state = WindowStyleState::new();
        let mut merged_style = StyleModifiers::none();

        for modifier in &self.modifiers {
            if modifier.matches(ctx)
                && let Some(style) = modifier.style_modifiers()
            {
                merged_style = merged_style.merge(style);
                state.applied_modifiers.push(modifier.id());
            }
        }

        state.style = merged_style;
        state
    }

    /// Evaluate and cache style for a window
    ///
    /// # Panics
    ///
    /// This function will not panic as it uses safe `HashMap` operations.
    pub fn evaluate_cached(&mut self, ctx: &ModifierContext<'_>) -> &WindowStyleState {
        let window_id = ctx.window_id;

        // Check if we need to recompute (avoid borrow conflict)
        if !self.window_style_cache.contains_key(&window_id) {
            let state = self.evaluate(ctx);
            self.window_style_cache.insert(window_id, state);
        }

        // SAFETY: We just inserted if it didn't exist
        self.window_style_cache
            .get(&window_id)
            .expect("cache entry was just inserted")
    }

    /// Get cached style for a window (if available)
    #[must_use]
    pub fn get_cached(&self, window_id: usize) -> Option<&WindowStyleState> {
        self.window_style_cache.get(&window_id)
    }

    /// Clear cache for a specific window
    pub fn clear_window_cache(&mut self, window_id: usize) {
        self.window_style_cache.remove(&window_id);
        self.window_behavior_cache.remove(&window_id);
    }

    /// Evaluate modifiers for a context and return computed behavior
    #[must_use]
    pub fn evaluate_behavior(&self, ctx: &ModifierContext<'_>) -> WindowBehaviorState {
        let mut state = WindowBehaviorState::new();
        let mut merged_behavior = BehaviorModifiers::none();

        for modifier in &self.modifiers {
            if modifier.matches(ctx)
                && let Some(behavior) = modifier.behavior_modifiers()
            {
                merged_behavior = merged_behavior.merge(behavior);
                state.applied_modifiers.push(modifier.id());
            }
        }

        state.behavior = merged_behavior;
        state
    }

    /// Get keybinding override for a key sequence in the given context
    #[must_use]
    pub fn get_keybinding_override(
        &self,
        ctx: &ModifierContext<'_>,
        key: &str,
    ) -> Option<KeyBindingAction> {
        for modifier in self.modifiers.iter().rev() {
            if modifier.matches(ctx)
                && let Some(behavior) = modifier.behavior_modifiers()
                && let Some(action) = behavior.keybinding_overrides.get(key)
            {
                return Some(action.clone());
            }
        }
        None
    }

    /// Check if a command is disabled in the given context
    #[must_use]
    pub fn is_command_disabled(&self, ctx: &ModifierContext<'_>, cmd_id: &str) -> bool {
        for modifier in &self.modifiers {
            if modifier.matches(ctx)
                && let Some(behavior) = modifier.behavior_modifiers()
                && behavior.disabled_commands.contains(cmd_id)
            {
                return true;
            }
        }
        false
    }

    /// Get number of registered modifiers
    #[must_use]
    pub fn len(&self) -> usize {
        self.modifiers.len()
    }

    /// Check if registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modifiers.is_empty()
    }

    /// Get list of modifier IDs
    #[must_use]
    pub fn modifier_ids(&self) -> Vec<ModifierId> {
        self.modifiers.iter().map(|m| m.id()).collect()
    }
}

impl std::fmt::Debug for ModifierRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModifierRegistry")
            .field("modifiers", &self.modifier_ids())
            .field("style_cache_size", &self.window_style_cache.len())
            .field("behavior_cache_size", &self.window_behavior_cache.len())
            .field("cache_valid", &self.cache_valid)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            modd::{EditMode, SubMode},
            modifier::traits::{ActiveWindowModifier, FiletypeModifier, InsertModeModifier},
            ui_component::ComponentId,
        },
        reovim_sys::style::Color,
    };

    fn create_test_context<'a>(
        edit_mode: &'a EditMode,
        sub_mode: &'a SubMode,
    ) -> ModifierContext<'a> {
        ModifierContext::new(ComponentId::EDITOR, edit_mode, sub_mode, 0, 0)
    }

    #[test]
    fn test_empty_registry() {
        let registry = ModifierRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_register_modifier() {
        let mut registry = ModifierRegistry::new();

        registry.register(Box::new(FiletypeModifier::new(
            "test:rust",
            "rust",
            StyleModifiers::none().with_border_color(Color::Rgb { r: 255, g: 0, b: 0 }),
        )));

        assert_eq!(registry.len(), 1);
        assert!(registry.modifier_ids().contains(&"test:rust"));
    }

    #[test]
    fn test_evaluate_matching() {
        let mut registry = ModifierRegistry::new();

        registry.register(Box::new(FiletypeModifier::new(
            "test:rust",
            "rust",
            StyleModifiers::none().with_border_color(Color::Rgb { r: 255, g: 0, b: 0 }),
        )));

        let edit_mode = EditMode::Normal;
        let sub_mode = SubMode::None;
        let ctx = create_test_context(&edit_mode, &sub_mode).with_filetype(Some("rust"));

        let state = registry.evaluate(&ctx);

        assert!(state.applied_modifiers.contains(&"test:rust"));
        assert_eq!(state.style.border_color, Some(Color::Rgb { r: 255, g: 0, b: 0 }));
    }

    #[test]
    fn test_evaluate_non_matching() {
        let mut registry = ModifierRegistry::new();

        registry.register(Box::new(FiletypeModifier::new(
            "test:rust",
            "rust",
            StyleModifiers::none().with_border_color(Color::Rgb { r: 255, g: 0, b: 0 }),
        )));

        let edit_mode = EditMode::Normal;
        let sub_mode = SubMode::None;
        let ctx = create_test_context(&edit_mode, &sub_mode).with_filetype(Some("python"));

        let state = registry.evaluate(&ctx);

        assert!(state.applied_modifiers.is_empty());
        assert!(state.style.border_color.is_none());
    }

    #[test]
    fn test_priority_ordering() {
        let mut registry = ModifierRegistry::new();

        // Lower priority (75)
        registry.register(Box::new(FiletypeModifier::new(
            "test:rust",
            "rust",
            StyleModifiers::none().with_border_color(Color::Rgb { r: 255, g: 0, b: 0 }),
        )));

        // Higher priority (175)
        registry.register(Box::new(ActiveWindowModifier::new(
            StyleModifiers::none().with_border_color(Color::Rgb { r: 0, g: 255, b: 0 }),
        )));

        let edit_mode = EditMode::Normal;
        let sub_mode = SubMode::None;
        let ctx = create_test_context(&edit_mode, &sub_mode)
            .with_filetype(Some("rust"))
            .with_active(true);

        let state = registry.evaluate(&ctx);

        // Higher priority should override
        assert_eq!(state.style.border_color, Some(Color::Rgb { r: 0, g: 255, b: 0 }));
        assert_eq!(state.applied_modifiers.len(), 2);
    }

    #[test]
    fn test_multiple_modifiers() {
        let mut registry = ModifierRegistry::new();

        registry.register(Box::new(FiletypeModifier::new(
            "test:rust",
            "rust",
            StyleModifiers::none().with_border_color(Color::Rgb { r: 255, g: 0, b: 0 }),
        )));

        registry.register(Box::new(InsertModeModifier::new(
            StyleModifiers::none().with_background(Color::Rgb { r: 0, g: 0, b: 255 }),
        )));

        let edit_mode = EditMode::Insert(crate::modd::InsertVariant::Standard);
        let sub_mode = SubMode::None;
        let ctx = create_test_context(&edit_mode, &sub_mode).with_filetype(Some("rust"));

        let state = registry.evaluate(&ctx);

        // Both should apply and merge
        assert_eq!(state.applied_modifiers.len(), 2);
        assert_eq!(state.style.border_color, Some(Color::Rgb { r: 255, g: 0, b: 0 }));
        assert_eq!(state.style.background_color, Some(Color::Rgb { r: 0, g: 0, b: 255 }));
    }

    #[test]
    fn test_unregister() {
        let mut registry = ModifierRegistry::new();

        registry.register(Box::new(FiletypeModifier::new(
            "test:rust",
            "rust",
            StyleModifiers::none(),
        )));

        assert_eq!(registry.len(), 1);

        registry.unregister("test:rust");

        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_cache() {
        let mut registry = ModifierRegistry::new();

        registry.register(Box::new(ActiveWindowModifier::new(
            StyleModifiers::none().with_border_color(Color::Rgb { r: 0, g: 255, b: 0 }),
        )));

        let edit_mode = EditMode::Normal;
        let sub_mode = SubMode::None;
        let ctx = create_test_context(&edit_mode, &sub_mode).with_active(true);

        // First call computes and caches
        let _ = registry.evaluate_cached(&ctx);
        assert!(registry.get_cached(0).is_some());

        // Clear cache
        registry.clear_window_cache(0);
        assert!(registry.get_cached(0).is_none());
    }
}
