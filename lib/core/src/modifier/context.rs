//! Modifier evaluation context

use crate::{
    modd::{EditMode, SubMode},
    ui_component::ComponentId,
};

/// Context for evaluating modifiers
///
/// Contains all the information needed to determine which modifiers should apply.
#[derive(Debug, Clone)]
pub struct ModifierContext<'a> {
    /// Current interactor ID (extensible, trait-based)
    pub interactor_id: ComponentId,
    /// Current edit mode (Normal, Insert, Visual)
    pub edit_mode: &'a EditMode,
    /// Current sub-mode (Command, `OperatorPending`, Leap, etc.)
    pub sub_mode: &'a SubMode,
    /// Filetype of the current buffer (e.g., "rust", "python")
    pub filetype: Option<&'a str>,
    /// Current window ID
    pub window_id: usize,
    /// Current buffer ID
    pub buffer_id: usize,
    /// Whether the window is the active/focused window
    pub is_window_active: bool,
    /// Whether the buffer has unsaved modifications
    pub is_modified: bool,
    /// Whether the window is floating
    pub is_floating: bool,
}

impl<'a> ModifierContext<'a> {
    /// Create a new modifier context
    #[must_use]
    pub const fn new(
        interactor_id: ComponentId,
        edit_mode: &'a EditMode,
        sub_mode: &'a SubMode,
        window_id: usize,
        buffer_id: usize,
    ) -> Self {
        Self {
            interactor_id,
            edit_mode,
            sub_mode,
            filetype: None,
            window_id,
            buffer_id,
            is_window_active: false,
            is_modified: false,
            is_floating: false,
        }
    }

    /// Set the filetype
    #[must_use]
    pub const fn with_filetype(mut self, filetype: Option<&'a str>) -> Self {
        self.filetype = filetype;
        self
    }

    /// Set whether the window is active
    #[must_use]
    pub const fn with_active(mut self, active: bool) -> Self {
        self.is_window_active = active;
        self
    }

    /// Set whether the buffer is modified
    #[must_use]
    pub const fn with_modified(mut self, modified: bool) -> Self {
        self.is_modified = modified;
        self
    }

    /// Set whether the window is floating
    #[must_use]
    pub const fn with_floating(mut self, floating: bool) -> Self {
        self.is_floating = floating;
        self
    }

    /// Check if currently in normal mode
    #[must_use]
    pub const fn is_normal(&self) -> bool {
        matches!(self.edit_mode, EditMode::Normal)
    }

    /// Check if currently in insert mode
    #[must_use]
    pub const fn is_insert(&self) -> bool {
        matches!(self.edit_mode, EditMode::Insert(_))
    }

    /// Check if currently in visual mode
    #[must_use]
    pub const fn is_visual(&self) -> bool {
        matches!(self.edit_mode, EditMode::Visual(_))
    }

    /// Check if currently in command sub-mode
    #[must_use]
    pub const fn is_command(&self) -> bool {
        matches!(self.sub_mode, SubMode::Command)
    }

    /// Check if focus is on editor
    #[must_use]
    pub fn is_editor_focused(&self) -> bool {
        self.interactor_id.0 == ComponentId::EDITOR.0
    }

    /// Check if the current interactor matches the given interactor ID
    #[must_use]
    pub fn is_interactor(&self, id: ComponentId) -> bool {
        self.interactor_id.0 == id.0
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::modd::{EditMode, SubMode},
    };

    #[test]
    fn test_context_builder() {
        let edit_mode = EditMode::Normal;
        let sub_mode = SubMode::None;

        let ctx = ModifierContext::new(ComponentId::EDITOR, &edit_mode, &sub_mode, 0, 0)
            .with_filetype(Some("rust"))
            .with_active(true)
            .with_modified(false);

        assert!(ctx.is_normal());
        assert!(ctx.is_editor_focused());
        assert_eq!(ctx.filetype, Some("rust"));
        assert!(ctx.is_window_active);
        assert!(!ctx.is_modified);
    }

    #[test]
    fn test_mode_checks() {
        let edit_mode = EditMode::Insert(crate::modd::InsertVariant::Standard);
        let sub_mode = SubMode::None;

        let ctx = ModifierContext::new(ComponentId::EDITOR, &edit_mode, &sub_mode, 0, 0);

        assert!(!ctx.is_normal());
        assert!(ctx.is_insert());
        assert!(!ctx.is_visual());
    }
}
