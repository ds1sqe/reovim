//! Display information registry for plugin-provided mode display strings and icons
//!
//! This module provides a registry where plugins can register custom display strings
//! and icons for different modes and components. This allows plugins to define their
//! own visual representations without modifying core code.

mod builder;
pub mod icons;

pub use builder::DisplayInfoBuilder;

use std::{collections::HashMap, sync::Arc};

use crate::{
    highlight::{Style, Theme},
    modd::{ComponentId, EditMode, ModeState, SubMode, VisualVariant},
    plugin::PluginStateRegistry,
};

/// Context for rendering dynamic display strings
///
/// Provides access to plugin state registry for querying dynamic information.
pub struct DisplayContext<'a> {
    /// Plugin state registry for accessing plugin state
    pub plugin_state: &'a PluginStateRegistry,
}

/// Function type for dynamic display string generation
///
/// This callback is invoked during rendering to generate dynamic display text.
/// It receives a `DisplayContext` with access to the `PluginStateRegistry`.
pub type DynamicDisplayFn = Arc<dyn Fn(&DisplayContext<'_>) -> String + Send + Sync>;

/// Display information for a mode/component
pub struct DisplayInfo {
    /// Display string shown in status line (e.g., " NORMAL ", " INSERT ")
    pub display_string: &'static str,
    /// Icon for compact display (e.g., "󰆾 ", "󰙅 ")
    pub icon: &'static str,
    /// Style for this component's status line appearance
    pub style: crate::highlight::Style,
    /// Optional dynamic display callback - if Some, takes precedence over `display_string`
    pub dynamic_display: Option<DynamicDisplayFn>,
}

impl std::fmt::Debug for DisplayInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DisplayInfo")
            .field("display_string", &self.display_string)
            .field("icon", &self.icon)
            .field("style", &self.style)
            .field("dynamic_display", &self.dynamic_display.is_some())
            .finish()
    }
}

impl Clone for DisplayInfo {
    fn clone(&self) -> Self {
        Self {
            display_string: self.display_string,
            icon: self.icon,
            style: self.style.clone(),
            dynamic_display: self.dynamic_display.as_ref().map(Arc::clone),
        }
    }
}

impl DisplayInfo {
    /// Create new display info with display string, icon, and style
    ///
    /// Creates a static display info with no dynamic callback.
    #[must_use]
    pub const fn new(
        display_string: &'static str,
        icon: &'static str,
        style: crate::highlight::Style,
    ) -> Self {
        Self {
            display_string,
            icon,
            style,
            dynamic_display: None,
        }
    }

    /// Add a dynamic display callback
    ///
    /// When set, this callback is invoked during rendering to generate the display string.
    /// The callback takes precedence over the static `display_string`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// DisplayInfo::new(" EXPLORER ", "󰙅 ", style)
    ///     .with_dynamic(|ctx| {
    ///         ctx.plugin_state.with::<ExplorerState, _, _>(|state| {
    ///             format!(" EXPLORER ({}) ", state.file_count)
    ///         }).unwrap_or_else(|| " EXPLORER ".to_string())
    ///     })
    /// ```
    #[must_use]
    pub fn with_dynamic<F>(mut self, f: F) -> Self
    where
        F: Fn(&DisplayContext<'_>) -> String + Send + Sync + 'static,
    {
        self.dynamic_display = Some(Arc::new(f));
        self
    }
}

// Removed EditModeKey and SubModeKey enums - no longer needed
// since status line shows [INTERACTOR][MODE] as separate sections

/// Registry for plugin-provided display information
///
/// This registry allows plugins to register custom display information
/// for their components (e.g., Explorer shows " EXPLORER " with orange color).
///
/// The status line displays: [INTERACTOR][MODE] as two separate sections,
/// so each component only needs one display registration.
#[derive(Debug, Default)]
pub struct DisplayRegistry {
    /// Display info for each component (`ComponentId` -> `DisplayInfo`)
    interactors: HashMap<ComponentId, DisplayInfo>,
}

impl DisplayRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register display info for a component
    ///
    /// This is shown in the INTERACTOR section of the status line.
    /// The MODE section will separately show the edit mode (Normal/Insert/Visual).
    pub fn register_interactor(&mut self, id: ComponentId, info: DisplayInfo) {
        self.interactors.insert(id, info);
    }

    /// Get display info for a component
    ///
    /// Simply looks up the component in the interactors map.
    #[must_use]
    pub fn get_display(&self, mode: &ModeState) -> Option<&DisplayInfo> {
        self.interactors.get(&mode.interactor_id)
    }

    /// Get display string for a mode, with fallback to hierarchical display
    ///
    /// If the `DisplayInfo` has a dynamic callback, it is invoked with the
    /// provided `plugin_state` to generate the display string.
    #[must_use]
    pub fn display_string(&self, mode: &ModeState, plugin_state: &PluginStateRegistry) -> String {
        self.get_display(mode).map_or_else(
            || self.hierarchical_display(mode),
            |info| {
                info.dynamic_display.as_ref().map_or_else(
                    || info.display_string.to_string(),
                    |dynamic| {
                        let ctx = DisplayContext { plugin_state };
                        dynamic(&ctx)
                    },
                )
            },
        )
    }

    /// Get icon for interactor, with fallback to `mode_icon`
    #[must_use]
    pub fn icon(&self, mode: &ModeState) -> &'static str {
        self.get_display(mode)
            .map_or_else(|| self.mode_icon(mode), |info| info.icon)
    }

    /// Register all built-in display info
    ///
    /// This registers the default display info for the core editor component.
    /// Plugins should register their own display info via `PluginContext`.
    ///
    /// Note: Editor modes (Normal/Insert/Visual) are shown in the MODE section
    /// of the status line, not in the interactor section, so they don't need
    /// to be registered here.
    pub fn register_builtins(&mut self, theme: &crate::highlight::Theme) {
        // Editor component
        // Mode (Normal/Insert/Visual) will be shown separately in MODE section
        self.register_interactor(
            ComponentId::EDITOR,
            DisplayInfo::new(" EDITOR ", "󰈸 ", theme.statusline.interactor.clone()),
        );
    }

    /// Get mode icon for the current mode state
    ///
    /// Priority: sub-mode -> interactor -> edit mode
    #[must_use]
    pub fn mode_icon(&self, mode: &ModeState) -> &'static str {
        // 1. Sub-modes take precedence
        match &mode.sub_mode {
            SubMode::Command => return icons::core::COMMAND,
            SubMode::OperatorPending { .. } => return icons::core::OPERATOR,
            SubMode::Interactor(id) => {
                // Check if interactor has registered icon
                if let Some(info) = self.interactors.get(id)
                    && !info.icon.is_empty()
                {
                    return info.icon;
                }
                return icons::fallback::INTERACTOR;
            }
            SubMode::None => {}
        }

        // 2. Non-editor interactors check registry
        if mode.interactor_id != ComponentId::EDITOR {
            if let Some(info) = self.interactors.get(&mode.interactor_id) {
                return info.icon;
            }
            return icons::fallback::INTERACTOR;
        }

        // 3. Editor edit modes
        match &mode.edit_mode {
            EditMode::Normal => icons::core::NORMAL,
            EditMode::Insert(_) => icons::core::INSERT,
            EditMode::Visual(_) => icons::core::VISUAL,
        }
    }

    /// Get style for current mode state
    ///
    /// Priority: sub-mode -> interactor -> edit mode
    #[must_use]
    pub fn mode_style<'t>(&self, mode: &ModeState, theme: &'t Theme) -> &'t Style {
        // 1. Sub-modes take precedence
        match &mode.sub_mode {
            SubMode::Command => return &theme.statusline.mode.command,
            SubMode::OperatorPending { .. } => return &theme.statusline.mode.operator_pending,
            SubMode::Interactor(_) => return &theme.statusline.mode.normal,
            SubMode::None => {}
        }

        // 2. Non-editor interactors use normal style
        if mode.interactor_id != ComponentId::EDITOR {
            return &theme.statusline.mode.normal;
        }

        // 3. Editor edit modes
        match &mode.edit_mode {
            EditMode::Normal => &theme.statusline.mode.normal,
            EditMode::Insert(_) => &theme.statusline.mode.insert,
            EditMode::Visual(_) => &theme.statusline.mode.visual,
        }
    }

    /// Get mode name for display (e.g., "Normal", "Insert", "Command")
    #[must_use]
    pub fn mode_name(&self, mode: &ModeState) -> &'static str {
        // 1. Sub-modes take precedence
        match &mode.sub_mode {
            SubMode::Command => return "Command",
            SubMode::OperatorPending { .. } => return "Operator",
            SubMode::Interactor(id) => return Self::interactor_mode_name(id.0),
            SubMode::None => {}
        }

        // 2. Editor edit modes
        match &mode.edit_mode {
            EditMode::Normal => "Normal",
            EditMode::Insert(_) => "Insert",
            EditMode::Visual(VisualVariant::Char) => "Visual",
            EditMode::Visual(VisualVariant::Line) => "V-Line",
            EditMode::Visual(VisualVariant::Block) => "V-Block",
        }
    }

    /// Get short mode name for display (e.g., "NORMAL", "INSERT")
    #[must_use]
    pub const fn short_mode_name(&self, mode: &ModeState) -> &'static str {
        match &mode.sub_mode {
            SubMode::Command => return "COMMAND",
            SubMode::OperatorPending { .. } => return "OPERATOR",
            SubMode::Interactor(_) => return "INTERACTOR",
            SubMode::None => {}
        }

        match &mode.edit_mode {
            EditMode::Normal => "NORMAL",
            EditMode::Insert(_) => "INSERT",
            EditMode::Visual(_) => "VISUAL",
        }
    }

    /// Build hierarchical mode display: "Kind | Mode | `SubMode`"
    ///
    /// Format: `Kind | Mode` or `Kind | Mode | SubMode`
    /// Examples: `Editor | Normal`, `Editor | Normal | Window`, `Explorer | Insert`
    #[must_use]
    pub fn hierarchical_display(&self, mode: &ModeState) -> String {
        let mut parts = Vec::with_capacity(3);

        // Part 1: Kind (interactor)
        let kind = self.interactors.get(&mode.interactor_id).map_or_else(
            || Self::default_interactor_name(mode.interactor_id.0),
            |info| info.display_string.trim(),
        );
        parts.push(kind);

        // Part 2: Edit mode
        let edit = match &mode.edit_mode {
            EditMode::Normal => "Normal",
            EditMode::Insert(_) => "Insert",
            EditMode::Visual(VisualVariant::Char) => "Visual",
            EditMode::Visual(VisualVariant::Line) => "V-Line",
            EditMode::Visual(VisualVariant::Block) => "V-Block",
        };
        parts.push(edit);

        // Part 3: Sub-mode (if active)
        if let Some(sub) = Self::sub_mode_name(mode) {
            parts.push(sub);
        }

        parts.join(" | ")
    }

    /// Get display name for interactor sub-mode
    fn interactor_mode_name(name: &str) -> &'static str {
        match name {
            "filter" => "Filter",
            "input" => "Input",
            "search" => "Search",
            "prompt" => "Prompt",
            "window" => "Window",
            "leap" => "Leap",
            _ => "Active",
        }
    }

    /// Get default interactor name for unregistered components
    fn default_interactor_name(name: &str) -> &'static str {
        match name {
            "editor" | "microscope" => "Editor",
            "command_line" => "Cmd",
            "explorer" => "Explorer",
            _ => "Component",
        }
    }

    /// Get sub-mode name if active
    fn sub_mode_name(mode: &ModeState) -> Option<&'static str> {
        match &mode.sub_mode {
            SubMode::None => None,
            SubMode::Command => Some("Command"),
            SubMode::OperatorPending { .. } => Some("Operator"),
            SubMode::Interactor(id) => Some(Self::interactor_mode_name(id.0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_info_new() {
        let style = crate::highlight::Style::new();
        let info = DisplayInfo::new(" TEST ", "󰆾 ", style);
        assert_eq!(info.display_string, " TEST ");
        assert_eq!(info.icon, "󰆾 ");
    }

    #[test]
    fn test_registry_lookup() {
        let mut registry = DisplayRegistry::new();
        let style = crate::highlight::Style::new();

        // Register a component
        registry
            .register_interactor(ComponentId::EDITOR, DisplayInfo::new(" EDITOR ", "󰈸 ", style));

        // Check that we can retrieve it
        let mode = ModeState::normal();
        let display = registry.get_display(&mode);
        assert!(display.is_some());
        assert_eq!(display.unwrap().display_string, " EDITOR ");
        assert_eq!(display.unwrap().icon, "󰈸 ");
    }

    #[test]
    fn test_builtins_registration() {
        let mut registry = DisplayRegistry::new();
        let theme = crate::highlight::Theme::default();
        registry.register_builtins(&theme);

        let plugin_state = PluginStateRegistry::new();

        // Check that editor component is registered
        let normal = ModeState::normal();
        let display = registry.get_display(&normal);
        assert!(display.is_some());
        assert_eq!(display.unwrap().display_string, " EDITOR ");
        assert_eq!(display.unwrap().icon, "󰈸 ");

        // Display string should use plugin-provided text
        assert_eq!(registry.display_string(&normal, &plugin_state).as_str(), " EDITOR ");
        assert_eq!(registry.icon(&normal), "󰈸 ");
    }

    #[test]
    fn test_fallback_for_unregistered() {
        let registry = DisplayRegistry::new();
        let plugin_state = PluginStateRegistry::new();

        // Unregistered component should return None
        let mode = ModeState::normal();
        assert!(registry.get_display(&mode).is_none());

        // display_string should fall back to hierarchical display
        let display_str = registry.display_string(&mode, &plugin_state);
        assert!(!display_str.is_empty());
        assert!(display_str.contains("Editor") || display_str.contains("Normal"));

        // icon should return mode icon (not empty anymore)
        assert_eq!(registry.icon(&mode), icons::core::NORMAL);
    }

    #[test]
    fn test_mode_icon_normal() {
        let registry = DisplayRegistry::new();
        let mode = ModeState::normal();
        assert_eq!(registry.mode_icon(&mode), icons::core::NORMAL);
    }

    #[test]
    fn test_mode_icon_insert() {
        let registry = DisplayRegistry::new();
        let mode = ModeState::insert();
        assert_eq!(registry.mode_icon(&mode), icons::core::INSERT);
    }

    #[test]
    fn test_mode_icon_visual() {
        let registry = DisplayRegistry::new();
        let mode = ModeState::visual();
        assert_eq!(registry.mode_icon(&mode), icons::core::VISUAL);
    }

    #[test]
    fn test_mode_icon_command() {
        let registry = DisplayRegistry::new();
        let mode = ModeState::command();
        assert_eq!(registry.mode_icon(&mode), icons::core::COMMAND);
    }

    #[test]
    fn test_mode_style_normal() {
        let registry = DisplayRegistry::new();
        let theme = crate::highlight::Theme::default();
        let mode = ModeState::normal();
        let style = registry.mode_style(&mode, &theme);
        assert_eq!(style, &theme.statusline.mode.normal);
    }

    #[test]
    fn test_mode_style_insert() {
        let registry = DisplayRegistry::new();
        let theme = crate::highlight::Theme::default();
        let mode = ModeState::insert();
        let style = registry.mode_style(&mode, &theme);
        assert_eq!(style, &theme.statusline.mode.insert);
    }

    #[test]
    fn test_mode_style_visual() {
        let registry = DisplayRegistry::new();
        let theme = crate::highlight::Theme::default();
        let mode = ModeState::visual();
        let style = registry.mode_style(&mode, &theme);
        assert_eq!(style, &theme.statusline.mode.visual);
    }

    #[test]
    fn test_mode_name() {
        let registry = DisplayRegistry::new();

        assert_eq!(registry.mode_name(&ModeState::normal()), "Normal");
        assert_eq!(registry.mode_name(&ModeState::insert()), "Insert");
        assert_eq!(registry.mode_name(&ModeState::visual()), "Visual");
        assert_eq!(registry.mode_name(&ModeState::visual_line()), "V-Line");
        assert_eq!(registry.mode_name(&ModeState::visual_block()), "V-Block");
        assert_eq!(registry.mode_name(&ModeState::command()), "Command");
    }

    #[test]
    fn test_short_mode_name() {
        let registry = DisplayRegistry::new();

        assert_eq!(registry.short_mode_name(&ModeState::normal()), "NORMAL");
        assert_eq!(registry.short_mode_name(&ModeState::insert()), "INSERT");
        assert_eq!(registry.short_mode_name(&ModeState::visual()), "VISUAL");
        assert_eq!(registry.short_mode_name(&ModeState::command()), "COMMAND");
    }

    #[test]
    fn test_hierarchical_display() {
        let registry = DisplayRegistry::new();

        // Normal mode should produce "Editor | Normal"
        let normal = ModeState::normal();
        let display = registry.hierarchical_display(&normal);
        assert!(display.contains("Editor"));
        assert!(display.contains("Normal"));

        // Command mode should include sub-mode
        let command = ModeState::command();
        let display = registry.hierarchical_display(&command);
        assert!(display.contains("Command"));
    }

    #[test]
    fn test_dynamic_display() {
        let mut registry = DisplayRegistry::new();
        let style = crate::highlight::Style::new();
        let plugin_state = PluginStateRegistry::new();

        // Register with dynamic display
        let info = DisplayInfo::new(" STATIC ", "󰙅 ", style).with_dynamic(|_ctx| {
            // Return dynamic content
            " DYNAMIC (42) ".to_string()
        });
        registry.register_interactor(ComponentId::EDITOR, info);

        let mode = ModeState::normal();

        // display_string should use dynamic callback
        assert_eq!(registry.display_string(&mode, &plugin_state), " DYNAMIC (42) ");

        // static display_string field is still available
        let display = registry.get_display(&mode).unwrap();
        assert_eq!(display.display_string, " STATIC ");
    }
}
