//! Text domain view module.
//!
//! [`TextDomainView`] implements [`ClientModule`] for the text editing domain.
//! It receives text domain projections and delegates buffer rendering to
//! [`DefaultViewportRenderer`].

use {
    crate::{
        ChromeSurface, ClientModule, ClientModuleError, PlatformCapabilities, ProbeResult, Rect,
        Style, ThemeProvider, TokenProvider, Version, ViewportContext, ViewportRenderer,
        projection::DomainProjection,
        traits::{CellGridClientModule, LocalInputResult, ModuleContext},
        viewport::DefaultViewportRenderer,
    },
    reovim_subsys_coordination::DomainId,
};

/// Text domain view module.
///
/// Bridges the domain-neutral projection architecture with the existing
/// text viewport renderer. Receives text domain projections (mode, cursor,
/// selection, viewport state) and delegates buffer rendering to
/// [`DefaultViewportRenderer`].
///
/// This is delegation, not a facade — `TextDomainView` adds domain-aware
/// projection routing while forwarding rendering to the existing renderer.
pub struct TextDomainView {
    domain_id: DomainId,
    renderer: DefaultViewportRenderer,
    /// Cached mode display string (from "text.mode" projection).
    mode_display: String,
}

impl TextDomainView {
    /// Create a new text domain view for the given domain.
    #[must_use]
    pub const fn new(domain_id: DomainId) -> Self {
        Self {
            domain_id,
            renderer: DefaultViewportRenderer,
            mode_display: String::new(),
        }
    }

    /// Cached mode display string from the latest "text.mode" projection.
    #[must_use]
    pub fn mode_display(&self) -> &str {
        &self.mode_display
    }

    /// Delegate full viewport rendering to the wrapped renderer.
    ///
    /// Called by the TUI compositor with the complete [`ViewportContext`].
    /// This is the primary rendering path — `render_domain_surface()` is
    /// for the generic domain view interface.
    #[allow(clippy::too_many_arguments)]
    pub fn render_viewport(
        &self,
        surface: &mut dyn ChromeSurface,
        viewport: Rect,
        ctx: &ViewportContext<'_>,
        modules: &[Box<dyn ClientModule>],
        tokens: &dyn TokenProvider,
        theme: &dyn ThemeProvider,
        caps: &dyn PlatformCapabilities,
    ) {
        self.renderer
            .render_viewport(surface, viewport, ctx, modules, tokens, theme, caps);
    }

    /// Compute gutter width via the wrapped renderer.
    pub fn gutter_width(
        &self,
        modules: &[Box<dyn ClientModule>],
        caps: &dyn PlatformCapabilities,
    ) -> u16 {
        self.renderer.gutter_width(modules, caps)
    }
}

impl ClientModule for TextDomainView {
    fn id(&self) -> &'static str {
        "text-domain-view"
    }

    fn name(&self) -> &'static str {
        "Text Domain View"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }

    fn domain_id(&self) -> Option<DomainId> {
        Some(self.domain_id)
    }

    fn on_projection(&mut self, projection: &DomainProjection) {
        if projection.domain_id != self.domain_id {
            return;
        }
        // Cache mode display string from "text.mode" projections.
        if projection.tag.as_str() == "text.mode" && !projection.transient {
            self.mode_display.clone_from(&projection.display);
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn render_domain_surface(
        &self,
        surface: &mut dyn ChromeSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        // Minimal rendering from projection cache.
        // Full rendering goes through render_viewport() delegation.
        if !self.mode_display.is_empty() && bounds.width > 0 && bounds.height > 0 {
            let style = Style::default();
            surface.write_styled(bounds.x, bounds.y, &self.mode_display, style);
        }
    }

    fn wants_local_input(&self) -> bool {
        false
    }

    fn on_local_input(&mut self, _input: &[u8]) -> LocalInputResult {
        LocalInputResult::Unhandled
    }
}

impl CellGridClientModule for TextDomainView {
    fn domain_gutter_width(&self) -> u16 {
        // Text domain needs gutter space for line numbers.
        // Actual width is computed dynamically by the compositor
        // via gutter_width() which queries annotation modules.
        0
    }
}

#[cfg(test)]
#[path = "text_view_tests.rs"]
mod tests;
