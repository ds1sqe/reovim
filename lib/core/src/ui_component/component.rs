//! Core `UIComponent` trait and `ComponentId` type
//!
//! This module provides the unified `UIComponent` trait that combines the functionality
//! of `Interactor`, `DisplayComponent`, and `Layer` traits into a single interface.

use crate::{
    bind::{EditModeKind, KeyBinding},
    component::RenderContext,
    frame::FrameBuffer,
    interactor::InputResult,
    modd::ModeState,
    screen::LayerBounds,
};

/// Unique identifier for UI components
///
/// This is the unified identifier that replaces `InteractorId` for the new component system.
/// Components are identified by a static string name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(pub &'static str);

impl ComponentId {
    // === Core constants ===
    // Plugins define their own IDs using ComponentId("plugin_name")

    /// Editor component (main text editing area)
    pub const EDITOR: Self = Self("editor");
    /// Command line component (`:` commands)
    pub const COMMAND_LINE: Self = Self("command_line");
    /// Status line component (bottom bar)
    pub const STATUS_LINE: Self = Self("status_line");
    /// Tab line component (top bar with tabs)
    pub const TAB_LINE: Self = Self("tab_line");
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for ComponentId {
    fn default() -> Self {
        Self::EDITOR
    }
}

/// Unified trait for all UI components
///
/// This trait combines the functionality of:
/// - `Interactor` - input handling
/// - `DisplayComponent` - display-only rendering
/// - `Layer` - z-ordered rendering
///
/// Components that don't handle input can use the default implementations
/// for the input methods (which return `NotHandled`).
///
/// # Examples
///
/// Display-only component (like StatusLine):
/// ```ignore
/// impl UIComponent for StatusLineComponent {
///     fn id(&self) -> ComponentId { ComponentId::STATUS_LINE }
///     fn display_name(&self) -> &'static str { "STATUS" }
///     fn z_order(&self) -> u8 { z_order::BASE }
///     fn is_visible(&self, _ctx: &RenderContext<'_>) -> bool { true }
///     fn bounds(&self, ctx: &RenderContext<'_>) -> LayerBounds { /* ... */ }
///     fn render_to_frame(&self, buffer: &mut FrameBuffer, ctx: &RenderContext<'_>) { /* ... */ }
///     // Input methods use defaults (not focusable, not handling input)
/// }
/// ```
///
/// Input-handling component (like Editor):
/// ```ignore
/// impl UIComponent for EditorComponent {
///     fn id(&self) -> ComponentId { ComponentId::EDITOR }
///     fn display_name(&self) -> &'static str { "EDITOR" }
///     fn icon(&self) -> Option<&'static str> { Some("") }
///     fn z_order(&self) -> u8 { z_order::EDITOR }
///     fn is_focusable(&self) -> bool { true }
///     fn handle_insert_char(&mut self, c: char, mode: &ModeState) -> InputResult { /* ... */ }
///     // ... other methods
/// }
/// ```
pub trait UIComponent: std::fmt::Debug + Send + Sync {
    // === Identity ===

    /// Unique identifier for this component
    fn id(&self) -> ComponentId;

    /// Display name shown in status line
    fn display_name(&self) -> &'static str;

    /// Optional icon for status line display
    fn icon(&self) -> Option<&'static str> {
        None
    }

    // === Rendering ===

    /// Z-order priority (higher draws on top)
    ///
    /// Use constants from `crate::screen::z_order` for standard values.
    fn z_order(&self) -> u8;

    /// Whether this component is currently visible
    fn is_visible(&self, ctx: &RenderContext<'_>) -> bool;

    /// Bounds of this component on screen
    fn bounds(&self, ctx: &RenderContext<'_>) -> LayerBounds;

    /// Render this component to the frame buffer
    fn render_to_frame(&self, buffer: &mut FrameBuffer, ctx: &RenderContext<'_>);

    /// Cursor position if this component owns the cursor
    ///
    /// Returns `Some((x, y))` if this component should position the cursor.
    fn cursor_position(&self, _ctx: &RenderContext<'_>) -> Option<(u16, u16)> {
        None
    }

    // === Input Handling ===

    /// Whether this component can receive focus
    ///
    /// Display-only components return `false` (default).
    /// Input-handling components return `true`.
    fn is_focusable(&self) -> bool {
        false
    }

    /// Handle character input
    ///
    /// Called when the component is focused and receives a character.
    /// Default implementation returns `NotHandled`.
    fn handle_insert_char(&mut self, _c: char, _mode: &ModeState) -> InputResult {
        InputResult::NotHandled
    }

    /// Handle backspace/delete backward
    ///
    /// Called when the component is focused and receives a delete key.
    /// Default implementation returns `NotHandled`.
    fn handle_delete_backward(&mut self, _mode: &ModeState) -> InputResult {
        InputResult::NotHandled
    }

    /// Whether this component captures all input when focused
    ///
    /// When `true`, input events are consumed by this component and
    /// not passed to other handlers.
    fn captures_input(&self) -> bool {
        false
    }

    // === Keybindings ===

    /// Provide keybindings for this component
    ///
    /// Returns a list of `(EditModeKind, KeyBinding)` tuples that define the
    /// keybindings for this component. These are registered automatically
    /// when the component is registered with the plugin system.
    ///
    /// The default implementation returns an empty list.
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn keybindings(&self) -> Vec<(EditModeKind, KeyBinding)> {
    ///     vec![
    ///         (EditModeKind::Normal, KeyBinding {
    ///             keys: "h",
    ///             command: CommandRef::Registered(builtin::CURSOR_LEFT),
    ///             hint: Some("Move left"),
    ///             group: Some("motion"),
    ///         }),
    ///     ]
    /// }
    /// ```
    fn keybindings(&self) -> Vec<(EditModeKind, KeyBinding)> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_id_equality() {
        assert_eq!(ComponentId::EDITOR, ComponentId("editor"));
        assert_ne!(ComponentId::EDITOR, ComponentId("explorer"));
    }

    #[test]
    fn test_component_id_display() {
        assert_eq!(format!("{}", ComponentId::EDITOR), "editor");
        assert_eq!(format!("{}", ComponentId::STATUS_LINE), "status_line");
    }

    #[test]
    fn test_component_id_default() {
        assert_eq!(ComponentId::default(), ComponentId::EDITOR);
    }
}
