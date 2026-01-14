//! Display driver for reovim.
//!
//! Linux equivalent: `drivers/video/`, `drivers/gpu/`
//!
//! # Architecture
//!
//! This crate defines trait interfaces for display operations.
//! Style types (`Style`, `Attributes`, `ColorMode`) are owned by this crate.
//! `Color` comes from `reovim-arch` (platform abstraction layer).
//!
//! ```text
//! lib/drivers/display/      <-- Traits + Style types (this crate)
//!        ^
//!        |  uses Color from
//!        |
//! lib/arch/                 <-- Platform-agnostic Color type
//! ```
//!
//! # Components
//!
//! - [`DisplayDriver`] - Terminal lifecycle and rendering
//! - [`WindowManager`] - Window splits, tabs, layout
//! - [`Compositor`] - Z-order layer management
//! - [`DisplayCapabilities`] - Terminal feature detection
//! - [`RenderCommand`] - Render operation commands
//! - [`Style`], [`Attributes`], [`ColorMode`] - Text styling

// ============================================================================
// Modules
// ============================================================================

mod border;
pub mod builder;
mod capabilities;
mod command;
mod compositor;
pub mod decoration;
mod error;
mod frame;
mod highlight;
mod mode;
mod policy;
mod render;
mod screen;
pub mod style;
mod traits;
pub mod ui;
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

// Mode display (Phase 6 - Kernel-driver architecture)
pub use mode::{CursorStyle, ModeDisplay};

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
// Style and Color Types (owned by this crate)
// ============================================================================
//
// Style types are defined locally in highlight.rs.
// Color comes from reovim-arch (platform abstraction layer).
// ============================================================================

pub use {
    highlight::{Attributes, ColorMode, Style, downgrade_color},
    reovim_arch::Color,
};

// ============================================================================
// Display Builder (Phase 5.11 - Component display registration)
// ============================================================================

pub use builder::{ComponentId, DisplayInfo, DisplayInfoBuilder, DisplayRegistry};

// ============================================================================
// Decoration System (Phase 5.11 - Concealment, highlighting, selection)
// ============================================================================

pub use decoration::{
    BufferDecorations, ConcealedLine, Decoration, DecorationGroup, DecorationProvider,
    DecorationProviderFactory, DecorationRef, DecorationStore, Span, apply_conceals,
    display_to_source_col, source_to_display_col,
};

// ============================================================================
// Style System (Phase 5.11 - Themes and Icons)
// ============================================================================

pub use style::{
    BuiltinFileIconProvider, BuiltinTheme, IconDef, IconProvider, IconRegistry, IconSet,
    ThemeManager, ThemeProvider,
};

// ============================================================================
// UI Primitives (Phase 5.11 - Unicode-aware text utilities)
// ============================================================================

pub use ui::{
    Alignment, align, display_width, pad_left, pad_right, truncate_end, truncate_start, wrap_text,
};
