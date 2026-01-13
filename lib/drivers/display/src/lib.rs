//! Display driver for reovim.
//!
//! Linux equivalent: `drivers/video/`, `drivers/gpu/`
//!
//! # Architecture
//!
//! This crate defines trait interfaces for display operations.
//! Types are re-exported from `reovim-core` during the staged migration.
//!
//! ```text
//! lib/drivers/display/      <-- Traits (this crate)
//!        ^
//!        |  re-exports types from (Phase 3.3)
//!        |
//! lib/core/                 <-- Cell, Style, FrameBuffer, ZGroup, ZOrder
//! ```
//!
//! # Migration Path
//!
//! - **Phase 3.3 (current)**: Re-export types from `reovim-core`
//! - **Phase 5-6 (future)**: Types move to this crate, `lib/core` imports from here
//!
//! # Components
//!
//! - [`DisplayDriver`] - Terminal lifecycle and rendering
//! - [`WindowManager`] - Window splits, tabs, layout
//! - [`Compositor`] - Z-order layer management
//! - [`DisplayCapabilities`] - Terminal feature detection
//! - [`RenderCommand`] - Render operation commands

// ============================================================================
// Modules
// ============================================================================

mod border;
mod capabilities;
mod command;
mod compositor;
mod error;
mod frame;
mod policy;
mod render;
mod screen;
mod traits;
mod window;
mod window_renderer;

// ============================================================================
// Re-exports
// ============================================================================

// Error types
pub use error::DisplayError;

// Capability detection
pub use capabilities::DisplayCapabilities;

// Render commands
pub use command::RenderCommand;

// Window types
pub use window::{NavigateDirection, Rect, SplitDirection, TerminalSize, WindowId};

// Traits
pub use traits::{DisplayDriver, WindowManager};

// Compositor (local implementation)
pub use compositor::{
    Bounds, Composable, ComposableId, Compositor, CompositorEntry, LayerCompositor, ZGroup, ZOrder,
};

// Frame types (Phase 5 - NEW local implementations)
pub use frame::{Cell, FrameBuffer, FrameBufferHandle, FrameRenderer, char_width};

// Policy traits (Phase 5 - Mechanism vs Policy separation)
pub use policy::{DefaultFocusPolicy, FocusPolicy, LayoutPolicy, SingleWindowLayout, WindowView};

// Screen management (Phase 5 - Terminal surface)
pub use screen::Screen;

// Window rendering (Phase 5 - Window content to cells)
pub use window_renderer::{LineNumberMode, RenderContent, WindowRenderer, WindowRendererConfig};

// Border rendering (Phase 5 - Window decoration)
pub use border::{
    BorderChars, BorderMode, BorderStyle, Corner, WindowAdjacency, inner_bounds, render_border,
    render_border_simple,
};

// Render pipeline (Phase 6 - Composable rendering)
pub use render::{
    GutterDecoration, InlineDecoration, RenderContext, RenderData, RenderStage, SeparatorChars,
    TabInfo, execute_pipeline, execute_pipeline_sorted, render_grid_separators, render_hseparator,
    render_intersection, render_line, render_line_simple, render_statusline,
    render_statusline_simple, render_tabline, render_vseparator,
};

// ============================================================================
// Style and Color Types (temporary re-exports from reovim-core)
// ============================================================================
//
// TODO(Phase 5-6): After type migration completes:
//   1. Move Style, Attributes, Color, ColorMode definitions to this crate
//   2. Update lib/core to import FROM this driver
//   3. Remove reovim-core dependency from this crate's Cargo.toml
// ============================================================================

pub use reovim_core::highlight::{Attributes, Color, ColorMode, Style};
