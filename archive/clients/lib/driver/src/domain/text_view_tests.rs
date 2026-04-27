use {
    super::*,
    crate::{
        Rect,
        projection::DomainProjection,
        testing::{MockPlatformCapabilities, TestModuleContext, surface::TestChromeGrid},
    },
    reovim_subsys_coordination::ProjectionTag,
};

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
        DomainId(99),
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
    let mut surface = TestChromeGrid::new(80, 24);
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

    view.render_viewport(&mut surface, viewport, &ctx, &modules, &tokens, &theme, &caps);
}
