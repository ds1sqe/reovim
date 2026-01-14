//! Editor settings and configuration

use crate::modd::{EditMode, ModeState, VisualVariant};

/// Represents when virtual edit is allowed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VirtualEditMode {
    #[default]
    None, // Never allow virtual positions (default, current behavior)
    All,    // Always allow virtual positions
    Block,  // Only in visual block mode
    Insert, // Only in insert mode
}

/// Configuration for virtual edit behavior
#[derive(Debug, Clone, Default)]
pub struct VirtualEditConfig {
    pub modes: Vec<VirtualEditMode>,
}

impl VirtualEditConfig {
    /// Check if virtual edit is allowed for the current mode
    #[must_use]
    pub fn is_virtual_allowed(&self, mode: &ModeState) -> bool {
        if self.modes.contains(&VirtualEditMode::All) {
            return true;
        }
        if self.modes.contains(&VirtualEditMode::Block)
            && matches!(mode.edit_mode, EditMode::Visual(VisualVariant::Block))
        {
            return true;
        }
        if self.modes.contains(&VirtualEditMode::Insert) && mode.is_insert() {
            return true;
        }
        false
    }
}

/// Editor-wide settings
#[derive(Debug, Clone, Default)]
pub struct EditorSettings {
    pub virtual_edit: VirtualEditConfig,
}
