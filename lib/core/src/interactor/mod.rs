//! UI component interactor system
//!
//! This module provides input-handling UI components that implement `UIComponent`.
//! The `InteractorRegistry` uses `UIComponent` trait for all registered components.
//!
//! Conceptual model:
//! ```text
//! User <-> Window <- UIComponent -> Buffer
//!          (view)    (input handler)  (data)
//! ```

use std::collections::HashMap;

use crate::{
    component::RenderContext,
    event::InnerEvent,
    frame::FrameBuffer,
    modd::ModeState,
    screen::{LayerBounds, z_order},
    ui_component::{ComponentId, UIComponent},
};

// Re-export ComponentId for backward compatibility
// (InteractorId is now an alias for ComponentId)
pub use crate::ui_component::ComponentId as InteractorId;

/// Result of handling input in a component
pub enum InputResult {
    /// Input was not handled
    NotHandled,
    /// Input was handled, request render
    Handled,
    /// Send this event to runtime (enlist pattern)
    SendEvent(InnerEvent),
}

/// Registry of UI component interactors
///
/// Manages all registered UI components and tracks the currently active one.
/// Uses `UIComponent` trait instead of the deprecated `Interactor` trait.
#[derive(Debug)]
pub struct InteractorRegistry {
    targets: HashMap<ComponentId, Box<dyn UIComponent>>,
    active: ComponentId,
}

impl Default for InteractorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InteractorRegistry {
    /// Create a new empty interactor registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            targets: HashMap::new(),
            active: ComponentId::EDITOR,
        }
    }

    /// Register a UI component
    ///
    /// If a component with the same ID already exists, it will be replaced.
    pub fn register(&mut self, target: Box<dyn UIComponent>) {
        let id = target.id();
        self.targets.insert(id, target);
    }

    /// Set the active component
    ///
    /// Returns `true` if the focus was changed, `false` if the component ID is not registered.
    pub fn set_active(&mut self, id: ComponentId) -> bool {
        if self.targets.contains_key(&id) {
            self.active = id;
            true
        } else {
            false
        }
    }

    /// Get the active component
    ///
    /// # Panics
    ///
    /// Panics if the active component is not registered (should never happen in normal use).
    #[must_use]
    pub fn active(&self) -> &dyn UIComponent {
        self.targets
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
        self.targets
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
        self.targets.get(&id).map(AsRef::as_ref)
    }

    /// Get a component by ID mutably
    pub fn get_mut(&mut self, id: ComponentId) -> Option<&mut Box<dyn UIComponent>> {
        self.targets.get_mut(&id)
    }

    /// Check if a component is registered
    #[must_use]
    pub fn contains(&self, id: ComponentId) -> bool {
        self.targets.contains_key(&id)
    }

    /// Get the number of registered components
    #[must_use]
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Check if the registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

// Built-in UI components

/// Editor component (main text editing area)
#[derive(Debug, Default)]
pub struct Editor {
    /// Clear landing page on next input
    pub clear_landing_on_input: bool,
}

impl UIComponent for Editor {
    fn id(&self) -> ComponentId {
        ComponentId::EDITOR
    }

    fn display_name(&self) -> &'static str {
        "EDITOR"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("")
    }

    fn z_order(&self) -> u8 {
        z_order::EDITOR
    }

    fn is_visible(&self, _ctx: &RenderContext<'_>) -> bool {
        true // Editor is always visible
    }

    fn bounds(&self, ctx: &RenderContext<'_>) -> LayerBounds {
        // Editor takes remaining space after tab line and before status line
        LayerBounds {
            x: 0,
            y: ctx.tab_line_offset,
            width: ctx.screen_width,
            height: ctx.screen_height.saturating_sub(ctx.tab_line_offset + 1),
        }
    }

    fn render_to_frame(&self, _buffer: &mut FrameBuffer, _ctx: &RenderContext<'_>) {
        // Rendering is delegated to EditorLayer which has access to Runtime state.
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn handle_insert_char(&mut self, c: char, mode_state: &ModeState) -> InputResult {
        if mode_state.is_command() || mode_state.is_insert() {
            let clear = std::mem::take(&mut self.clear_landing_on_input);
            InputResult::SendEvent(InnerEvent::FocusInput {
                char: Some(c),
                delete: false,
                clear_landing: clear,
            })
        } else {
            InputResult::NotHandled
        }
    }

    fn handle_delete_backward(&mut self, mode_state: &ModeState) -> InputResult {
        if mode_state.is_command() || mode_state.is_insert() {
            InputResult::SendEvent(InnerEvent::FocusInput {
                char: None,
                delete: true,
                clear_landing: false,
            })
        } else {
            InputResult::NotHandled
        }
    }

    fn captures_input(&self) -> bool {
        true
    }
}

/// Explorer component (file browser sidebar)
///
/// Handles filename input for create/rename operations.
#[derive(Debug, Default)]
pub struct Explorer {
    /// Current input buffer for filename operations
    pub input_buffer: String,
    /// Whether input mode is active
    pub input_active: bool,
}

impl UIComponent for Explorer {
    fn id(&self) -> ComponentId {
        ComponentId::EXPLORER
    }

    fn display_name(&self) -> &'static str {
        "EXPLORER"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("")
    }

    fn z_order(&self) -> u8 {
        z_order::EXPLORER
    }

    fn is_visible(&self, _ctx: &RenderContext<'_>) -> bool {
        // Visibility controlled by layout manager
        true
    }

    fn bounds(&self, ctx: &RenderContext<'_>) -> LayerBounds {
        // Explorer sidebar bounds - actual size controlled by layout manager
        LayerBounds {
            x: 0,
            y: ctx.tab_line_offset,
            width: 30, // Default width, actual controlled by layout
            height: ctx.screen_height.saturating_sub(ctx.tab_line_offset + 1),
        }
    }

    fn render_to_frame(&self, _buffer: &mut FrameBuffer, _ctx: &RenderContext<'_>) {
        // Rendering is delegated to ExplorerLayer which has access to Runtime state.
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn handle_insert_char(&mut self, c: char, _mode_state: &ModeState) -> InputResult {
        if self.input_active {
            self.input_buffer.push(c);
            InputResult::Handled
        } else {
            InputResult::NotHandled
        }
    }

    fn handle_delete_backward(&mut self, _mode_state: &ModeState) -> InputResult {
        if self.input_active {
            self.input_buffer.pop();
            InputResult::Handled
        } else {
            InputResult::NotHandled
        }
    }

    fn captures_input(&self) -> bool {
        self.input_active
    }
}

/// Telescope component (fuzzy finder)
///
/// Marker struct - actual state is in `TelescopeState` on Runtime.
#[derive(Debug, Default)]
pub struct Telescope;

impl UIComponent for Telescope {
    fn id(&self) -> ComponentId {
        ComponentId::TELESCOPE
    }

    fn display_name(&self) -> &'static str {
        "TELESCOPE"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("")
    }

    fn z_order(&self) -> u8 {
        z_order::TELESCOPE
    }

    fn is_visible(&self, _ctx: &RenderContext<'_>) -> bool {
        // Visibility controlled by TelescopeState
        true
    }

    fn bounds(&self, ctx: &RenderContext<'_>) -> LayerBounds {
        // Telescope is a modal overlay - full screen
        LayerBounds::full_screen(ctx.screen_width, ctx.screen_height)
    }

    fn render_to_frame(&self, _buffer: &mut FrameBuffer, _ctx: &RenderContext<'_>) {
        // Rendering is delegated to TelescopeLayer which has access to Runtime state.
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn handle_insert_char(&mut self, c: char, _mode_state: &ModeState) -> InputResult {
        InputResult::SendEvent(InnerEvent::FocusInput {
            char: Some(c),
            delete: false,
            clear_landing: false,
        })
    }

    fn handle_delete_backward(&mut self, _mode_state: &ModeState) -> InputResult {
        InputResult::SendEvent(InnerEvent::FocusInput {
            char: None,
            delete: true,
            clear_landing: false,
        })
    }

    fn captures_input(&self) -> bool {
        true // Telescope captures all input when active
    }
}

/// Settings menu component
#[derive(Debug, Default)]
pub struct Settings;

impl UIComponent for Settings {
    fn id(&self) -> ComponentId {
        ComponentId::SETTINGS
    }

    fn display_name(&self) -> &'static str {
        "SETTINGS"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("")
    }

    fn z_order(&self) -> u8 {
        z_order::SETTINGS_MENU
    }

    fn is_visible(&self, _ctx: &RenderContext<'_>) -> bool {
        // Visibility controlled by SettingsMenuState
        true
    }

    fn bounds(&self, ctx: &RenderContext<'_>) -> LayerBounds {
        // Settings is a modal overlay - full screen
        LayerBounds::full_screen(ctx.screen_width, ctx.screen_height)
    }

    fn render_to_frame(&self, _buffer: &mut FrameBuffer, _ctx: &RenderContext<'_>) {
        // Rendering is delegated to SettingsMenuLayer which has access to Runtime state.
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn captures_input(&self) -> bool {
        true // Settings menu captures all input when active
    }
}

/// Command line component (`:` command input)
///
/// Handles input for command-line mode. State is managed via the enlist pattern,
/// with the actual `CommandLine` struct on Runtime.
#[derive(Debug, Default)]
pub struct CommandLineInt;

impl UIComponent for CommandLineInt {
    fn id(&self) -> ComponentId {
        ComponentId::COMMAND_LINE
    }

    fn display_name(&self) -> &'static str {
        "COMMAND"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("󰘳 ")
    }

    fn z_order(&self) -> u8 {
        z_order::BASE // Command line is at bottom, same level as status line
    }

    fn is_visible(&self, _ctx: &RenderContext<'_>) -> bool {
        // Visibility controlled by mode state (command mode)
        true
    }

    fn bounds(&self, ctx: &RenderContext<'_>) -> LayerBounds {
        // Command line is at the bottom of the screen (same as status line)
        LayerBounds {
            x: 0,
            y: ctx.screen_height.saturating_sub(1),
            width: ctx.screen_width,
            height: 1,
        }
    }

    fn render_to_frame(&self, _buffer: &mut FrameBuffer, _ctx: &RenderContext<'_>) {
        // Rendering is handled by Screen::render_command_line_to_buffer
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn handle_insert_char(&mut self, c: char, _mode_state: &ModeState) -> InputResult {
        InputResult::SendEvent(InnerEvent::FocusInput {
            char: Some(c),
            delete: false,
            clear_landing: false,
        })
    }

    fn handle_delete_backward(&mut self, _mode_state: &ModeState) -> InputResult {
        InputResult::SendEvent(InnerEvent::FocusInput {
            char: None,
            delete: true,
            clear_landing: false,
        })
    }

    fn captures_input(&self) -> bool {
        true // Command line captures all input when active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interactor_id_equality() {
        assert_eq!(ComponentId::EDITOR, ComponentId("editor"));
        assert_ne!(ComponentId::EDITOR, ComponentId::EXPLORER);
    }

    #[test]
    fn test_interactor_registry_register() {
        let mut registry = InteractorRegistry::new();
        assert!(registry.is_empty());

        registry.register(Box::new(Editor::default()));
        assert_eq!(registry.len(), 1);
        assert!(registry.contains(ComponentId::EDITOR));
    }

    #[test]
    fn test_interactor_registry_active() {
        let mut registry = InteractorRegistry::new();
        registry.register(Box::new(Editor::default()));
        registry.register(Box::new(Telescope));

        assert_eq!(registry.active_id(), ComponentId::EDITOR);

        assert!(registry.set_active(ComponentId::TELESCOPE));
        assert_eq!(registry.active_id(), ComponentId::TELESCOPE);

        // Can't set to unregistered interactor
        assert!(!registry.set_active(ComponentId::EXPLORER));
        assert_eq!(registry.active_id(), ComponentId::TELESCOPE);
    }
}
