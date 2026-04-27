use {
    super::*,
    crate::draw::{ColorDepth, Insets, RenderingModel},
};

struct StubPlatform;

impl PlatformCapabilities for StubPlatform {
    fn rendering_model(&self) -> RenderingModel {
        RenderingModel::CellGrid
    }

    fn grid_size(&self) -> Option<(u16, u16)> {
        Some((80, 24))
    }

    fn color_depth(&self) -> ColorDepth {
        ColorDepth::TrueColor
    }

    fn pixel_size(&self) -> Option<(u32, u32)> {
        None
    }

    fn reliable_unicode_width(&self) -> bool {
        true
    }

    fn dark_mode(&self) -> bool {
        true
    }

    fn smooth_scroll(&self) -> bool {
        false
    }

    fn pointer_events(&self) -> bool {
        true
    }

    fn touch_input(&self) -> bool {
        false
    }

    fn haptic(&self) -> bool {
        false
    }

    fn safe_area(&self) -> Insets {
        Insets::ZERO
    }

    fn has_focus(&self) -> bool {
        true
    }

    fn clipboard_available(&self) -> bool {
        true
    }

    fn screen_reader_active(&self) -> bool {
        false
    }
}

#[test]
fn platform_capabilities_rendering_model() {
    let p = StubPlatform;
    assert_eq!(p.rendering_model(), RenderingModel::CellGrid);
}

#[test]
fn platform_capabilities_grid_size() {
    let p = StubPlatform;
    assert_eq!(p.grid_size(), Some((80, 24)));
}

#[test]
fn platform_capabilities_color_depth() {
    let p = StubPlatform;
    assert_eq!(p.color_depth(), ColorDepth::TrueColor);
}

#[test]
fn platform_capabilities_pixel_size_none() {
    let p = StubPlatform;
    assert!(p.pixel_size().is_none());
}

#[test]
fn platform_capabilities_flags() {
    let p = StubPlatform;
    assert!(p.reliable_unicode_width());
    assert!(p.dark_mode());
    assert!(!p.smooth_scroll());
    assert!(p.pointer_events());
    assert!(!p.touch_input());
    assert!(!p.haptic());
    assert!(p.has_focus());
    assert!(p.clipboard_available());
    assert!(!p.screen_reader_active());
}

#[test]
fn platform_capabilities_safe_area() {
    let p = StubPlatform;
    assert_eq!(p.safe_area(), Insets::ZERO);
}

#[test]
fn platform_capabilities_object_safety() {
    let p: Box<dyn PlatformCapabilities> = Box::new(StubPlatform);
    assert_eq!(p.rendering_model(), RenderingModel::CellGrid);
}
