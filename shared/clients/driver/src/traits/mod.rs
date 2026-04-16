use std::sync::Arc;

use crate::{
    AnnotationContext, BufferId, BufferUpdateEvent, ChromePosition, ClientModuleError, ColumnWidth,
    GutterCell, Insets, OptionValue, ProbeResult, Rect, RenderBehavior, RenderingModel, Style,
    SyntaxToken, TransformedLine, Version, ViewportContext, VirtualLine, WindowId, WindowLayout,
    projection::DomainProjection,
    types::{Color, ColorDepth, InlineDecoration},
};

use reovim_subsys_coordination::DomainId;

// =============================================================================
// LocalInputResult (#753 F4)
// =============================================================================

/// Result of local input handling by a domain view module.
///
/// Returned by [`ClientModule::on_local_input`] to indicate how the input
/// was processed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalInputResult {
    /// The input was consumed locally (e.g., scrolling, selection).
    /// Do not send to the server.
    Consumed,
    /// The input was not handled. Send it to the server as usual.
    Unhandled,
    /// The input should be sent to the server (module transformed it).
    SendToServer,
}

// =============================================================================
// ClientModule — the single trait CORE interacts with
// =============================================================================

/// The central trait for all client-side modules.
///
/// Every UI feature (statusline, diagnostics, explorer, etc.) implements this
/// trait. CORE dispatches to modules via this interface and has zero knowledge
/// of specific UI features.
///
/// All methods have default no-op implementations so modules only override
/// what they need.
#[allow(unused_variables)]
pub trait ClientModule: Send + Sync + 'static {
    // ---- Identity ----

    /// Unique identifier for this module instance.
    fn id(&self) -> &'static str;

    /// Module kind (used for server-side extension matching).
    fn kind(&self) -> &'static str {
        self.id()
    }

    /// Human-readable display name.
    fn name(&self) -> &'static str;

    /// Module version.
    fn version(&self) -> Version;

    /// Required module dependencies (by kind).
    fn dependencies(&self) -> &[&str] {
        &[]
    }

    /// Optional module dependencies (by kind).
    fn optional_dependencies(&self) -> &[&str] {
        &[]
    }

    /// Server extension kinds this module handles notifications for.
    fn server_kinds(&self) -> Vec<&'static str> {
        vec![self.kind()]
    }

    // ---- Lifecycle ----

    /// Initialize the module. Called once after construction.
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult;

    /// Clean up resources. Called before the module is dropped.
    ///
    /// # Errors
    ///
    /// Returns `ClientModuleError` if cleanup fails.
    fn exit(&mut self) -> Result<(), ClientModuleError>;

    /// Called after all modules have been loaded.
    fn on_all_loaded(&mut self, ctx: &ModuleContext) {}

    // ---- Role declaration (checked every frame) ----

    /// Whether this module contributes chrome (statusline, sidebar, overlay).
    fn has_chrome(&self) -> bool {
        false
    }

    /// Whether this module contributes to buffer rendering.
    fn has_buffer_contrib(&self) -> bool {
        false
    }

    /// Whether this module contributes gutter annotations.
    fn has_annotations(&self) -> bool {
        false
    }

    // ---- Events ----

    /// Handle a server notification (JSON data).
    fn on_notification(&mut self, data: &str) {}

    /// Handle an option value change.
    fn on_option_changed(&mut self, name: &str, value: &OptionValue) {}

    /// Handle a buffer content update.
    fn on_buffer_update(&mut self, event: &BufferUpdateEvent) {}

    /// Handle a cursor position change.
    fn on_cursor_update(&mut self, buffer_id: BufferId, line: usize, col: usize) {}

    /// Handle focus change to a different buffer.
    fn on_buffer_focus(&mut self, buffer_id: BufferId) {}

    /// Handle a mode change (e.g., "normal", "insert", "visual").
    fn on_mode_change(&mut self, mode: &str) {}

    /// Handle platform capabilities change.
    fn on_capabilities_changed(&mut self, caps: &dyn PlatformCapabilities) {}

    /// Handle theme change.
    fn on_theme_changed(&mut self, theme: &dyn ThemeProvider) {}

    /// Periodic tick. Returns `true` if state changed and a redraw is needed.
    fn tick(&mut self) -> bool {
        false
    }

    // ---- Domain view (#753 F4) ----

    /// The domain this module belongs to, if any.
    ///
    /// Domain view modules return `Some(domain_id)` to receive projections
    /// from that domain. Chrome-only modules return `None` (default).
    fn domain_id(&self) -> Option<DomainId> {
        None
    }

    /// Handle a domain projection update.
    ///
    /// Called when a projection matching this module's `domain_id()` arrives.
    /// Modules update their internal state from the projection's content bytes.
    /// Transient projections are also delivered (check `projection.transient`).
    fn on_projection(&mut self, projection: &DomainProjection) {}

    /// Render domain content into the given surface region.
    ///
    /// Called by the compositor for domain view modules. The module decodes
    /// its cached projection data and renders into the surface.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn render_domain_surface(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        caps: &dyn PlatformCapabilities,
    ) {
    }

    /// Whether this module wants to handle input locally before sending to server.
    ///
    /// When `true`, the compositor calls `on_local_input()` first. The module
    /// can consume the input (e.g., scrolling) or pass it through to the server.
    fn wants_local_input(&self) -> bool {
        false
    }

    /// Handle local input before it reaches the server.
    ///
    /// Only called when `wants_local_input()` returns `true`.
    /// Returns a `LocalInputResult` indicating how the input was handled.
    fn on_local_input(&mut self, input: &[u8]) -> LocalInputResult {
        LocalInputResult::Unhandled
    }

    // ---- Chrome ----

    /// Where this module's chrome is rendered.
    fn chrome_position(&self) -> ChromePosition {
        ChromePosition::Bottom
    }

    /// Requested size (height for Top/Bottom, width for Left/Right).
    fn chrome_requested_size(&self, caps: &dyn PlatformCapabilities) -> u16 {
        1
    }

    /// Priority for chrome allocation (higher = allocated first).
    fn chrome_priority(&self) -> u16 {
        0
    }

    /// Z-order for overlay chrome (higher = drawn on top).
    fn chrome_z_order(&self) -> u16 {
        0
    }

    /// Render chrome into the given surface region.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        caps: &dyn PlatformCapabilities,
    ) {
    }

    // ---- Buffer contribution ----

    /// Priority for buffer contribution dispatch (higher = checked first).
    fn buffer_contrib_priority(&self) -> u16 {
        0
    }

    /// Classify a token category for rendering behavior.
    fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
        None
    }

    /// Transform a line before rendering.
    fn transform_line(&self, buf: BufferId, line: usize, text: &str) -> Option<TransformedLine> {
        None
    }

    /// Map a cursor column to a display column.
    fn map_cursor_column(&self, buf: BufferId, line: usize, col: usize) -> Option<u16> {
        None
    }

    /// Return fold ranges (`start_line`, `line_count`) for hidden lines.
    fn fold_ranges(&self) -> &[(usize, usize)] {
        &[]
    }

    /// Return virtual lines to inject into the buffer display.
    fn virtual_lines(&self) -> &[VirtualLine] {
        &[]
    }

    /// Return inline decorations for a given line.
    fn inline_decorations(&self, line: usize) -> &[InlineDecoration] {
        &[]
    }

    /// Override cursor position (for modules that manage their own cursor).
    fn cursor_position(&self, w: u16, h: u16) -> Option<(u16, u16)> {
        None
    }

    // ---- Annotations (gutter) ----

    /// Width of this module's annotation column.
    fn annotation_column_width(
        &self,
        ctx: &AnnotationContext,
        caps: &dyn PlatformCapabilities,
    ) -> ColumnWidth {
        ColumnWidth::Fixed(0)
    }

    /// Render a gutter cell for the given line.
    fn annotate(&self, line: usize, ctx: &AnnotationContext) -> Option<GutterCell> {
        None
    }

    /// Priority for annotation ordering (higher = leftmost in gutter).
    fn annotation_priority(&self) -> u16 {
        0
    }
}

// =============================================================================
// ModuleContext
// =============================================================================

/// Context provided to modules during initialization and lifecycle events.
pub struct ModuleContext<'a> {
    pub capabilities: &'a dyn PlatformCapabilities,
    pub server: Arc<dyn ServerHandle>,
    pub theme: &'a dyn ThemeProvider,
    /// Client-side service registry for cross-module communication.
    ///
    /// `None` for FFI modules compiled against API version < 0.2.0.
    pub services: Option<&'a crate::services::ClientServiceRegistry>,
    /// Module state introspection (query which modules are loaded/running).
    ///
    /// `None` for FFI modules compiled against API version < 0.2.0.
    pub module_registry: Option<&'a dyn ClientModuleRegistry>,
}

// =============================================================================
// ClientModuleRegistry
// =============================================================================

/// Trait for querying client module state (read-only introspection).
///
/// Implemented by the loader (or a snapshot wrapper) and exposed via
/// `ModuleContext.module_registry`. Modules can query whether dependencies
/// are loaded and running.
pub trait ClientModuleRegistry: Send + Sync {
    /// Check if a module with the given kind is currently running.
    fn is_running(&self, kind: &str) -> bool;

    /// Get the state of a module by kind.
    fn module_state(&self, kind: &str) -> Option<crate::loader::ClientModuleState>;

    /// List all loaded module kinds.
    fn loaded_kinds(&self) -> Vec<String>;

    /// Count of currently running modules.
    fn running_count(&self) -> usize;
}

// =============================================================================
// PlatformCapabilities
// =============================================================================

/// Platform capabilities and constraints.
///
/// Queried by modules to adapt their behavior to the current platform
/// (terminal, web, mobile, etc.).
#[allow(unused_variables)]
pub trait PlatformCapabilities: Send + Sync {
    /// The rendering model this platform uses.
    fn rendering_model(&self) -> RenderingModel;

    /// Grid size in cells (columns, rows). `None` if not a grid-based platform.
    fn grid_size(&self) -> Option<(u16, u16)>;

    /// Color depth supported by the display.
    fn color_depth(&self) -> ColorDepth;

    /// Pixel dimensions of the display. `None` if not available.
    fn pixel_size(&self) -> Option<(u32, u32)>;

    /// Whether Unicode width calculations are reliable on this platform.
    fn reliable_unicode_width(&self) -> bool;

    /// Whether the display is in dark mode.
    fn dark_mode(&self) -> bool;

    /// Whether smooth scrolling is supported.
    fn smooth_scroll(&self) -> bool;

    /// Whether pointer (mouse) events are available.
    fn pointer_events(&self) -> bool;

    /// Whether touch input is available.
    fn touch_input(&self) -> bool;

    /// Whether haptic feedback is available.
    fn haptic(&self) -> bool;

    /// Safe area insets (for notch/rounded corners).
    fn safe_area(&self) -> Insets;

    /// Whether the window currently has focus.
    fn has_focus(&self) -> bool;

    /// Whether clipboard access is available.
    fn clipboard_available(&self) -> bool;

    /// Whether a screen reader is active.
    fn screen_reader_active(&self) -> bool;
}

// =============================================================================
// RenderSurface
// =============================================================================

/// Abstraction over the rendering target.
///
/// Provides primitive drawing operations. Platform adapters implement this
/// for their specific rendering backend (cell grid, canvas, native views).
pub trait RenderSurface {
    /// Write styled text at position. Returns the number of columns consumed.
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16;

    /// Apply a style to an existing cell (without changing its content).
    fn apply_style(&mut self, x: u16, y: u16, style: Style);

    /// Override the background color of a cell.
    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color);

    /// Fill a rectangle with a character and style.
    fn fill(&mut self, rect: Rect, ch: char, style: Style);

    /// Clear a rectangle (reset to default).
    fn clear(&mut self, rect: Rect);

    /// Size of the surface in cells (columns, rows).
    fn size(&self) -> (u16, u16);
}

// =============================================================================
// ServerHandle
// =============================================================================

/// Handle for communicating with the server from a client module.
#[allow(unused_variables)]
pub trait ServerHandle: Send + Sync {
    /// Query option values from the server.
    fn get_options(&self, names: &[&str]) -> Vec<(String, OptionValue)>;

    /// Execute a server command.
    fn execute_command(&self, command: &str);

    /// List available command names.
    ///
    /// Default: empty list (for backward compatibility with existing impls).
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn list_commands(&self) -> Vec<String> {
        Vec::new()
    }

    /// Get metadata about a named option.
    ///
    /// Default: `None` (for backward compatibility with existing impls).
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn get_option_metadata(&self, name: &str) -> Option<crate::OptionMetadata> {
        None
    }
}

// =============================================================================
// ThemeProvider
// =============================================================================

/// Provides theme/highlight information to modules.
pub trait ThemeProvider: Send + Sync {
    /// Get the style for a highlight group.
    fn highlight(&self, group: &str) -> Style;

    /// Get the style for the first matching highlight group in the list.
    fn highlight_with_fallback(&self, groups: &[&str]) -> Style;

    /// Default foreground style.
    fn foreground(&self) -> Style;

    /// Default background style.
    fn background(&self) -> Style;

    /// Whether the current theme is dark.
    fn is_dark(&self) -> bool;
}

// =============================================================================
// TokenProvider
// =============================================================================

/// Provides syntax tokens for viewport rendering.
///
/// Abstracts over the annotation/syntax cache. The TUI implements this
/// with `AnnotationCacheManager`, but other platforms can provide tokens
/// from different sources.
pub trait TokenProvider: Send + Sync {
    /// Get syntax tokens for a specific line in a buffer.
    fn tokens_for_line(&self, buffer_id: BufferId, line: u32) -> Vec<SyntaxToken>;
}

// =============================================================================
// ViewportRenderer
// =============================================================================

/// Renders buffer content into a viewport region.
///
/// The default implementation handles gutter, folds, virtual lines, token
/// classification, conceals, selections, and cursor rendering. Custom
/// implementations can override for specialized rendering (e.g., hex editor).
pub trait ViewportRenderer: Send + Sync {
    /// Calculate the total gutter width from annotation modules.
    fn gutter_width(
        &self,
        modules: &[Box<dyn ClientModule>],
        caps: &dyn PlatformCapabilities,
    ) -> u16;

    /// Render the buffer viewport into the surface.
    #[allow(clippy::too_many_arguments)]
    fn render_viewport(
        &self,
        surface: &mut dyn RenderSurface,
        viewport: Rect,
        ctx: &ViewportContext<'_>,
        modules: &[Box<dyn ClientModule>],
        tokens: &dyn TokenProvider,
        theme: &dyn ThemeProvider,
        caps: &dyn PlatformCapabilities,
    );
}

// =============================================================================
// LayoutPolicy
// =============================================================================

/// Determines how windows are arranged in the viewport.
pub trait LayoutPolicy: Send + Sync {
    /// Compute window positions given the available viewport and window list.
    fn layout(&self, viewport: Rect, windows: &[WindowId], focused: WindowId) -> Vec<WindowLayout>;
}

#[cfg(test)]
mod tests;
