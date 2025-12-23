//! Display information registry for plugin-provided mode display strings and icons
//!
//! This module provides a registry where plugins can register custom display strings
//! and icons for different modes and components. This allows plugins to define their
//! own visual representations without modifying core code.

mod builder;

pub use builder::DisplayInfoBuilder;

use std::collections::HashMap;

use crate::modd::{ComponentId, EditMode, InsertVariant, ModeState, SubMode, VisualVariant};

/// Display information for a mode/component
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    /// Display string shown in status line (e.g., " NORMAL ", " INSERT ")
    pub display_string: &'static str,
    /// Icon for compact display (e.g., "󰆾 ", "󰙅 ")
    pub icon: &'static str,
}

impl DisplayInfo {
    /// Create new display info with both display string and icon
    #[must_use]
    pub const fn new(display_string: &'static str, icon: &'static str) -> Self {
        Self {
            display_string,
            icon,
        }
    }
}

/// Key for edit mode display lookup
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EditModeKey {
    Normal,
    Insert,
    InsertReplace,
    VisualChar,
    VisualLine,
    VisualBlock,
}

impl From<&EditMode> for EditModeKey {
    fn from(mode: &EditMode) -> Self {
        match mode {
            EditMode::Normal => Self::Normal,
            EditMode::Insert(InsertVariant::Standard) => Self::Insert,
            EditMode::Insert(InsertVariant::Replace) => Self::InsertReplace,
            EditMode::Visual(VisualVariant::Char) => Self::VisualChar,
            EditMode::Visual(VisualVariant::Line) => Self::VisualLine,
            EditMode::Visual(VisualVariant::Block) => Self::VisualBlock,
        }
    }
}

/// Key for sub-mode display lookup
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubModeKey {
    /// No sub-mode active
    None,
    /// Command line mode
    Command,
    /// Operator pending mode (d, y, c waiting for motion)
    OperatorPending,
    /// Plugin-provided interactor (identified by `ComponentId`)
    Interactor(ComponentId),
}

impl From<&SubMode> for SubModeKey {
    fn from(mode: &SubMode) -> Self {
        match mode {
            SubMode::None => Self::None,
            SubMode::Command => Self::Command,
            SubMode::OperatorPending { .. } => Self::OperatorPending,
            SubMode::Interactor(id) => Self::Interactor(*id),
        }
    }
}

/// Registry for plugin-provided display strings and icons
///
/// This registry allows plugins to register custom display information for:
/// - Component focus (e.g., Explorer shows " EXPLORER ")
/// - Edit modes within a component (e.g., Editor + Normal shows " NORMAL ")
/// - Sub-modes (e.g., Command shows " COMMAND ")
///
/// Lookups are prioritized:
/// 1. Sub-mode (if not None) - highest priority
/// 2. Component + `EditMode` combination
/// 3. Component only (default for that focus)
/// 4. Fallback to hardcoded defaults
#[derive(Debug, Default)]
pub struct DisplayRegistry {
    /// Display info for component focus (`ComponentId` -> `DisplayInfo`)
    interactors: HashMap<ComponentId, DisplayInfo>,

    /// Display info for component + edit mode combination
    /// Key: (`ComponentId`, `EditModeKey`)
    component_modes: HashMap<(ComponentId, EditModeKey), DisplayInfo>,

    /// Display info for sub-modes
    sub_modes: HashMap<SubModeKey, DisplayInfo>,
}

impl DisplayRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register display info for a component focus
    ///
    /// This is used when the component is focused but no specific edit mode
    /// display is registered.
    pub fn register_interactor(&mut self, id: ComponentId, info: DisplayInfo) {
        self.interactors.insert(id, info);
    }

    /// Register display info for a component + edit mode combination
    ///
    /// For example, Editor + Normal = " NORMAL ", Editor + Insert = " INSERT "
    pub fn register_component_mode(
        &mut self,
        id: ComponentId,
        mode: EditModeKey,
        info: DisplayInfo,
    ) {
        self.component_modes.insert((id, mode), info);
    }

    /// Register display info for a sub-mode
    ///
    /// Sub-modes take priority over component/edit mode displays
    pub fn register_sub_mode(&mut self, key: SubModeKey, info: DisplayInfo) {
        self.sub_modes.insert(key, info);
    }

    /// Get display info for a mode state
    ///
    /// Returns the most specific display info available:
    /// 1. Sub-mode (if not None)
    /// 2. Component + edit mode
    /// 3. Component only
    /// 4. None (use fallback)
    #[must_use]
    pub fn get_display(&self, mode: &ModeState) -> Option<&DisplayInfo> {
        // Priority 1: Sub-mode
        let sub_key = SubModeKey::from(&mode.sub_mode);
        if sub_key != SubModeKey::None
            && let Some(info) = self.sub_modes.get(&sub_key)
        {
            return Some(info);
        }

        // Priority 2: Component + edit mode
        let edit_key = EditModeKey::from(&mode.edit_mode);
        if let Some(info) = self.component_modes.get(&(mode.interactor_id, edit_key)) {
            return Some(info);
        }

        // Priority 3: Component only
        self.interactors.get(&mode.interactor_id)
    }

    /// Get display string for a mode, with fallback to hierarchical display
    #[must_use]
    pub fn display_string(&self, mode: &ModeState) -> String {
        self.get_display(mode)
            .map_or_else(|| mode.hierarchical_display(), |info| info.display_string.to_string())
    }

    /// Get icon for a mode, with fallback
    #[must_use]
    pub fn icon(&self, mode: &ModeState) -> &'static str {
        self.get_display(mode).map_or(" ", |info| info.icon)
    }

    /// Register all built-in display info
    ///
    /// This registers the default display strings and icons for core components.
    /// Plugins should register their own display info via `PluginContext`.
    pub fn register_builtins(&mut self) {
        // === Editor modes (core) ===
        self.register_component_mode(
            ComponentId::EDITOR,
            EditModeKey::Normal,
            DisplayInfo::new(" NORMAL ", "󰆾 "),
        );
        self.register_component_mode(
            ComponentId::EDITOR,
            EditModeKey::Insert,
            DisplayInfo::new(" INSERT ", "󰏫 "),
        );
        self.register_component_mode(
            ComponentId::EDITOR,
            EditModeKey::InsertReplace,
            DisplayInfo::new(" REPLACE ", "󰏫 "),
        );
        self.register_component_mode(
            ComponentId::EDITOR,
            EditModeKey::VisualChar,
            DisplayInfo::new(" VISUAL ", "󰒉 "),
        );
        self.register_component_mode(
            ComponentId::EDITOR,
            EditModeKey::VisualLine,
            DisplayInfo::new(" V-LINE ", "󰒉 "),
        );
        self.register_component_mode(
            ComponentId::EDITOR,
            EditModeKey::VisualBlock,
            DisplayInfo::new(" V-BLOCK ", "󰒉 "),
        );

        // === Core sub-modes ===
        self.register_sub_mode(SubModeKey::Command, DisplayInfo::new(" COMMAND ", " "));
        self.register_sub_mode(SubModeKey::OperatorPending, DisplayInfo::new(" OPERATOR ", "󰆾 "));
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::modd::OperatorType};

    #[test]
    fn test_display_info_new() {
        let info = DisplayInfo::new(" TEST ", "󰆾 ");
        assert_eq!(info.display_string, " TEST ");
        assert_eq!(info.icon, "󰆾 ");
    }

    #[test]
    fn test_edit_mode_key_conversion() {
        assert_eq!(EditModeKey::from(&EditMode::Normal), EditModeKey::Normal);
        assert_eq!(
            EditModeKey::from(&EditMode::Insert(InsertVariant::Standard)),
            EditModeKey::Insert
        );
        assert_eq!(
            EditModeKey::from(&EditMode::Insert(InsertVariant::Replace)),
            EditModeKey::InsertReplace
        );
        assert_eq!(
            EditModeKey::from(&EditMode::Visual(VisualVariant::Char)),
            EditModeKey::VisualChar
        );
    }

    #[test]
    fn test_sub_mode_key_conversion() {
        assert_eq!(SubModeKey::from(&SubMode::None), SubModeKey::None);
        assert_eq!(SubModeKey::from(&SubMode::Command), SubModeKey::Command);
        assert_eq!(
            SubModeKey::from(&SubMode::OperatorPending {
                operator: OperatorType::Delete,
                count: None
            }),
            SubModeKey::OperatorPending
        );
        assert_eq!(
            SubModeKey::from(&SubMode::Interactor(ComponentId("leap"))),
            SubModeKey::Interactor(ComponentId("leap"))
        );
    }

    #[test]
    fn test_registry_lookup_priority() {
        let mut registry = DisplayRegistry::new();

        // Register component, component+mode, and sub-mode
        registry
            .register_interactor(ComponentId::EDITOR, DisplayInfo::new(" EDITOR DEFAULT ", "E"));
        registry.register_component_mode(
            ComponentId::EDITOR,
            EditModeKey::Normal,
            DisplayInfo::new(" NORMAL ", "N"),
        );
        registry.register_sub_mode(SubModeKey::Command, DisplayInfo::new(" COMMAND ", "C"));

        // Sub-mode takes priority
        let command_mode = ModeState::command();
        assert_eq!(registry.display_string(&command_mode).as_str(), " COMMAND ");

        // Component + mode is second priority
        let normal_mode = ModeState::normal();
        assert_eq!(registry.display_string(&normal_mode).as_str(), " NORMAL ");
    }

    #[test]
    fn test_builtins_registration() {
        let mut registry = DisplayRegistry::new();
        registry.register_builtins();

        // Check editor normal mode
        let normal = ModeState::normal();
        assert_eq!(registry.display_string(&normal).as_str(), " NORMAL ");
        assert_eq!(registry.icon(&normal), "󰆾 ");

        // Check command mode
        let command = ModeState::command();
        assert_eq!(registry.display_string(&command).as_str(), " COMMAND ");

        // Check operator pending mode
        let op = ModeState::operator_pending(crate::modd::OperatorType::Delete, None);
        assert_eq!(registry.display_string(&op).as_str(), " OPERATOR ");
    }
}
