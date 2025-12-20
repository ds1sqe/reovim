//! Modifier trait definitions

use super::{behavior::BehaviorModifiers, context::ModifierContext, style::StyleModifiers};

/// Unique identifier for a modifier
pub type ModifierId = &'static str;

/// Trait for modifiers that can conditionally apply styling and behavior changes
///
/// Modifiers are evaluated in priority order. Higher priority modifiers
/// can override lower priority ones.
pub trait Modifier: std::fmt::Debug + Send + Sync {
    /// Unique identifier for this modifier
    fn id(&self) -> ModifierId;

    /// Check if this modifier applies to the given context
    ///
    /// Returns true if the modifier's conditions are met.
    fn matches(&self, ctx: &ModifierContext<'_>) -> bool;

    /// Get style modifiers to apply
    ///
    /// Returns None if this modifier doesn't affect styling.
    fn style_modifiers(&self) -> Option<&StyleModifiers> {
        None
    }

    /// Get behavior modifiers to apply
    ///
    /// Returns None if this modifier doesn't affect behavior.
    fn behavior_modifiers(&self) -> Option<&BehaviorModifiers> {
        None
    }

    /// Priority for this modifier (higher = applied later, can override)
    ///
    /// Default priority is 100. Built-in modifiers use:
    /// - 0-50: Base/default modifiers
    /// - 50-100: Filetype modifiers
    /// - 100-150: Mode modifiers
    /// - 150-200: State modifiers (active, modified)
    /// - 200+: User/plugin modifiers
    fn priority(&self) -> u16 {
        100
    }

    /// Description for debugging/logging
    fn description(&self) -> &'static str {
        self.id()
    }
}

/// A simple modifier that matches based on a predicate function
#[derive(Debug)]
pub struct PredicateModifier {
    id: ModifierId,
    description: &'static str,
    priority: u16,
    style: StyleModifiers,
    predicate: fn(&ModifierContext<'_>) -> bool,
}

impl PredicateModifier {
    /// Create a new predicate modifier
    #[must_use]
    pub const fn new(
        id: ModifierId,
        description: &'static str,
        priority: u16,
        style: StyleModifiers,
        predicate: fn(&ModifierContext<'_>) -> bool,
    ) -> Self {
        Self {
            id,
            description,
            priority,
            style,
            predicate,
        }
    }
}

impl Modifier for PredicateModifier {
    fn id(&self) -> ModifierId {
        self.id
    }

    fn matches(&self, ctx: &ModifierContext<'_>) -> bool {
        (self.predicate)(ctx)
    }

    fn style_modifiers(&self) -> Option<&StyleModifiers> {
        Some(&self.style)
    }

    fn priority(&self) -> u16 {
        self.priority
    }

    fn description(&self) -> &'static str {
        self.description
    }
}

/// Modifier that matches a specific filetype
#[derive(Debug)]
pub struct FiletypeModifier {
    id: ModifierId,
    filetype: &'static str,
    style: StyleModifiers,
}

impl FiletypeModifier {
    /// Create a new filetype modifier
    #[must_use]
    pub const fn new(id: ModifierId, filetype: &'static str, style: StyleModifiers) -> Self {
        Self {
            id,
            filetype,
            style,
        }
    }
}

impl Modifier for FiletypeModifier {
    fn id(&self) -> ModifierId {
        self.id
    }

    fn matches(&self, ctx: &ModifierContext<'_>) -> bool {
        ctx.filetype == Some(self.filetype)
    }

    fn style_modifiers(&self) -> Option<&StyleModifiers> {
        Some(&self.style)
    }

    fn priority(&self) -> u16 {
        75 // Filetype modifiers
    }

    fn description(&self) -> &'static str {
        self.id
    }
}

/// Modifier that matches when window is active
#[derive(Debug)]
pub struct ActiveWindowModifier {
    style: StyleModifiers,
}

impl ActiveWindowModifier {
    /// Create a new active window modifier
    #[must_use]
    pub const fn new(style: StyleModifiers) -> Self {
        Self { style }
    }
}

impl Modifier for ActiveWindowModifier {
    fn id(&self) -> ModifierId {
        "builtin:active_window"
    }

    fn matches(&self, ctx: &ModifierContext<'_>) -> bool {
        ctx.is_window_active
    }

    fn style_modifiers(&self) -> Option<&StyleModifiers> {
        Some(&self.style)
    }

    fn priority(&self) -> u16 {
        175 // State modifiers
    }

    fn description(&self) -> &'static str {
        "Active window styling"
    }
}

/// Modifier that matches when buffer is modified
#[derive(Debug)]
pub struct ModifiedBufferModifier {
    style: StyleModifiers,
}

impl ModifiedBufferModifier {
    /// Create a new modified buffer modifier
    #[must_use]
    pub const fn new(style: StyleModifiers) -> Self {
        Self { style }
    }
}

impl Modifier for ModifiedBufferModifier {
    fn id(&self) -> ModifierId {
        "builtin:modified_buffer"
    }

    fn matches(&self, ctx: &ModifierContext<'_>) -> bool {
        ctx.is_modified
    }

    fn style_modifiers(&self) -> Option<&StyleModifiers> {
        Some(&self.style)
    }

    fn priority(&self) -> u16 {
        180 // State modifiers
    }

    fn description(&self) -> &'static str {
        "Modified buffer styling"
    }
}

/// Modifier that matches insert mode
#[derive(Debug)]
pub struct InsertModeModifier {
    style: StyleModifiers,
}

impl InsertModeModifier {
    /// Create a new insert mode modifier
    #[must_use]
    pub const fn new(style: StyleModifiers) -> Self {
        Self { style }
    }
}

impl Modifier for InsertModeModifier {
    fn id(&self) -> ModifierId {
        "builtin:insert_mode"
    }

    fn matches(&self, ctx: &ModifierContext<'_>) -> bool {
        ctx.is_insert()
    }

    fn style_modifiers(&self) -> Option<&StyleModifiers> {
        Some(&self.style)
    }

    fn priority(&self) -> u16 {
        125 // Mode modifiers
    }

    fn description(&self) -> &'static str {
        "Insert mode styling"
    }
}

/// Modifier that matches visual mode
#[derive(Debug)]
pub struct VisualModeModifier {
    style: StyleModifiers,
}

impl VisualModeModifier {
    /// Create a new visual mode modifier
    #[must_use]
    pub const fn new(style: StyleModifiers) -> Self {
        Self { style }
    }
}

impl Modifier for VisualModeModifier {
    fn id(&self) -> ModifierId {
        "builtin:visual_mode"
    }

    fn matches(&self, ctx: &ModifierContext<'_>) -> bool {
        ctx.is_visual()
    }

    fn style_modifiers(&self) -> Option<&StyleModifiers> {
        Some(&self.style)
    }

    fn priority(&self) -> u16 {
        125 // Mode modifiers
    }

    fn description(&self) -> &'static str {
        "Visual mode styling"
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            interactor::InteractorId,
            modd::{EditMode, InsertVariant, SubMode},
        },
    };

    #[test]
    fn test_filetype_modifier() {
        let modifier = FiletypeModifier::new("test:rust", "rust", StyleModifiers::none());

        let edit_mode = EditMode::Normal;
        let sub_mode = SubMode::None;
        let ctx = ModifierContext::new(InteractorId::EDITOR, &edit_mode, &sub_mode, 0, 0)
            .with_filetype(Some("rust"));

        assert!(modifier.matches(&ctx));

        let ctx_python = ModifierContext::new(InteractorId::EDITOR, &edit_mode, &sub_mode, 0, 0)
            .with_filetype(Some("python"));

        assert!(!modifier.matches(&ctx_python));
    }

    #[test]
    fn test_active_window_modifier() {
        let modifier = ActiveWindowModifier::new(StyleModifiers::none());

        let edit_mode = EditMode::Normal;
        let sub_mode = SubMode::None;

        let ctx_active = ModifierContext::new(InteractorId::EDITOR, &edit_mode, &sub_mode, 0, 0)
            .with_active(true);
        let ctx_inactive = ModifierContext::new(InteractorId::EDITOR, &edit_mode, &sub_mode, 0, 0)
            .with_active(false);

        assert!(modifier.matches(&ctx_active));
        assert!(!modifier.matches(&ctx_inactive));
    }

    #[test]
    fn test_insert_mode_modifier() {
        let modifier = InsertModeModifier::new(StyleModifiers::none());

        let insert_mode = EditMode::Insert(InsertVariant::Standard);
        let normal_mode = EditMode::Normal;
        let sub_mode = SubMode::None;

        let ctx_insert = ModifierContext::new(InteractorId::EDITOR, &insert_mode, &sub_mode, 0, 0);
        let ctx_normal = ModifierContext::new(InteractorId::EDITOR, &normal_mode, &sub_mode, 0, 0);

        assert!(modifier.matches(&ctx_insert));
        assert!(!modifier.matches(&ctx_normal));
    }

    #[test]
    fn test_modifier_priority() {
        let filetype = FiletypeModifier::new("test", "rust", StyleModifiers::none());
        let active = ActiveWindowModifier::new(StyleModifiers::none());
        let insert = InsertModeModifier::new(StyleModifiers::none());

        assert!(filetype.priority() < insert.priority());
        assert!(insert.priority() < active.priority());
    }
}
