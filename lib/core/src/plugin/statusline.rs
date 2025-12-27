//! Statusline section provider trait
//!
//! This module provides a generic trait for plugins that want to contribute
//! sections to the status line. This is a plugin-agnostic API - core doesn't
//! know about specific plugins, only that some provider might exist.
//!
//! # Example
//!
//! ```ignore
//! use reovim_core::plugin::{
//!     StatuslineSectionProvider, StatuslineRenderContext, RenderedSection, SectionAlignment
//! };
//!
//! struct MyStatuslineProvider;
//!
//! impl StatuslineSectionProvider for MyStatuslineProvider {
//!     fn render_sections(&self, ctx: &StatuslineRenderContext) -> Vec<RenderedSection> {
//!         vec![RenderedSection {
//!             text: " LSP ".to_string(),
//!             style: None,
//!             alignment: SectionAlignment::Right,
//!             priority: 100,
//!         }]
//!     }
//! }
//! ```

use std::sync::Arc;

use crate::highlight::Style;

/// Alignment for statusline sections
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum SectionAlignment {
    /// Render after built-in left sections (mode, filename)
    #[default]
    Left,
    /// Render in the middle (between left and right sections)
    Center,
    /// Render before built-in right sections (position, pending keys)
    Right,
}

/// Context passed to statusline section providers during rendering
#[derive(Debug)]
pub struct StatuslineRenderContext<'a> {
    /// Plugin state registry for accessing plugin state
    pub plugin_state: &'a super::PluginStateRegistry,
    /// Width of the status line in characters
    pub screen_width: u16,
    /// Current row of the status line
    pub status_row: u16,
}

/// A rendered section ready for display
#[derive(Debug, Clone)]
pub struct RenderedSection {
    /// Text content of the section
    pub text: String,
    /// Optional style override (uses statusline background if None)
    pub style: Option<Style>,
    /// Where to align this section
    pub alignment: SectionAlignment,
    /// Priority within alignment group (lower = further left/first)
    pub priority: u32,
}

/// Trait for plugins that provide statusline sections
///
/// Implement this trait to contribute custom sections to the status line.
/// The provider is called during each render cycle to get current sections.
pub trait StatuslineSectionProvider: Send + Sync + 'static {
    /// Render all sections provided by this provider
    ///
    /// Returns a vector of rendered sections. Sections are sorted by
    /// alignment and priority before rendering.
    fn render_sections(&self, ctx: &StatuslineRenderContext) -> Vec<RenderedSection>;
}

/// Shared statusline section provider type alias
pub type SharedStatuslineSectionProvider = Arc<dyn StatuslineSectionProvider>;
