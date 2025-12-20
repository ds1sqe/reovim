//! Render context for UI components
//!
//! This module provides the `RenderContext` which is passed to UI components
//! during rendering. It contains screen dimensions, theme, and color mode info.
//!
//! Also provides `RenderState` which bundles all runtime state needed for rendering.

use std::collections::BTreeMap;

use crate::{
    buffer::Buffer,
    command_line::CommandLine,
    completion::CompletionState,
    explorer::ExplorerState,
    folding::FoldManager,
    highlight::{ColorMode, HighlightStore, Theme},
    indent::IndentAnalyzer,
    leap::LeapState,
    modd::ModeState,
    modifier::{ModifierRegistry, StyleModifiers, WindowBehaviorState, WindowStyleState},
    settings_menu::SettingsMenuState,
    telescope::TelescopeState,
};

/// Runtime state bundled for rendering
///
/// This struct bundles all the runtime state needed by components during rendering,
/// reducing the number of parameters passed through the rendering pipeline.
#[derive(Debug)]
pub struct RenderState<'a> {
    /// All open buffers
    pub buffers: &'a BTreeMap<usize, Buffer>,
    /// Syntax highlight store
    pub highlight_store: &'a HighlightStore,
    /// Current mode state
    pub mode: &'a ModeState,
    /// Command line state
    pub command_line: &'a CommandLine,
    /// Pending key sequence for display
    pub pending_keys: &'a str,
    /// Last executed command for display
    pub last_command: &'a str,
    /// Color mode (ANSI, 256, `TrueColor`)
    pub color_mode: ColorMode,
    /// Current theme
    pub theme: &'a Theme,
    /// Explorer state (if explorer is enabled)
    pub explorer: Option<&'a ExplorerState>,
    /// Completion popup state
    pub completion: &'a CompletionState,
    /// Telescope fuzzy finder state
    pub telescope: &'a TelescopeState,
    /// Leap motion state
    pub leap: &'a LeapState,
    /// Fold manager for code folding
    pub fold_manager: &'a FoldManager,
    /// Indent analyzer for guide rendering
    pub indent_analyzer: &'a IndentAnalyzer,
    /// Settings menu state
    pub settings_menu: &'a SettingsMenuState,
    /// Modifier registry for evaluating style/behavior modifiers
    pub modifier_registry: Option<&'a ModifierRegistry>,
}

/// Context passed to UI components during rendering
///
/// Contains screen dimensions, theme, color mode, and optionally the full runtime state.
/// The state field is optional for backward compatibility - components that don't need
/// full state access can work with just the basic context.
#[derive(Debug, Clone, Copy)]
pub struct RenderContext<'a> {
    /// Screen width in columns
    pub screen_width: u16,
    /// Screen height in rows
    pub screen_height: u16,
    /// Current theme
    pub theme: &'a Theme,
    /// Color mode (ANSI, 256, `TrueColor`)
    pub color_mode: ColorMode,
    /// Y offset for tab line (0 if no tabs, 1 if tabs visible)
    pub tab_line_offset: u16,
    /// Optional runtime state for components that need full state access
    pub state: Option<&'a RenderState<'a>>,
    /// Evaluated style modifiers for this rendering context
    pub modifier_style: Option<&'a WindowStyleState>,
    /// Evaluated behavior modifiers for this rendering context
    pub modifier_behavior: Option<&'a WindowBehaviorState>,
}

impl<'a> RenderContext<'a> {
    /// Create a new render context without runtime state
    #[must_use]
    pub const fn new(
        screen_width: u16,
        screen_height: u16,
        theme: &'a Theme,
        color_mode: ColorMode,
    ) -> Self {
        Self {
            screen_width,
            screen_height,
            theme,
            color_mode,
            tab_line_offset: 0,
            state: None,
            modifier_style: None,
            modifier_behavior: None,
        }
    }

    /// Create a render context with runtime state
    #[must_use]
    pub const fn with_state(
        screen_width: u16,
        screen_height: u16,
        theme: &'a Theme,
        color_mode: ColorMode,
        state: &'a RenderState<'a>,
    ) -> Self {
        Self {
            screen_width,
            screen_height,
            theme,
            color_mode,
            tab_line_offset: 0,
            state: Some(state),
            modifier_style: None,
            modifier_behavior: None,
        }
    }

    /// Set the tab line offset
    #[must_use]
    pub const fn with_tab_offset(mut self, offset: u16) -> Self {
        self.tab_line_offset = offset;
        self
    }

    /// Set the evaluated style modifiers
    #[must_use]
    pub const fn with_modifier_style(mut self, style: &'a WindowStyleState) -> Self {
        self.modifier_style = Some(style);
        self
    }

    /// Set the evaluated behavior modifiers
    #[must_use]
    pub const fn with_modifier_behavior(mut self, behavior: &'a WindowBehaviorState) -> Self {
        self.modifier_behavior = Some(behavior);
        self
    }

    /// Get the status line row (bottom of screen)
    #[must_use]
    pub const fn status_line_row(&self) -> u16 {
        self.screen_height.saturating_sub(1)
    }

    /// Get the runtime state
    ///
    /// Returns `None` if the context was created without state.
    #[must_use]
    pub const fn state(&self) -> Option<&RenderState<'a>> {
        self.state
    }

    /// Get style overrides from evaluated modifiers
    ///
    /// Returns `None` if no modifiers were evaluated.
    #[must_use]
    pub fn style_overrides(&self) -> Option<&StyleModifiers> {
        self.modifier_style.map(|s| &s.style)
    }

    /// Get behavior flags from evaluated modifiers
    ///
    /// Returns `None` if no modifiers were evaluated.
    #[must_use]
    pub const fn behavior(&self) -> Option<&WindowBehaviorState> {
        self.modifier_behavior
    }
}
