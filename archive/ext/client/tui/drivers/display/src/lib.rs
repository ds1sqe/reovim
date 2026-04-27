#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
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
//! ext/client/tui/drivers/display/  <-- Traits + Style types (this crate)
//!        ^
//!        |  uses Color from
//!        |
//! arch/                            <-- Platform-agnostic Color type
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

pub mod annotation;
mod border;
pub mod builder;
mod capabilities;
pub mod color_blend;
mod command;
mod compositor;
pub mod decoration;
mod error;
mod frame;
mod highlight;
pub mod layout;
mod mode;
pub mod overlay_content;
mod policy;
pub mod popup_utils;
mod render;
pub mod render_backend;
mod screen;
pub mod statusline;
pub mod style;
pub mod syntax;
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

// Window and buffer types (IDs re-exported from kernel as SSOT)
// Geometry types (Rect, Size) come from common client model via window module
pub use {
    reovim_kernel::api::v1::{BufferId, WindowId},
    window::{Direction, Rect, Size, SplitDirection, TerminalSize, TerminalSizeExt},
};

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
    TabInfo, TablineStyles, execute_pipeline, execute_pipeline_sorted, render_grid_separators,
    render_hseparator, render_intersection, render_line, render_line_simple, render_statusline,
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
    BufferDecorationSource, BufferDecorationSourceRegistry, BufferDecorations, ConcealedLine,
    Decoration, DecorationGroup, DecorationProvider, DecorationProviderFactory,
    DecorationProviderKey, DecorationProviderRegistry, DecorationRef, DecorationSourceKey,
    DecorationStore, Span, apply_conceals, display_to_source_col, source_to_display_col,
};

// ============================================================================
// Style System (Phase 5.11 - Themes and Icons)
// ============================================================================

pub use style::{
    BuiltinFileIconProvider, BuiltinTheme, IconDef, IconProvider, IconRegistry, IconSet,
    StyleGroupRegistry, ThemeLoader, ThemeManager, ThemeProvider,
};

// ============================================================================
// UI Primitives (Phase 5.11 - Unicode-aware text utilities)
// ============================================================================

pub use ui::{
    Alignment, align, display_width, pad_left, pad_right, truncate_end, truncate_start, wrap_text,
};

// ============================================================================
// Annotation System (Issue #455 - Generic Line Annotation System)
// ============================================================================
//
// Generic annotation system for displaying per-line information in the gutter.
// Decouples data sources from visual presentation.
// ============================================================================

pub use annotation::{
    Annotation, AnnotationContext, AnnotationKind, AnnotationLayer, AnnotationPayload,
    AnnotationPresenter, AnnotationSource, AnnotationSourceKey, AnnotationSourceRegistry,
    AnnotationStore, AnnotationTarget, BlamePresenter, BufferAnnotationStore, ColumnConfig,
    ColumnWidth, ComposedLine, ComposerBuilder, DiagnosticPresenter, GitSignsPresenter, GutterCell,
    GutterComposer, GutterConfig, GutterRenderer, GutterRendererKey, GutterRendererRegistry,
    KindPattern, LineNumberPresenter, PresentedOutput, PresenterContext, PresenterRegistry,
    SourceId, VisibilityMode,
};

// ============================================================================
// Window Layout Subsystem (Epic #403 - Nested Compositor Architecture)
// ============================================================================
//
// Hyprland-inspired window management with nested layers.
// Each layer is a self-contained compositor with Tiled, Float, and Overlay zones.
// ============================================================================

pub use color_blend::{dim_style, lerp_color};

pub use layout::{
    // Layer types
    Anchor,
    CLICK_THROUGH_THRESHOLD,
    // Compositor traits
    CompositeResult,
    // Zone traits
    FloatingLayer,
    FloatingWindow,
    Layer,
    LayerConfig,
    LayerId,
    MIN_WINDOW_HEIGHT,
    MIN_WINDOW_WIDTH,
    OverlayConstraints,
    OverlayLayer,
    OverlayWindow,
    RootCompositor,
    TiledLayer,
    WindowError,
    WindowLayerCompositor,
    WindowPlacement,
    Zone,
};

// ============================================================================
// Statusline System (Issue #441 - Extensible Statusline)
// ============================================================================
//
// Lualine-inspired statusline with sections (A-B-C | X-Y-Z),
// pluggable components, and mode-specific theming.
// ============================================================================

pub use statusline::{
    // Component API
    ComponentContext,
    // Display-agnostic data provider types (from statusline driver)
    ComponentData,
    ComponentDataContext,
    ComponentDataProvider,
    ComponentDataProviderKey,
    ComponentDataProviderRegistry,
    ComponentOutput,
    ComponentProvider,
    ComponentProviderKey,
    ComponentProviderRegistry,
    // Height configuration
    ContentMetrics,
    DataProviderAdapter,
    DiagnosticCounts,
    HeightConfig,
    HeightResult,
    // Multi-row layout
    LayoutCalculator,
    MultiRowLayout,
    OverflowStrategy,
    RowContent,
    ScreenThreshold,
    // Section types
    Section,
    SectionId,
    SectionPosition,
    // Provider traits
    StatuslineProvider,
    StatuslineProviderKey,
    StatuslineProviderRegistry,
    // Renderer
    StatuslineRendererConfig,
    StatuslineSeparator,
    calculate_height,
    calculate_height_from_sections,
    calculate_height_with_metrics,
    render_sections,
    truncate_sections,
};

// ============================================================================
// Overlay Content System (Issue #457 - Which-Key Integration)
// ============================================================================
//
// Mechanism for overlay windows to display custom content (popups, menus).
// Modules register content providers via ServiceRegistry.
// ============================================================================

pub use overlay_content::{
    OverlayContentKey, OverlayContentProvider, OverlayContentRegistry, OverlayContentStorage,
};

// ============================================================================
// Syntax Token Cache (Phase 13.0 #470 - StreamTokens Integration)
// ============================================================================
//
// Client-side caching of syntax tokens from StreamTokens RPC.
// Converts byte offsets to positions and provides efficient line queries.
// ============================================================================

pub use syntax::{AnnotationCacheManager, CachedToken, LayeredTokenCache, TokenSpan};
