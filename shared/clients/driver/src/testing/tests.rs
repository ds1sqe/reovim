use std::collections::HashMap;

use crate::{
    ColorDepth, OptionValue, PlatformCapabilities, Rect, RenderSurface, RenderingModel,
    ServerHandle, Style, ThemeProvider, traits::ClientModuleRegistry, types::Color,
};

use super::*;

// =============================================================================
// RecordingSurface
// =============================================================================

#[test]
fn recording_surface_write_records_chars() {
    let mut s = RecordingSurface::new(10, 1);
    s.write_styled(0, 0, "hello", Style::default());
    assert_eq!(s.char_at(0, 0), 'h');
    assert_eq!(s.char_at(4, 0), 'o');
    assert_eq!(s.char_at(5, 0), ' ');
}

#[test]
fn recording_surface_write_records_style() {
    let mut s = RecordingSurface::new(10, 1);
    let style = Style::new().fg(Color::Red).bold();
    s.write_styled(0, 0, "hi", style.clone());
    assert_eq!(*s.style_at(0, 0), style);
    assert_eq!(*s.style_at(1, 0), style);
    assert_eq!(*s.style_at(2, 0), Style::new()); // unwritten
}

#[test]
fn recording_surface_fill() {
    let mut s = RecordingSurface::new(5, 3);
    s.fill(Rect::new(1, 1, 3, 2), '#', Style::new().bg(Color::Blue));
    assert_eq!(s.char_at(0, 0), ' ');
    assert_eq!(s.char_at(1, 1), '#');
    assert_eq!(s.char_at(3, 2), '#');
    assert_eq!(s.char_at(4, 2), ' ');
}

#[test]
fn recording_surface_clear() {
    let mut s = RecordingSurface::new(5, 1);
    s.write_styled(0, 0, "hello", Style::default());
    s.clear(Rect::new(0, 0, 5, 1));
    assert_eq!(s.char_at(0, 0), ' ');
    assert_eq!(s.char_at(4, 0), ' ');
}

#[test]
fn recording_surface_apply_style() {
    let mut s = RecordingSurface::new(5, 1);
    s.write_styled(0, 0, "A", Style::default());
    s.apply_style(0, 0, Style::new().fg(Color::Green));
    assert_eq!(s.char_at(0, 0), 'A');
    assert_eq!(s.style_at(0, 0).fg, Some(Color::Green));
}

#[test]
fn recording_surface_overlay_bg() {
    let mut s = RecordingSurface::new(5, 1);
    s.write_styled(0, 0, "X", Style::default());
    s.overlay_bg(0, 0, Color::Yellow);
    assert_eq!(s.style_at(0, 0).bg, Some(Color::Yellow));
}

#[test]
fn recording_surface_has_content() {
    let s = RecordingSurface::new(5, 1);
    assert!(!s.has_content());
    let mut s2 = RecordingSurface::new(5, 1);
    s2.write_styled(0, 0, "x", Style::default());
    assert!(s2.has_content());
}

#[test]
fn recording_surface_snapshot() {
    let mut s = RecordingSurface::new(5, 2);
    s.write_styled(0, 0, "abc", Style::default());
    s.write_styled(0, 1, "de", Style::default());
    let snap = s.snapshot();
    assert_eq!(snap, "abc\nde");
}

#[test]
fn recording_surface_assert_text_at_pass() {
    let mut s = RecordingSurface::new(10, 1);
    s.write_styled(2, 0, "hi", Style::default());
    s.assert_text_at(2, 0, "hi");
}

#[test]
#[should_panic(expected = "text mismatch")]
fn recording_surface_assert_text_at_fail() {
    let s = RecordingSurface::new(10, 1);
    s.assert_text_at(0, 0, "nope");
}

#[test]
fn recording_surface_out_of_bounds_write() {
    let mut s = RecordingSurface::new(3, 1);
    let cols = s.write_styled(0, 5, "oob", Style::default());
    assert_eq!(cols, 0);
}

#[test]
fn recording_surface_size() {
    let s = RecordingSurface::new(40, 12);
    assert_eq!(s.size(), (40, 12));
}

// =============================================================================
// WriteSurface
// =============================================================================

#[test]
fn write_surface_records() {
    let mut s = WriteSurface::new(80, 24);
    s.write_styled(5, 3, "hello", Style::default());
    assert_eq!(s.writes().len(), 1);
    assert_eq!(s.writes()[0].text, "hello");
}

#[test]
fn write_surface_text_at() {
    let mut s = WriteSurface::new(80, 24);
    s.write_styled(0, 0, "first", Style::default());
    s.write_styled(0, 0, "second", Style::new().bold());
    assert_eq!(s.text_at(0, 0), Some("second"));
}

#[test]
fn write_surface_style_at() {
    let mut s = WriteSurface::new(80, 24);
    let style = Style::new().fg(Color::Red);
    s.write_styled(0, 0, "x", style.clone());
    assert_eq!(s.style_at(0, 0), Some(&style));
}

#[test]
fn write_surface_missing() {
    let s = WriteSurface::new(80, 24);
    assert!(s.text_at(0, 0).is_none());
    assert!(s.style_at(0, 0).is_none());
}

#[test]
fn write_surface_size() {
    let s = WriteSurface::new(120, 40);
    assert_eq!(s.size(), (120, 40));
}

// =============================================================================
// MockPlatformCapabilities
// =============================================================================

#[test]
fn mock_caps_defaults() {
    let caps = MockPlatformCapabilities::new();
    assert_eq!(caps.rendering_model(), RenderingModel::CellGrid);
    assert_eq!(caps.grid_size(), Some((80, 24)));
    assert_eq!(caps.color_depth(), ColorDepth::TrueColor);
    assert!(caps.dark_mode());
    assert!(caps.has_focus());
}

#[test]
fn mock_caps_builder() {
    let caps = MockPlatformCapabilities::new()
        .with_grid_size(120, 40)
        .with_color_depth(ColorDepth::Ansi256)
        .with_dark_mode(false)
        .with_focus(false);
    assert_eq!(caps.grid_size(), Some((120, 40)));
    assert_eq!(caps.color_depth(), ColorDepth::Ansi256);
    assert!(!caps.dark_mode());
    assert!(!caps.has_focus());
}

// =============================================================================
// MockServerHandle
// =============================================================================

#[test]
fn mock_server_returns_options() {
    let mut opts = HashMap::new();
    opts.insert("number".to_string(), OptionValue::Bool(true));
    let server = MockServerHandle::new().with_options(opts);
    let result = server.get_options(&["number", "missing"]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], ("number".to_string(), OptionValue::Bool(true)));
}

#[test]
fn mock_server_records_commands() {
    let server = MockServerHandle::new();
    server.execute_command("echo hello");
    server.execute_command("quit");
    assert_eq!(server.executed_commands(), vec!["echo hello", "quit"]);
}

#[test]
fn mock_server_defaults() {
    let server = MockServerHandle::new();
    assert!(server.get_options(&["anything"]).is_empty());
    assert!(server.executed_commands().is_empty());
    assert!(server.list_commands().is_empty());
    assert!(server.get_option_metadata("x").is_none());
}

// =============================================================================
// MockThemeProvider
// =============================================================================

#[test]
fn mock_theme_default() {
    let theme = MockThemeProvider::new();
    assert_eq!(theme.highlight("Normal"), Style::default());
    assert!(theme.is_dark());
    assert_eq!(theme.foreground().fg, Some(Color::White));
    assert_eq!(theme.background().bg, Some(Color::Black));
}

#[test]
fn mock_theme_seeded() {
    let style = Style::new().fg(Color::Red);
    let theme = MockThemeProvider::new().with_highlight("Error", style.clone());
    assert_eq!(theme.highlight("Error"), style);
    assert_eq!(theme.highlight("Normal"), Style::default());
}

#[test]
fn mock_theme_fallback() {
    let style = Style::new().fg(Color::Blue);
    let theme = MockThemeProvider::new().with_highlight("CursorLine", style.clone());
    assert_eq!(theme.highlight_with_fallback(&["Missing", "CursorLine"]), style);
    assert_eq!(theme.highlight_with_fallback(&["Missing", "AlsoMissing"]), Style::default());
}

// =============================================================================
// MockModuleRegistry
// =============================================================================

#[test]
fn mock_registry_empty() {
    let reg = MockModuleRegistry::new();
    assert!(!reg.is_running("anything"));
    assert!(reg.module_state("anything").is_none());
    assert!(reg.loaded_kinds().is_empty());
    assert_eq!(reg.running_count(), 0);
}

#[test]
fn mock_registry_with_running() {
    let reg = MockModuleRegistry::new().with_running(&["statusline", "whichkey"]);
    assert!(reg.is_running("statusline"));
    assert!(reg.is_running("whichkey"));
    assert!(!reg.is_running("missing"));
    assert_eq!(reg.running_count(), 2);
}

// =============================================================================
// TestModuleContext
// =============================================================================

#[test]
fn test_context_default() {
    let test_ctx = TestModuleContext::builder().build();
    let ctx = test_ctx.as_context();
    assert_eq!(ctx.capabilities.grid_size(), Some((80, 24)));
    assert!(ctx.theme.is_dark());
    assert!(ctx.services.is_none());
    assert!(ctx.module_registry.is_none());
}

#[test]
fn test_context_with_services() {
    let services = crate::services::ClientServiceRegistry::new();
    let test_ctx = TestModuleContext::builder().services(services).build();
    let ctx = test_ctx.as_context();
    assert!(ctx.services.is_some());
}

#[test]
fn test_context_with_running_modules() {
    let test_ctx = TestModuleContext::builder()
        .running_modules(&["lsp", "statusline"])
        .build();
    let ctx = test_ctx.as_context();
    assert!(ctx.module_registry.unwrap().is_running("lsp"));
}

#[test]
fn test_context_configured() {
    let test_ctx = TestModuleContext::builder()
        .grid_size(120, 40)
        .dark_mode(false)
        .highlight("Error", Style::new().fg(Color::Red))
        .build();
    let ctx = test_ctx.as_context();
    assert_eq!(ctx.capabilities.grid_size(), Some((120, 40)));
    assert!(!ctx.capabilities.dark_mode());
    assert_eq!(ctx.theme.highlight("Error").fg, Some(Color::Red));
}

#[test]
fn test_context_accessors() {
    let test_ctx = TestModuleContext::builder().build();
    assert_eq!(test_ctx.capabilities().grid_size, Some((80, 24)));
    assert!(test_ctx.server().executed_commands().is_empty());
    assert!(test_ctx.theme().is_dark());
}
