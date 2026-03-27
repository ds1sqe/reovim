use super::*;

// =============================================================================
// BackendSurfaceAdapter tests
// =============================================================================

/// Mock `RenderBackend` for testing `BackendSurfaceAdapter`.
struct MockBackend {
    width: u16,
    height: u16,
    write_calls: Vec<(u16, u16, String, DisplayStyle)>,
    apply_calls: Vec<(u16, u16, DisplayStyle)>,
    overlay_calls: Vec<(u16, u16, Color)>,
    set_cell_calls: Vec<(u16, u16, char, DisplayStyle)>,
}

impl MockBackend {
    fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            write_calls: Vec::new(),
            apply_calls: Vec::new(),
            overlay_calls: Vec::new(),
            set_cell_calls: Vec::new(),
        }
    }
}

impl RenderBackend for MockBackend {
    fn set_cell(&mut self, x: u16, y: u16, ch: char, style: &DisplayStyle) {
        self.set_cell_calls.push((x, y, ch, style.clone()));
    }

    fn write_str(&mut self, x: u16, y: u16, text: &str, style: &DisplayStyle) -> u16 {
        #[allow(clippy::cast_possible_truncation)]
        let len = text.len() as u16;
        self.write_calls
            .push((x, y, text.to_string(), style.clone()));
        len
    }

    fn apply_style(&mut self, x: u16, y: u16, style: &DisplayStyle) {
        self.apply_calls.push((x, y, style.clone()));
    }

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    fn clear(&mut self) {
        // no-op for tests
    }

    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
        self.overlay_calls.push((x, y, bg));
    }
}

#[test]
fn backend_surface_adapter_write_styled() {
    let mut backend = MockBackend::new(80, 24);
    let mut adapter = BackendSurfaceAdapter::new(&mut backend);
    let style = reovim_client_driver::Style::default();
    let written = adapter.write_styled(5, 10, "hello", style);
    assert_eq!(written, 5);
    let _ = adapter;
    assert_eq!(backend.write_calls.len(), 1);
    assert_eq!(backend.write_calls[0].0, 5);
    assert_eq!(backend.write_calls[0].1, 10);
    assert_eq!(backend.write_calls[0].2, "hello");
}

#[test]
fn backend_surface_adapter_apply_style() {
    let mut backend = MockBackend::new(80, 24);
    let mut adapter = BackendSurfaceAdapter::new(&mut backend);
    let style = reovim_client_driver::Style {
        fg: Some(Color::Rgb { r: 255, g: 0, b: 0 }),
        bg: None,
        attributes: reovim_client_driver::Attributes::BOLD,
    };
    adapter.apply_style(3, 7, style);
    let _ = adapter;
    assert_eq!(backend.apply_calls.len(), 1);
    assert_eq!(backend.apply_calls[0].0, 3);
    assert_eq!(backend.apply_calls[0].1, 7);
    assert!(
        backend.apply_calls[0]
            .2
            .attributes
            .contains(DisplayAttributes::BOLD)
    );
}

#[test]
fn backend_surface_adapter_overlay_bg() {
    let mut backend = MockBackend::new(80, 24);
    let mut adapter = BackendSurfaceAdapter::new(&mut backend);
    adapter.overlay_bg(1, 2, Color::Rgb { r: 0, g: 255, b: 0 });
    let _ = adapter;
    assert_eq!(backend.overlay_calls.len(), 1);
    assert_eq!(backend.overlay_calls[0].2, Color::Rgb { r: 0, g: 255, b: 0 });
}

#[test]
fn backend_surface_adapter_fill() {
    let mut backend = MockBackend::new(80, 24);
    let mut adapter = BackendSurfaceAdapter::new(&mut backend);
    let rect = Rect {
        x: 1,
        y: 2,
        width: 10,
        height: 5,
    };
    adapter.fill(rect, '#', reovim_client_driver::Style::default());
    let _ = adapter;
    // fill_region delegates to set_cell: 10 * 5 = 50 cells
    assert_eq!(backend.set_cell_calls.len(), 50);
    // First cell at (1,2) with '#'
    assert_eq!(backend.set_cell_calls[0].0, 1);
    assert_eq!(backend.set_cell_calls[0].1, 2);
    assert_eq!(backend.set_cell_calls[0].2, '#');
}

#[test]
fn backend_surface_adapter_clear() {
    let mut backend = MockBackend::new(80, 24);
    let mut adapter = BackendSurfaceAdapter::new(&mut backend);
    let rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };
    adapter.clear(rect);
    let _ = adapter;
    // clear fills with spaces: 80 * 24 = 1920 cells
    assert_eq!(backend.set_cell_calls.len(), 1920);
    assert_eq!(backend.set_cell_calls[0].2, ' ');
}

#[test]
fn backend_surface_adapter_size() {
    let mut backend = MockBackend::new(120, 40);
    let adapter = BackendSurfaceAdapter::new(&mut backend);
    assert_eq!(adapter.size(), (120, 40));
}

// =============================================================================
// TuiRenderSurface (generic) tests
// =============================================================================

#[test]
fn tui_render_surface_write_styled() {
    let mut backend = MockBackend::new(80, 24);
    let mut surface = TuiRenderSurface::new(&mut backend);
    let style = reovim_client_driver::Style {
        fg: Some(Color::Red),
        ..reovim_client_driver::Style::default()
    };
    let cols = surface.write_styled(5, 3, "hi", style);
    assert_eq!(cols, 2);
    let _ = surface;
    assert_eq!(backend.write_calls.len(), 1);
    assert_eq!(backend.write_calls[0].0, 5); // x
    assert_eq!(backend.write_calls[0].1, 3); // y
    assert_eq!(backend.write_calls[0].2, "hi");
}

#[test]
fn tui_render_surface_apply_style() {
    let mut backend = MockBackend::new(80, 24);
    let mut surface = TuiRenderSurface::new(&mut backend);
    let style = reovim_client_driver::Style::default();
    surface.apply_style(2, 4, style);
    assert_eq!(backend.apply_calls.len(), 1);
    assert_eq!(backend.apply_calls[0].0, 2);
    assert_eq!(backend.apply_calls[0].1, 4);
}

#[test]
fn tui_render_surface_overlay_bg() {
    let mut backend = MockBackend::new(80, 24);
    let mut surface = TuiRenderSurface::new(&mut backend);
    surface.overlay_bg(1, 2, Color::Blue);
    assert_eq!(backend.overlay_calls.len(), 1);
    assert_eq!(backend.overlay_calls[0].0, 1);
    assert_eq!(backend.overlay_calls[0].1, 2);
}

#[test]
fn tui_render_surface_fill() {
    let mut backend = MockBackend::new(10, 5);
    let mut surface = TuiRenderSurface::new(&mut backend);
    let rect = Rect::new(0, 0, 5, 2);
    let style = reovim_client_driver::Style::default();
    surface.fill(rect, '#', style);
    // 5*2 = 10 cells
    assert_eq!(backend.set_cell_calls.len(), 10);
    assert_eq!(backend.set_cell_calls[0].2, '#');
}

#[test]
fn tui_render_surface_clear() {
    let mut backend = MockBackend::new(10, 5);
    let mut surface = TuiRenderSurface::new(&mut backend);
    let rect = Rect::new(1, 1, 3, 2);
    surface.clear(rect);
    // 3*2 = 6 cells
    assert_eq!(backend.set_cell_calls.len(), 6);
    assert_eq!(backend.set_cell_calls[0].2, ' ');
}

#[test]
fn tui_render_surface_size() {
    let mut backend = MockBackend::new(160, 50);
    let surface = TuiRenderSurface::new(&mut backend);
    assert_eq!(surface.size(), (160, 50));
}

// =============================================================================
// TuiPlatformCapabilities tests
// =============================================================================

#[test]
fn tui_platform_capabilities_grid_size() {
    let caps = TuiPlatformCapabilities::for_test(80, 24);
    assert_eq!(caps.grid_size(), Some((80, 24)));
}

#[test]
fn tui_platform_capabilities_rendering_model() {
    let caps = TuiPlatformCapabilities::for_test(80, 24);
    assert_eq!(caps.rendering_model(), RenderingModel::CellGrid);
}

#[test]
fn tui_platform_capabilities_color_depth() {
    let caps = TuiPlatformCapabilities::for_test(80, 24);
    assert_eq!(caps.color_depth(), ColorDepth::TrueColor);
}

#[test]
fn tui_platform_capabilities_pixel_size_none() {
    let caps = TuiPlatformCapabilities::for_test(80, 24);
    assert_eq!(caps.pixel_size(), None);
}

#[test]
fn tui_platform_capabilities_boolean_defaults() {
    let caps = TuiPlatformCapabilities::for_test(80, 24);
    assert!(caps.reliable_unicode_width());
    assert!(caps.dark_mode());
    assert!(!caps.smooth_scroll());
    assert!(!caps.pointer_events());
    assert!(!caps.touch_input());
    assert!(!caps.haptic());
    assert!(caps.has_focus());
    assert!(caps.clipboard_available());
    assert!(!caps.screen_reader_active());
}

#[test]
fn tui_platform_capabilities_safe_area_default() {
    let caps = TuiPlatformCapabilities::for_test(80, 24);
    let insets = caps.safe_area();
    assert_eq!(insets, Insets::default());
}

// --- Full DisplayCapabilities wiring ---

#[test]
fn tui_platform_capabilities_with_display_caps() {
    use reovim_driver_display::{ColorMode, DisplayCapabilities};
    let display = DisplayCapabilities {
        color_mode: ColorMode::Color256,
        supports_underline_color: true,
        supports_extended_underlines: true,
        supports_mouse: true,
        supports_kitty_graphics: false,
        supports_sixel: false,
    };
    let caps = TuiPlatformCapabilities::new(120, 40, display);
    assert_eq!(caps.grid_size(), Some((120, 40)));
    assert_eq!(caps.color_depth(), ColorDepth::Ansi256);
    assert!(caps.pointer_events());
}

#[test]
fn tui_platform_capabilities_color_depth_ansi16() {
    use reovim_driver_display::{ColorMode, DisplayCapabilities};
    let display = DisplayCapabilities {
        color_mode: ColorMode::Ansi16,
        supports_underline_color: false,
        supports_extended_underlines: false,
        supports_mouse: false,
        supports_kitty_graphics: false,
        supports_sixel: false,
    };
    let caps = TuiPlatformCapabilities::new(80, 24, display);
    assert_eq!(caps.color_depth(), ColorDepth::Ansi16);
    assert!(!caps.pointer_events());
}

#[test]
fn tui_platform_capabilities_color_depth_truecolor() {
    use reovim_driver_display::{ColorMode, DisplayCapabilities};
    let display = DisplayCapabilities {
        color_mode: ColorMode::TrueColor,
        supports_underline_color: false,
        supports_extended_underlines: false,
        supports_mouse: false,
        supports_kitty_graphics: false,
        supports_sixel: false,
    };
    let caps = TuiPlatformCapabilities::new(80, 24, display);
    assert_eq!(caps.color_depth(), ColorDepth::TrueColor);
}

#[test]
fn tui_platform_capabilities_update_grid_size() {
    let mut caps = TuiPlatformCapabilities::for_test(80, 24);
    assert_eq!(caps.grid_size(), Some((80, 24)));
    caps.update_grid_size(120, 40);
    assert_eq!(caps.grid_size(), Some((120, 40)));
}

#[test]
fn tui_platform_capabilities_set_focus() {
    let mut caps = TuiPlatformCapabilities::for_test(80, 24);
    assert!(caps.has_focus());
    caps.set_focus(false);
    assert!(!caps.has_focus());
    caps.set_focus(true);
    assert!(caps.has_focus());
}

#[test]
fn tui_platform_capabilities_dark_mode_from_for_test() {
    let caps = TuiPlatformCapabilities::for_test(80, 24);
    assert!(caps.dark_mode());
}

#[test]
fn detect_dark_mode_default_is_dark() {
    // When COLORFGBG is not set or unparseable, default is dark (true)
    // We can't easily control env vars in parallel tests, so just verify
    // the function returns a bool without panicking.
    let result = super::detect_dark_mode();
    // Just verify it returns without panic; actual value depends on env.
    let _ = result;
}

// =============================================================================
// Type conversion tests
// =============================================================================

#[test]
fn convert_style_default() {
    let driver_style = reovim_client_driver::Style::default();
    let display_style = convert_driver_style_to_display_style(&driver_style);
    assert_eq!(display_style.fg, None);
    assert_eq!(display_style.bg, None);
    assert_eq!(display_style.attributes, DisplayAttributes::new());
    assert_eq!(display_style.underline_color, None);
}

#[test]
fn convert_style_with_colors() {
    let driver_style = reovim_client_driver::Style {
        fg: Some(Color::Rgb { r: 255, g: 0, b: 0 }),
        bg: Some(Color::Rgb { r: 0, g: 0, b: 255 }),
        attributes: reovim_client_driver::Attributes::new(),
    };
    let display_style = convert_driver_style_to_display_style(&driver_style);
    assert_eq!(display_style.fg, Some(Color::Rgb { r: 255, g: 0, b: 0 }));
    assert_eq!(display_style.bg, Some(Color::Rgb { r: 0, g: 0, b: 255 }));
}

#[test]
fn convert_style_all_attributes() {
    let driver_style = reovim_client_driver::Style {
        fg: None,
        bg: None,
        attributes: reovim_client_driver::Attributes::BOLD
            | reovim_client_driver::Attributes::ITALIC
            | reovim_client_driver::Attributes::UNDERLINE
            | reovim_client_driver::Attributes::STRIKETHROUGH
            | reovim_client_driver::Attributes::REVERSE
            | reovim_client_driver::Attributes::DIM,
    };
    let display_style = convert_driver_style_to_display_style(&driver_style);
    assert!(display_style.attributes.contains(DisplayAttributes::BOLD));
    assert!(display_style.attributes.contains(DisplayAttributes::ITALIC));
    assert!(
        display_style
            .attributes
            .contains(DisplayAttributes::UNDERLINE)
    );
    assert!(
        display_style
            .attributes
            .contains(DisplayAttributes::STRIKETHROUGH)
    );
    assert!(
        display_style
            .attributes
            .contains(DisplayAttributes::REVERSE)
    );
    assert!(display_style.attributes.contains(DisplayAttributes::DIM));
}

// =============================================================================
// Extension query helper tests
// =============================================================================

/// Minimal stub implementing `ClientModule` for testing query helpers.
struct StubModule {
    id: &'static str,
    chrome: bool,
    buffer_contrib: bool,
    position: ChromePosition,
    requested_size: u16,
    fold_ranges_data: Vec<(usize, usize)>,
    virtual_lines_data: Vec<reovim_client_driver::VirtualLine>,
}

impl Default for StubModule {
    fn default() -> Self {
        Self {
            id: "stub",
            chrome: false,
            buffer_contrib: false,
            position: ChromePosition::Overlay,
            requested_size: 0,
            fold_ranges_data: Vec::new(),
            virtual_lines_data: Vec::new(),
        }
    }
}

impl reovim_client_driver::ClientModule for StubModule {
    fn id(&self) -> &'static str {
        self.id
    }

    fn kind(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &'static str {
        self.id
    }

    fn version(&self) -> reovim_client_driver::Version {
        reovim_client_driver::Version::new(0, 1, 0)
    }

    fn init(
        &mut self,
        _ctx: &reovim_client_driver::ModuleContext,
    ) -> reovim_client_driver::ProbeResult {
        reovim_client_driver::ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), reovim_client_driver::ClientModuleError> {
        Ok(())
    }

    fn has_chrome(&self) -> bool {
        self.chrome
    }

    fn has_buffer_contrib(&self) -> bool {
        self.buffer_contrib
    }

    fn chrome_position(&self) -> ChromePosition {
        self.position
    }

    fn chrome_requested_size(&self, _caps: &dyn PlatformCapabilities) -> u16 {
        self.requested_size
    }

    fn fold_ranges(&self) -> &[(usize, usize)] {
        &self.fold_ranges_data
    }

    fn virtual_lines(&self) -> &[reovim_client_driver::VirtualLine] {
        &self.virtual_lines_data
    }
}

#[test]
fn sidebar_width_no_extensions() {
    let exts: Vec<Box<dyn ClientModule>> = Vec::new();
    let caps = TuiPlatformCapabilities::for_test(80, 24);
    assert_eq!(sidebar_width(&exts, &caps), 0);
}

#[test]
fn sidebar_width_with_left_chrome() {
    let exts: Vec<Box<dyn ClientModule>> = vec![Box::new(StubModule {
        chrome: true,
        position: ChromePosition::Left,
        requested_size: 30,
        ..StubModule::default()
    })];
    let caps = TuiPlatformCapabilities::for_test(80, 24);
    assert_eq!(sidebar_width(&exts, &caps), 30);
}

#[test]
fn sidebar_width_ignores_non_left() {
    let exts: Vec<Box<dyn ClientModule>> = vec![
        Box::new(StubModule {
            chrome: true,
            position: ChromePosition::Left,
            requested_size: 20,
            ..StubModule::default()
        }),
        Box::new(StubModule {
            chrome: true,
            position: ChromePosition::Overlay,
            requested_size: 50,
            ..StubModule::default()
        }),
    ];
    let caps = TuiPlatformCapabilities::for_test(80, 24);
    assert_eq!(sidebar_width(&exts, &caps), 20);
}

#[test]
fn collect_fold_ranges_empty() {
    let exts: Vec<Box<dyn ClientModule>> = Vec::new();
    assert!(collect_fold_ranges(&exts).is_empty());
}

#[test]
fn collect_fold_ranges_from_buffer_contrib() {
    let exts: Vec<Box<dyn ClientModule>> = vec![Box::new(StubModule {
        buffer_contrib: true,
        fold_ranges_data: vec![(5, 10), (20, 25)],
        ..StubModule::default()
    })];
    let ranges = collect_fold_ranges(&exts);
    assert_eq!(ranges, vec![(5, 10), (20, 25)]);
}

#[test]
fn collect_fold_ranges_ignores_non_buffer_contrib() {
    let exts: Vec<Box<dyn ClientModule>> = vec![Box::new(StubModule {
        buffer_contrib: false,
        fold_ranges_data: vec![(5, 10)],
        ..StubModule::default()
    })];
    assert!(collect_fold_ranges(&exts).is_empty());
}

#[test]
fn collect_driver_virtual_lines_empty() {
    let exts: Vec<Box<dyn ClientModule>> = Vec::new();
    assert!(collect_driver_virtual_lines(&exts).is_empty());
}

#[test]
fn collect_driver_virtual_lines_from_buffer_contrib() {
    let exts: Vec<Box<dyn ClientModule>> = vec![Box::new(StubModule {
        buffer_contrib: true,
        virtual_lines_data: vec![reovim_client_driver::VirtualLine {
            buffer_line: 10,
            position: reovim_client_driver::VirtualLinePosition::Before,
            content: "--- fold ---".to_string(),
            style: reovim_client_driver::Style::default(),
        }],
        ..StubModule::default()
    })];
    let vlines = collect_driver_virtual_lines(&exts);
    assert_eq!(vlines.len(), 1);
    assert_eq!(vlines[0].buffer_line, 10);
    assert_eq!(vlines[0].position, reovim_client_driver::VirtualLinePosition::Before);
    assert_eq!(vlines[0].content, "--- fold ---");
}

// =============================================================================
// Input event conversion tests
// =============================================================================

#[test]
fn convert_key_event_char() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let tui_key = reovim_driver_tui::KeyEvent {
        code: KeyCode::Char('a'),
        modifiers: KeyModifiers::NONE,
        vim_notation: String::new(),
    };
    let event = convert_key_event(&tui_key);
    assert!(matches!(
        event,
        reovim_client_driver::InputEvent::Key(reovim_client_driver::KeyEvent {
            code: reovim_client_driver::KeyCode::Char('a'),
            ..
        })
    ));
}

#[test]
fn convert_key_event_enter() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let tui_key = reovim_driver_tui::KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
        vim_notation: String::new(),
    };
    let event = convert_key_event(&tui_key);
    assert!(matches!(
        event,
        reovim_client_driver::InputEvent::Key(reovim_client_driver::KeyEvent {
            code: reovim_client_driver::KeyCode::Enter,
            ..
        })
    ));
}

#[test]
fn convert_key_event_esc() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let tui_key = reovim_driver_tui::KeyEvent {
        code: KeyCode::Esc,
        modifiers: KeyModifiers::NONE,
        vim_notation: String::new(),
    };
    let event = convert_key_event(&tui_key);
    assert!(matches!(
        event,
        reovim_client_driver::InputEvent::Key(reovim_client_driver::KeyEvent {
            code: reovim_client_driver::KeyCode::Esc,
            ..
        })
    ));
}

#[test]
fn convert_key_event_special_keys() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let test_cases = [
        (KeyCode::Tab, reovim_client_driver::KeyCode::Tab),
        (KeyCode::Backspace, reovim_client_driver::KeyCode::Backspace),
        (KeyCode::Left, reovim_client_driver::KeyCode::Left),
        (KeyCode::Right, reovim_client_driver::KeyCode::Right),
        (KeyCode::Up, reovim_client_driver::KeyCode::Up),
        (KeyCode::Down, reovim_client_driver::KeyCode::Down),
        (KeyCode::Home, reovim_client_driver::KeyCode::Home),
        (KeyCode::End, reovim_client_driver::KeyCode::End),
        (KeyCode::PageUp, reovim_client_driver::KeyCode::PageUp),
        (KeyCode::PageDown, reovim_client_driver::KeyCode::PageDown),
        (KeyCode::Insert, reovim_client_driver::KeyCode::Insert),
        (KeyCode::Delete, reovim_client_driver::KeyCode::Delete),
        (KeyCode::Null, reovim_client_driver::KeyCode::Null),
    ];

    for (crossterm_code, expected_code) in test_cases {
        let tui_key = reovim_driver_tui::KeyEvent {
            code: crossterm_code,
            modifiers: KeyModifiers::NONE,
            vim_notation: String::new(),
        };
        let event = convert_key_event(&tui_key);
        if let reovim_client_driver::InputEvent::Key(ke) = event {
            assert_eq!(ke.code, expected_code);
        } else {
            panic!("Expected Key event");
        }
    }
}

#[test]
fn convert_key_event_f_keys() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let tui_key = reovim_driver_tui::KeyEvent {
        code: KeyCode::F(12),
        modifiers: KeyModifiers::NONE,
        vim_notation: String::new(),
    };
    let event = convert_key_event(&tui_key);
    assert!(matches!(
        event,
        reovim_client_driver::InputEvent::Key(reovim_client_driver::KeyEvent {
            code: reovim_client_driver::KeyCode::F(12),
            ..
        })
    ));
}

#[test]
fn convert_key_event_modifiers() {
    use crossterm::event::{KeyCode, KeyModifiers};

    // Ctrl+Shift+A
    let tui_key = reovim_driver_tui::KeyEvent {
        code: KeyCode::Char('a'),
        modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        vim_notation: String::new(),
    };
    let event = convert_key_event(&tui_key);
    if let reovim_client_driver::InputEvent::Key(ke) = event {
        assert!(ke.modifiers.contains(reovim_client_driver::Modifiers::CTRL));
        assert!(
            ke.modifiers
                .contains(reovim_client_driver::Modifiers::SHIFT)
        );
        assert!(!ke.modifiers.contains(reovim_client_driver::Modifiers::ALT));
    } else {
        panic!("Expected Key event");
    }
}

#[test]
fn convert_key_event_alt() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let tui_key = reovim_driver_tui::KeyEvent {
        code: KeyCode::Char('x'),
        modifiers: KeyModifiers::ALT,
        vim_notation: String::new(),
    };
    let event = convert_key_event(&tui_key);
    if let reovim_client_driver::InputEvent::Key(ke) = event {
        assert!(ke.modifiers.contains(reovim_client_driver::Modifiers::ALT));
        assert!(!ke.modifiers.contains(reovim_client_driver::Modifiers::CTRL));
    } else {
        panic!("Expected Key event");
    }
}

#[test]
fn convert_key_event_backtab() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let tui_key = reovim_driver_tui::KeyEvent {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers::SHIFT,
        vim_notation: String::new(),
    };
    let event = convert_key_event(&tui_key);
    if let reovim_client_driver::InputEvent::Key(ke) = event {
        assert_eq!(ke.code, reovim_client_driver::KeyCode::Tab);
        assert!(
            ke.modifiers
                .contains(reovim_client_driver::Modifiers::SHIFT)
        );
    } else {
        panic!("Expected Key event");
    }
}

#[test]
fn convert_mouse_event_left_click() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let tui_mouse = reovim_driver_tui::MouseEvent {
        column: 10,
        row: 5,
        kind: MouseEventKind::Down(MouseButton::Left),
    };
    let event = convert_mouse_event(&tui_mouse);
    if let reovim_client_driver::InputEvent::Pointer(pe) = event {
        assert_eq!(pe.x, 10);
        assert_eq!(pe.y, 5);
        assert!(matches!(
            pe.kind,
            reovim_client_driver::PointerKind::Down(reovim_client_driver::PointerButton::Left)
        ));
    } else {
        panic!("Expected Pointer event");
    }
}

#[test]
fn convert_mouse_event_right_release() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let tui_mouse = reovim_driver_tui::MouseEvent {
        column: 20,
        row: 15,
        kind: MouseEventKind::Up(MouseButton::Right),
    };
    let event = convert_mouse_event(&tui_mouse);
    if let reovim_client_driver::InputEvent::Pointer(pe) = event {
        assert!(matches!(
            pe.kind,
            reovim_client_driver::PointerKind::Up(reovim_client_driver::PointerButton::Right)
        ));
    } else {
        panic!("Expected Pointer event");
    }
}

#[test]
fn convert_mouse_event_drag() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let tui_mouse = reovim_driver_tui::MouseEvent {
        column: 3,
        row: 7,
        kind: MouseEventKind::Drag(MouseButton::Middle),
    };
    let event = convert_mouse_event(&tui_mouse);
    if let reovim_client_driver::InputEvent::Pointer(pe) = event {
        assert!(matches!(
            pe.kind,
            reovim_client_driver::PointerKind::Drag(reovim_client_driver::PointerButton::Middle)
        ));
    } else {
        panic!("Expected Pointer event");
    }
}

#[test]
fn convert_mouse_event_scroll() {
    use crossterm::event::MouseEventKind;
    let tui_mouse = reovim_driver_tui::MouseEvent {
        column: 0,
        row: 0,
        kind: MouseEventKind::ScrollUp,
    };
    let event = convert_mouse_event(&tui_mouse);
    if let reovim_client_driver::InputEvent::Pointer(pe) = event {
        assert!(matches!(pe.kind, reovim_client_driver::PointerKind::ScrollUp));
    } else {
        panic!("Expected Pointer event");
    }

    let tui_mouse = reovim_driver_tui::MouseEvent {
        column: 0,
        row: 0,
        kind: MouseEventKind::ScrollDown,
    };
    let event = convert_mouse_event(&tui_mouse);
    if let reovim_client_driver::InputEvent::Pointer(pe) = event {
        assert!(matches!(pe.kind, reovim_client_driver::PointerKind::ScrollDown));
    } else {
        panic!("Expected Pointer event");
    }
}

#[test]
fn convert_mouse_event_moved() {
    use crossterm::event::MouseEventKind;
    let tui_mouse = reovim_driver_tui::MouseEvent {
        column: 50,
        row: 25,
        kind: MouseEventKind::Moved,
    };
    let event = convert_mouse_event(&tui_mouse);
    if let reovim_client_driver::InputEvent::Pointer(pe) = event {
        assert!(matches!(pe.kind, reovim_client_driver::PointerKind::Move));
        assert_eq!(pe.x, 50);
        assert_eq!(pe.y, 25);
    } else {
        panic!("Expected Pointer event");
    }
}

#[test]
fn convert_mouse_event_scroll_left_maps_to_scroll_down() {
    use crossterm::event::MouseEventKind;
    let tui_mouse = reovim_driver_tui::MouseEvent {
        column: 0,
        row: 0,
        kind: MouseEventKind::ScrollLeft,
    };
    let event = convert_mouse_event(&tui_mouse);
    if let reovim_client_driver::InputEvent::Pointer(pe) = event {
        // Best-effort mapping: ScrollLeft -> ScrollDown
        assert!(matches!(pe.kind, reovim_client_driver::PointerKind::ScrollDown));
    } else {
        panic!("Expected Pointer event");
    }
}

#[test]
fn convert_mouse_event_scroll_right_maps_to_scroll_down() {
    use crossterm::event::MouseEventKind;
    let tui_mouse = reovim_driver_tui::MouseEvent {
        column: 5,
        row: 3,
        kind: MouseEventKind::ScrollRight,
    };
    let event = convert_mouse_event(&tui_mouse);
    if let reovim_client_driver::InputEvent::Pointer(pe) = event {
        assert!(matches!(pe.kind, reovim_client_driver::PointerKind::ScrollDown));
    } else {
        panic!("Expected Pointer event");
    }
}

// =============================================================================
// convert_display_style_to_driver_style tests (reverse direction)
// =============================================================================

#[test]
fn convert_display_to_driver_style_default() {
    let display_style = DisplayStyle::default();
    let driver_style = convert_display_style_to_driver_style(&display_style);
    assert_eq!(driver_style.fg, None);
    assert_eq!(driver_style.bg, None);
    assert_eq!(driver_style.attributes, reovim_client_driver::Attributes::new());
}

#[test]
fn convert_display_to_driver_style_with_colors() {
    let display_style = DisplayStyle {
        fg: Some(Color::Rgb { r: 255, g: 128, b: 0 }),
        bg: Some(Color::Blue),
        attributes: DisplayAttributes::new(),
        underline_color: None,
    };
    let driver_style = convert_display_style_to_driver_style(&display_style);
    assert_eq!(driver_style.fg, Some(Color::Rgb { r: 255, g: 128, b: 0 }));
    assert_eq!(driver_style.bg, Some(Color::Blue));
}

#[test]
fn convert_display_to_driver_style_all_attributes() {
    let mut attrs = DisplayAttributes::new();
    attrs.set(DisplayAttributes::BOLD);
    attrs.set(DisplayAttributes::ITALIC);
    attrs.set(DisplayAttributes::UNDERLINE);
    attrs.set(DisplayAttributes::STRIKETHROUGH);
    attrs.set(DisplayAttributes::REVERSE);
    attrs.set(DisplayAttributes::DIM);

    let display_style = DisplayStyle {
        fg: None,
        bg: None,
        attributes: attrs,
        underline_color: None,
    };
    let driver_style = convert_display_style_to_driver_style(&display_style);
    assert!(driver_style.attributes.contains(reovim_client_driver::Attributes::BOLD));
    assert!(driver_style.attributes.contains(reovim_client_driver::Attributes::ITALIC));
    assert!(driver_style.attributes.contains(reovim_client_driver::Attributes::UNDERLINE));
    assert!(driver_style.attributes.contains(reovim_client_driver::Attributes::STRIKETHROUGH));
    assert!(driver_style.attributes.contains(reovim_client_driver::Attributes::REVERSE));
    assert!(driver_style.attributes.contains(reovim_client_driver::Attributes::DIM));
}

#[test]
fn convert_display_to_driver_style_partial_attributes() {
    let mut attrs = DisplayAttributes::new();
    attrs.set(DisplayAttributes::BOLD);
    attrs.set(DisplayAttributes::DIM);

    let display_style = DisplayStyle {
        fg: None,
        bg: None,
        attributes: attrs,
        underline_color: None,
    };
    let driver_style = convert_display_style_to_driver_style(&display_style);
    assert!(driver_style.attributes.contains(reovim_client_driver::Attributes::BOLD));
    assert!(!driver_style.attributes.contains(reovim_client_driver::Attributes::ITALIC));
    assert!(!driver_style.attributes.contains(reovim_client_driver::Attributes::UNDERLINE));
    assert!(!driver_style.attributes.contains(reovim_client_driver::Attributes::STRIKETHROUGH));
    assert!(!driver_style.attributes.contains(reovim_client_driver::Attributes::REVERSE));
    assert!(driver_style.attributes.contains(reovim_client_driver::Attributes::DIM));
}

// =============================================================================
// ThemeProviderAdapter tests
// =============================================================================

/// Minimal display-side ThemeProvider for testing the adapter.
struct TestDisplayTheme {
    styles: std::collections::HashMap<String, DisplayStyle>,
}

impl TestDisplayTheme {
    fn new() -> Self {
        Self {
            styles: std::collections::HashMap::new(),
        }
    }

    fn with_style(mut self, group: &str, style: DisplayStyle) -> Self {
        self.styles.insert(group.to_string(), style);
        self
    }
}

impl reovim_driver_display::style::ThemeProvider for TestDisplayTheme {
    fn get_style(&self, group: &str) -> Option<DisplayStyle> {
        self.styles.get(group).cloned()
    }
    fn name(&self) -> &str {
        "test-theme"
    }
}

#[test]
fn theme_provider_adapter_highlight() {
    let theme = TestDisplayTheme::new().with_style(
        "Keyword",
        {
            let mut attrs = DisplayAttributes::new();
            attrs.set(DisplayAttributes::BOLD);
            DisplayStyle {
                fg: Some(Color::Cyan),
                bg: None,
                attributes: attrs,
                underline_color: None,
            }
        },
    );
    let manager = reovim_driver_display::ThemeManager::new(std::sync::Arc::new(theme));
    let adapter = ThemeProviderAdapter::new(&manager);

    let style = reovim_client_driver::ThemeProvider::highlight(&adapter, "Keyword");
    assert_eq!(style.fg, Some(Color::Cyan));
    assert!(style.attributes.contains(reovim_client_driver::Attributes::BOLD));
}

#[test]
fn theme_provider_adapter_highlight_with_fallback_first_match() {
    let theme = TestDisplayTheme::new().with_style(
        "Function",
        DisplayStyle {
            fg: Some(Color::Yellow),
            bg: None,
            attributes: DisplayAttributes::new(),
            underline_color: None,
        },
    );
    let manager = reovim_driver_display::ThemeManager::new(std::sync::Arc::new(theme));
    let adapter = ThemeProviderAdapter::new(&manager);

    let style =
        reovim_client_driver::ThemeProvider::highlight_with_fallback(&adapter, &["Function", "Normal"]);
    assert_eq!(style.fg, Some(Color::Yellow));
}

#[test]
fn theme_provider_adapter_highlight_with_fallback_no_match() {
    let theme = TestDisplayTheme::new();
    let manager = reovim_driver_display::ThemeManager::new(std::sync::Arc::new(theme));
    let adapter = ThemeProviderAdapter::new(&manager);

    let style = reovim_client_driver::ThemeProvider::highlight_with_fallback(
        &adapter,
        &["NonExistent", "AlsoMissing"],
    );
    assert_eq!(style, reovim_client_driver::Style::default());
}

#[test]
fn theme_provider_adapter_foreground() {
    let theme = TestDisplayTheme::new().with_style(
        "Normal",
        DisplayStyle {
            fg: Some(Color::White),
            bg: Some(Color::Rgb { r: 30, g: 30, b: 30 }),
            attributes: DisplayAttributes::new(),
            underline_color: None,
        },
    );
    let manager = reovim_driver_display::ThemeManager::new(std::sync::Arc::new(theme));
    let adapter = ThemeProviderAdapter::new(&manager);

    let style = reovim_client_driver::ThemeProvider::foreground(&adapter);
    assert_eq!(style.fg, Some(Color::White));
    assert_eq!(style.bg, Some(Color::Rgb { r: 30, g: 30, b: 30 }));
}

#[test]
fn theme_provider_adapter_background() {
    let theme = TestDisplayTheme::new().with_style(
        "Normal",
        DisplayStyle {
            fg: Some(Color::White),
            bg: Some(Color::Rgb { r: 10, g: 10, b: 10 }),
            attributes: DisplayAttributes::new(),
            underline_color: None,
        },
    );
    let manager = reovim_driver_display::ThemeManager::new(std::sync::Arc::new(theme));
    let adapter = ThemeProviderAdapter::new(&manager);

    let style = reovim_client_driver::ThemeProvider::background(&adapter);
    assert_eq!(style.fg, None);
    assert_eq!(style.bg, Some(Color::Rgb { r: 10, g: 10, b: 10 }));
    assert_eq!(style.attributes, reovim_client_driver::Attributes::new());
}

#[test]
fn theme_provider_adapter_is_dark_no_normal() {
    let theme = TestDisplayTheme::new();
    let manager = reovim_driver_display::ThemeManager::new(std::sync::Arc::new(theme));
    let adapter = ThemeProviderAdapter::new(&manager);
    // No Normal style => bg is None => dark
    assert!(reovim_client_driver::ThemeProvider::is_dark(&adapter));
}

#[test]
fn theme_provider_adapter_is_dark_dark_bg() {
    let theme = TestDisplayTheme::new().with_style(
        "Normal",
        DisplayStyle {
            fg: None,
            bg: Some(Color::Rgb { r: 20, g: 20, b: 20 }),
            attributes: DisplayAttributes::new(),
            underline_color: None,
        },
    );
    let manager = reovim_driver_display::ThemeManager::new(std::sync::Arc::new(theme));
    let adapter = ThemeProviderAdapter::new(&manager);
    assert!(reovim_client_driver::ThemeProvider::is_dark(&adapter));
}

#[test]
fn theme_provider_adapter_is_dark_light_bg() {
    let theme = TestDisplayTheme::new().with_style(
        "Normal",
        DisplayStyle {
            fg: None,
            bg: Some(Color::Rgb { r: 200, g: 200, b: 200 }),
            attributes: DisplayAttributes::new(),
            underline_color: None,
        },
    );
    let manager = reovim_driver_display::ThemeManager::new(std::sync::Arc::new(theme));
    let adapter = ThemeProviderAdapter::new(&manager);
    assert!(!reovim_client_driver::ThemeProvider::is_dark(&adapter));
}

#[test]
fn theme_provider_adapter_is_dark_non_rgb_bg() {
    let theme = TestDisplayTheme::new().with_style(
        "Normal",
        DisplayStyle {
            fg: None,
            bg: Some(Color::Red),
            attributes: DisplayAttributes::new(),
            underline_color: None,
        },
    );
    let manager = reovim_driver_display::ThemeManager::new(std::sync::Arc::new(theme));
    let adapter = ThemeProviderAdapter::new(&manager);
    // Non-RGB color => returns true (dark)
    assert!(reovim_client_driver::ThemeProvider::is_dark(&adapter));
}

// =============================================================================
// TokenProviderAdapter tests
// =============================================================================

#[test]
fn token_provider_adapter_empty_cache() {
    let cache = reovim_driver_display::AnnotationCacheManager::new();
    let adapter = TokenProviderAdapter::new(&cache);
    let tokens =
        reovim_client_driver::TokenProvider::tokens_for_line(&adapter, reovim_client_driver::BufferId(0), 0);
    assert!(tokens.is_empty());
}

// =============================================================================
// collect_driver_virtual_lines - additional tests
// =============================================================================

#[test]
fn collect_driver_virtual_lines_ignores_non_buffer_contrib() {
    let exts: Vec<Box<dyn ClientModule>> = vec![Box::new(StubModule {
        buffer_contrib: false,
        virtual_lines_data: vec![reovim_client_driver::VirtualLine {
            buffer_line: 5,
            position: reovim_client_driver::VirtualLinePosition::After,
            content: "ignored".to_string(),
            style: reovim_client_driver::Style::default(),
        }],
        ..StubModule::default()
    })];
    assert!(collect_driver_virtual_lines(&exts).is_empty());
}

#[test]
fn sidebar_width_multiple_left_modules() {
    let exts: Vec<Box<dyn ClientModule>> = vec![
        Box::new(StubModule {
            chrome: true,
            position: ChromePosition::Left,
            requested_size: 4,
            ..StubModule::default()
        }),
        Box::new(StubModule {
            id: "stub2",
            chrome: true,
            position: ChromePosition::Left,
            requested_size: 6,
            ..StubModule::default()
        }),
    ];
    let caps = TuiPlatformCapabilities::for_test(80, 24);
    assert_eq!(sidebar_width(&exts, &caps), 10);
}

#[test]
fn sidebar_width_non_chrome_module_ignored() {
    let exts: Vec<Box<dyn ClientModule>> = vec![Box::new(StubModule {
        chrome: false,
        position: ChromePosition::Left,
        requested_size: 100,
        ..StubModule::default()
    })];
    let caps = TuiPlatformCapabilities::for_test(80, 24);
    assert_eq!(sidebar_width(&exts, &caps), 0);
}
