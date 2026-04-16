//! Text domain view module.
//!
//! [`TextDomainView`] implements [`ClientModule`] for the text editing domain.
//! It receives text domain projections and delegates buffer rendering to
//! [`DefaultViewportRenderer`].

use crate::{
    ClientModule, ClientModuleError, PlatformCapabilities, ProbeResult, Rect, RenderSurface,
    Style, ThemeProvider, TokenProvider, Version, ViewportContext, ViewportRenderer,
    projection::DomainProjection,
    traits::{CellGridClientModule, LocalInputResult, ModuleContext},
    viewport::DefaultViewportRenderer,
};
use reovim_subsys_coordination::DomainId;

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
    pub fn new(domain_id: DomainId) -> Self {
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
        surface: &mut dyn RenderSurface,
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
        surface: &mut dyn RenderSurface,
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
mod tests {
    use super::*;
    use crate::{
        Rect,
        projection::DomainProjection,
        testing::{MockPlatformCapabilities, RecordingSurface, TestModuleContext},
    };
    use reovim_subsys_coordination::ProjectionTag;

    fn text_domain() -> DomainId {
        DomainId(1)
    }

    #[test]
    fn text_domain_view_identity() {
        let view = TextDomainView::new(text_domain());
        assert_eq!(view.id(), "text-domain-view");
        assert_eq!(view.name(), "Text Domain View");
        assert_eq!(view.version(), Version::new(0, 1, 0));
    }

    #[test]
    fn text_domain_view_reports_domain_id() {
        let view = TextDomainView::new(text_domain());
        assert_eq!(view.domain_id(), Some(text_domain()));
    }

    #[test]
    fn text_domain_view_lifecycle() {
        let mut view = TextDomainView::new(text_domain());
        let ctx = {
            let ctx = Box::leak(Box::new(TestModuleContext::builder().build()));
            ctx.as_context()
        };
        assert!(matches!(view.init(&ctx), ProbeResult::Success));
        assert!(view.exit().is_ok());
    }

    #[test]
    fn text_domain_view_caches_mode_projection() {
        let mut view = TextDomainView::new(text_domain());
        assert!(view.mode_display().is_empty());

        let proj = DomainProjection::new(
            ProjectionTag::new("text.mode"),
            text_domain(),
            b"normal".to_vec(),
            "NORMAL".into(),
        );
        view.on_projection(&proj);
        assert_eq!(view.mode_display(), "NORMAL");
    }

    #[test]
    fn text_domain_view_ignores_other_domain() {
        let mut view = TextDomainView::new(text_domain());

        let proj = DomainProjection::new(
            ProjectionTag::new("text.mode"),
            DomainId(99), // different domain
            b"orbit".to_vec(),
            "ORBIT".into(),
        );
        view.on_projection(&proj);
        assert!(view.mode_display().is_empty());
    }

    #[test]
    fn text_domain_view_ignores_transient_mode() {
        let mut view = TextDomainView::new(text_domain());

        let proj = DomainProjection::new(
            ProjectionTag::new("text.mode"),
            text_domain(),
            b"normal".to_vec(),
            "NORMAL".into(),
        );
        view.on_projection(&proj);
        assert_eq!(view.mode_display(), "NORMAL");

        // Transient projection should not overwrite cached mode.
        let mut transient = DomainProjection::new(
            ProjectionTag::new("text.mode"),
            text_domain(),
            b"temp".to_vec(),
            "TEMP".into(),
        );
        transient.transient = true;
        view.on_projection(&transient);
        assert_eq!(view.mode_display(), "NORMAL");
    }

    #[test]
    fn text_domain_view_local_input_unhandled() {
        let mut view = TextDomainView::new(text_domain());
        assert!(!view.wants_local_input());
        assert_eq!(view.on_local_input(&[0x1b]), LocalInputResult::Unhandled);
    }

    #[test]
    fn text_domain_view_cell_grid_gutter() {
        let view = TextDomainView::new(text_domain());
        assert_eq!(CellGridClientModule::domain_gutter_width(&view), 0);
    }

    #[test]
    fn text_domain_view_gutter_width_delegates() {
        let view = TextDomainView::new(text_domain());
        let modules: Vec<Box<dyn ClientModule>> = vec![];
        let caps = MockPlatformCapabilities::new();
        // With no annotation modules, gutter width is 0.
        assert_eq!(view.gutter_width(&modules, &caps), 0);
    }

    struct EmptyTokens;
    impl crate::TokenProvider for EmptyTokens {
        fn tokens_for_line(&self, _: crate::BufferId, _: u32) -> Vec<crate::SyntaxToken> {
            Vec::new()
        }
    }

    #[test]
    fn text_domain_view_render_viewport_delegates() {
        let view = TextDomainView::new(text_domain());
        let mut surface = RecordingSurface::new(80, 24);
        let viewport = Rect::new(0, 0, 80, 24);
        let ctx = ViewportContext {
            buffer_id: None,
            buffer_lines: None,
            cursor: None,
            scroll_top: 0,
            local_selection: None,
            remote_clients: &[],
            fold_ranges: &[],
            virtual_lines: &[],
            opacity: 1.0,
            line_number_mode: crate::LineNumberMode::None,
            gutter_width: 0,
            sidebar_width: 0,
            is_insert_mode: false,
            render_self_cursor: false,
            my_client_id: 0,
        };
        let modules: Vec<Box<dyn ClientModule>> = vec![];
        let tokens = EmptyTokens;
        let theme = crate::testing::MockThemeProvider::new();
        let caps = MockPlatformCapabilities::new();

        // Should not panic — delegates to DefaultViewportRenderer.
        view.render_viewport(&mut surface, viewport, &ctx, &modules, &tokens, &theme, &caps);
    }
}
