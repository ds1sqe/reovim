//! Bridge adapter: wraps `TuiExtension` as `ClientModule`.
//!
//! Temporary adapter deleted in Phase 12.7 when all extensions are native
//! `ClientModule` implementations. Every `TuiExtension` is wrapped at
//! construction with per-extension role classification.

use reovim_client_driver::{
    AnnotationContext, BufferId, BufferUpdateEvent, ChromePosition, ClientModule,
    ClientModuleError, ColumnWidth, GutterCell, InlineDecoration, ModuleContext,
    PlatformCapabilities, ProbeResult, Rect, RenderBehavior, RenderSurface, TransformedLine,
    Version, VirtualLine, VirtualLinePosition,
};
use reovim_driver_display::render_backend::TuiExtension;
use reovim_driver_display::{
    render_backend::{
        RenderBackend, RenderBehavior as DisplayRenderBehavior,
        TransformedLine as DisplayTransformedLine,
        VirtualLinePosition as DisplayVirtualLinePosition,
    },
    Attributes as DisplayAttributes, Style as DisplayStyle,
};

use reovim_client_driver::types::Color;

// =============================================================================
// TuiExtensionBridge
// =============================================================================

/// Bridge adapter wrapping `TuiExtension` as `ClientModule`.
///
/// Temporary -- deleted in Phase 12.7 when all extensions are native
/// `ClientModule` implementations.
pub struct TuiExtensionBridge {
    inner: Box<dyn TuiExtension>,
    chrome: bool,
    buffer_contrib: bool,
    chrome_position: ChromePosition,
    chrome_priority: u16,
    cached_fold_ranges: Vec<(usize, usize)>,
    cached_virtual_lines: Vec<VirtualLine>,
    cached_inline_decorations: Vec<InlineDecoration>,
}

impl TuiExtensionBridge {
    /// Wrap a `TuiExtension` as a `ClientModule`.
    #[must_use]
    pub fn new(ext: Box<dyn TuiExtension>) -> Self {
        let (chrome, buffer_contrib, position, priority) = classify_extension(ext.kind());
        Self {
            inner: ext,
            chrome,
            buffer_contrib,
            chrome_position: position,
            chrome_priority: priority,
            cached_fold_ranges: Vec::new(),
            cached_virtual_lines: Vec::new(),
            cached_inline_decorations: Vec::new(),
        }
    }

    fn refresh_caches(&mut self) {
        self.cached_fold_ranges = self
            .inner
            .fold_hidden_lines()
            .iter()
            .map(|&(start, count)| (start as usize, count as usize))
            .collect();

        self.cached_virtual_lines = self
            .inner
            .virtual_lines()
            .iter()
            .map(|vl| VirtualLine {
                buffer_line: vl.buffer_line,
                position: match vl.position {
                    DisplayVirtualLinePosition::Before => VirtualLinePosition::Before,
                    DisplayVirtualLinePosition::After => VirtualLinePosition::After,
                },
                content: vl.content.clone(),
                style: convert_display_style_to_driver_style(&vl.style),
            })
            .collect();
    }
}

/// Bulk-wrap a vector of `TuiExtension`s into `ClientModule`s.
#[must_use]
pub fn wrap_extensions(extensions: Vec<Box<dyn TuiExtension>>) -> Vec<Box<dyn ClientModule>> {
    extensions
        .into_iter()
        .map(|ext| -> Box<dyn ClientModule> { Box::new(TuiExtensionBridge::new(ext)) })
        .collect()
}

// =============================================================================
// Per-extension classification
// =============================================================================

/// Classify an extension by its `kind()` into (chrome, `buffer_contrib`, position, priority).
fn classify_extension(kind: &str) -> (bool, bool, ChromePosition, u16) {
    match kind {
        "cmdline" => (true, false, ChromePosition::Bottom, 90),
        "whichkey" => (true, false, ChromePosition::Overlay, 50),
        "notification" => (true, false, ChromePosition::Overlay, 40),
        "microscope" => (true, false, ChromePosition::Overlay, 80),
        "completion" => (true, false, ChromePosition::Overlay, 70),
        "explorer" => (true, false, ChromePosition::Left, 60),
        "hover" => (true, false, ChromePosition::Overlay, 45),
        "signature-help" => (true, false, ChromePosition::Overlay, 44),
        "landing" => (true, false, ChromePosition::Overlay, 30),
        "polyblocks" => (true, false, ChromePosition::Overlay, 10),
        "pair" | "markdown" | "diagnostics" | "range-finder-jump" | "range-finder-fold" => {
            (false, true, ChromePosition::Bottom, 0)
        }
        _ => (true, false, ChromePosition::Overlay, 0),
    }
}

// =============================================================================
// ClientModule implementation
// =============================================================================

impl ClientModule for TuiExtensionBridge {
    // ---- Identity ----

    fn id(&self) -> &'static str {
        self.inner.kind()
    }

    fn kind(&self) -> &'static str {
        self.inner.kind()
    }

    fn name(&self) -> &'static str {
        self.inner.kind()
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn dependencies(&self) -> &[&str] {
        self.inner.dependencies()
    }

    fn server_kinds(&self) -> Vec<&'static str> {
        self.inner.server_kinds()
    }

    // ---- Lifecycle ----

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        self.inner.init();
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        self.inner.exit();
        Ok(())
    }

    // ---- Role ----

    fn has_chrome(&self) -> bool {
        self.chrome
    }

    fn has_buffer_contrib(&self) -> bool {
        self.buffer_contrib
    }

    fn has_annotations(&self) -> bool {
        false
    }

    // ---- Events ----

    fn on_notification(&mut self, data: &str) {
        self.inner.apply_notification(data);
        self.refresh_caches();
    }

    fn on_mode_change(&mut self, mode: &str) {
        let is_insert = mode.eq_ignore_ascii_case("insert");
        self.inner.on_mode_change(mode, is_insert);
        self.refresh_caches();
    }

    fn on_buffer_update(&mut self, event: &BufferUpdateEvent) {
        #[allow(clippy::cast_possible_truncation)]
        let buffer_id = event.buffer_id.0 as u64;
        self.inner.on_buffer_update(buffer_id, &event.new_lines);
        self.refresh_caches();
    }

    fn on_cursor_update(&mut self, buffer_id: BufferId, line: usize, col: usize) {
        #[allow(clippy::cast_possible_truncation)]
        let bid = buffer_id.0 as u64;
        self.inner.on_cursor_update(bid, line, col);
        self.refresh_caches();
    }

    fn tick(&mut self) -> bool {
        let changed = self.inner.tick();
        if changed {
            self.refresh_caches();
        }
        changed
    }

    // ---- Chrome ----

    fn chrome_position(&self) -> ChromePosition {
        self.chrome_position
    }

    fn chrome_requested_size(&self, _caps: &dyn PlatformCapabilities) -> u16 {
        if self.inner.kind() == "explorer" {
            self.inner.content_offset_left()
        } else {
            1
        }
    }

    fn chrome_priority(&self) -> u16 {
        self.chrome_priority
    }

    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        if !self.inner.is_active() {
            return;
        }
        let mut adapter = SurfaceBackendAdapter::new(surface, bounds);
        self.inner.render(&mut adapter);
    }

    // ---- Buffer contrib ----

    fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
        self.inner
            .classify_token(category)
            .map(|old| convert_render_behavior(&old))
    }

    fn transform_line(
        &self,
        buf: BufferId,
        line: usize,
        text: &str,
    ) -> Option<TransformedLine> {
        #[allow(clippy::cast_possible_truncation)]
        let bid = buf.0 as u64;
        self.inner
            .transform_line(bid, line, text)
            .map(|old| convert_transformed_line(&old))
    }

    fn map_cursor_column(&self, buf: BufferId, line: usize, col: usize) -> Option<u16> {
        #[allow(clippy::cast_possible_truncation)]
        let bid = buf.0 as u64;
        self.inner.map_cursor_column(bid, line, col)
    }

    fn fold_ranges(&self) -> &[(usize, usize)] {
        &self.cached_fold_ranges
    }

    fn virtual_lines(&self) -> &[VirtualLine] {
        &self.cached_virtual_lines
    }

    fn inline_decorations(&self, _line: usize) -> &[InlineDecoration] {
        &self.cached_inline_decorations
    }

    fn cursor_position(&self, w: u16, h: u16) -> Option<(u16, u16)> {
        self.inner.cursor_position(w, h)
    }

    // ---- Annotations ----

    fn annotation_column_width(
        &self,
        _ctx: &AnnotationContext,
        _caps: &dyn PlatformCapabilities,
    ) -> ColumnWidth {
        ColumnWidth::Fixed(0)
    }

    fn annotate(&self, _line: usize, _ctx: &AnnotationContext) -> Option<GutterCell> {
        None
    }

    fn annotation_priority(&self) -> u16 {
        0
    }
}

// =============================================================================
// SurfaceBackendAdapter
// =============================================================================

/// Wraps `&mut dyn RenderSurface` as `RenderBackend` for legacy extensions.
///
/// Chrome extensions call `RenderBackend` methods; this adapter translates
/// those calls to the new `RenderSurface` trait.
struct SurfaceBackendAdapter<'a> {
    surface: &'a mut dyn RenderSurface,
    bounds: Rect,
}

impl<'a> SurfaceBackendAdapter<'a> {
    fn new(surface: &'a mut dyn RenderSurface, bounds: Rect) -> Self {
        Self { surface, bounds }
    }
}

impl RenderBackend for SurfaceBackendAdapter<'_> {
    fn set_cell(&mut self, x: u16, y: u16, ch: char, style: &DisplayStyle) {
        let driver_style = convert_display_style_to_driver_style(style);
        let s = String::from(ch);
        self.surface.write_styled(
            self.bounds.x.saturating_add(x),
            self.bounds.y.saturating_add(y),
            &s,
            driver_style,
        );
    }

    fn write_str(&mut self, x: u16, y: u16, text: &str, style: &DisplayStyle) -> u16 {
        let driver_style = convert_display_style_to_driver_style(style);
        self.surface.write_styled(
            self.bounds.x.saturating_add(x),
            self.bounds.y.saturating_add(y),
            text,
            driver_style,
        )
    }

    fn apply_style(&mut self, x: u16, y: u16, style: &DisplayStyle) {
        let driver_style = convert_display_style_to_driver_style(style);
        self.surface.apply_style(
            self.bounds.x.saturating_add(x),
            self.bounds.y.saturating_add(y),
            driver_style,
        );
    }

    fn size(&self) -> (u16, u16) {
        (self.bounds.width, self.bounds.height)
    }

    fn clear(&mut self) {
        self.surface.clear(self.bounds);
    }

    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
        self.surface.overlay_bg(
            self.bounds.x.saturating_add(x),
            self.bounds.y.saturating_add(y),
            bg,
        );
    }
}

// =============================================================================
// Type conversion helpers
// =============================================================================

/// Convert display driver `RenderBehavior` to client driver `RenderBehavior`.
fn convert_render_behavior(old: &DisplayRenderBehavior) -> RenderBehavior {
    match old {
        DisplayRenderBehavior::Highlight => RenderBehavior::Highlight,
        DisplayRenderBehavior::Conceal { replacement } => RenderBehavior::Conceal {
            replacement: replacement.clone(),
        },
        DisplayRenderBehavior::Background => {
            RenderBehavior::Background(Color::default())
        }
        DisplayRenderBehavior::Hide => RenderBehavior::Hide,
        DisplayRenderBehavior::FullWidthLine { ch } => RenderBehavior::FullWidthLine {
            ch: *ch,
            style: reovim_client_driver::Style::default(),
        },
    }
}

/// Convert display driver `TransformedLine` to client driver `TransformedLine`.
///
/// Old format: `{ text: String, styles: Vec<Option<Style>> }` (per-character).
/// New format: `{ segments: Vec<(String, Option<Style>)> }` (run-length).
fn convert_transformed_line(old: &DisplayTransformedLine) -> TransformedLine {
    let mut segments = Vec::new();
    let mut current_text = String::new();
    let mut current_style: Option<Option<DisplayStyle>> = None;

    for (i, ch) in old.text.chars().enumerate() {
        let style = old.styles.get(i).cloned().flatten();

        if current_style.as_ref() == Some(&style) {
            current_text.push(ch);
        } else {
            if !current_text.is_empty() {
                let converted = current_style
                    .take()
                    .flatten()
                    .as_ref()
                    .map(convert_display_style_to_driver_style);
                segments.push((std::mem::take(&mut current_text), converted));
            }
            current_text.push(ch);
            current_style = Some(style);
        }
    }

    if !current_text.is_empty() {
        let converted = current_style
            .flatten()
            .as_ref()
            .map(convert_display_style_to_driver_style);
        segments.push((current_text, converted));
    }

    TransformedLine { segments }
}

/// Convert display driver `Style` to client driver `Style`.
const fn convert_display_style_to_driver_style(
    display: &DisplayStyle,
) -> reovim_client_driver::Style {
    let mut attrs = reovim_client_driver::Attributes::new();

    if display.attributes.contains(DisplayAttributes::BOLD) {
        attrs.set(reovim_client_driver::Attributes::BOLD);
    }
    if display.attributes.contains(DisplayAttributes::ITALIC) {
        attrs.set(reovim_client_driver::Attributes::ITALIC);
    }
    if display.attributes.contains(DisplayAttributes::UNDERLINE) {
        attrs.set(reovim_client_driver::Attributes::UNDERLINE);
    }
    if display.attributes.contains(DisplayAttributes::STRIKETHROUGH) {
        attrs.set(reovim_client_driver::Attributes::STRIKETHROUGH);
    }
    if display.attributes.contains(DisplayAttributes::REVERSE) {
        attrs.set(reovim_client_driver::Attributes::REVERSE);
    }
    if display.attributes.contains(DisplayAttributes::DIM) {
        attrs.set(reovim_client_driver::Attributes::DIM);
    }

    reovim_client_driver::Style {
        fg: display.fg,
        bg: display.bg,
        attributes: attrs,
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
