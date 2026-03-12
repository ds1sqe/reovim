use reovim_driver_display::FrameBuffer;

use super::*;

// =========================================================================
// Helpers
// =========================================================================

fn active_payload() -> String {
    r#"{"active":true,"diagnostics":[{"bufferId":1,"items":[{"startLine":5,"startCol":2,"endLine":5,"endCol":8,"severity":"error","message":"type mismatch","source":"rust-analyzer"}]}]}"#.to_owned()
}

fn multi_buffer_payload() -> String {
    r#"{"active":true,"diagnostics":[{"bufferId":1,"items":[{"startLine":3,"startCol":0,"endLine":3,"endCol":5,"severity":"warning","message":"unused variable","source":"rustc"}]},{"bufferId":2,"items":[{"startLine":10,"startCol":1,"endLine":10,"endCol":4,"severity":"hint","message":"consider using let","source":null}]}]}"#.to_owned()
}

fn inactive_payload() -> String {
    r#"{"active":false}"#.to_owned()
}

// =========================================================================
// Basic trait tests
// =========================================================================

#[test]
fn new_is_inactive() {
    let ext = DiagnosticsExtension::new();
    assert!(!ext.is_active());
    assert_eq!(ext.kind(), "diagnostics");
}

#[test]
fn default_is_inactive() {
    let ext = DiagnosticsExtension::default();
    assert!(!ext.is_active());
}

#[test]
fn trait_object() {
    let ext: Box<dyn TuiExtension> = Box::new(DiagnosticsExtension::new());
    assert_eq!(ext.kind(), "diagnostics");
    assert!(!ext.is_active());
}

// =========================================================================
// apply_notification tests
// =========================================================================

#[test]
fn apply_activates() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification(&active_payload());
    assert!(ext.is_active());
    assert_eq!(ext.buffers.len(), 1);
    assert_eq!(ext.buffers[0].buffer_id, 1);
    assert_eq!(ext.buffers[0].items.len(), 1);
    assert_eq!(ext.buffers[0].items[0].severity, "error");
}

#[test]
fn apply_deactivates() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification(&active_payload());
    assert!(ext.is_active());

    ext.apply_notification(&inactive_payload());
    assert!(!ext.is_active());
    assert!(ext.buffers.is_empty());
}

#[test]
fn apply_invalid_json() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification("not json{{{");
    assert!(!ext.is_active());
}

#[test]
fn apply_multi_buffer() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification(&multi_buffer_payload());
    assert!(ext.is_active());
    assert_eq!(ext.buffers.len(), 2);
    assert_eq!(ext.buffers[0].buffer_id, 1);
    assert_eq!(ext.buffers[1].buffer_id, 2);
}

#[test]
fn apply_overwrites_previous() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification(&active_payload());
    assert_eq!(ext.buffers.len(), 1);

    ext.apply_notification(&multi_buffer_payload());
    assert_eq!(ext.buffers.len(), 2);
}

// =========================================================================
// render tests
// =========================================================================

#[test]
fn render_no_op_without_viewport() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification(&active_payload());

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);
    // render() is a no-op, should not modify framebuffer
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn render_with_viewport_no_buffer_id() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification(&active_payload());

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 4,
        content_height: 24,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(80, 24);
    ext.render_with_viewport(&mut fb, &viewport);
    // No buffer_id → no rendering
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn render_with_viewport_wrong_buffer() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification(&active_payload());

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 4,
        content_height: 24,
        buffer_id: Some(99), // no diagnostics for buffer 99
    };
    let mut fb = FrameBuffer::new(80, 24);
    ext.render_with_viewport(&mut fb, &viewport);
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn render_with_viewport_shows_virtual_text() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification(&active_payload());

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 4,
        content_height: 24,
        buffer_id: Some(1),
    };
    let mut fb = FrameBuffer::new(80, 24);
    ext.render_with_viewport(&mut fb, &viewport);

    // Diagnostic at line 5 → screen row 5 (scroll_top=0).
    // Virtual text should appear somewhere on that row.
    // Check that the row has red-colored text (error severity).
    let mut found_colored = false;
    for x in 0..80 {
        if let Some(cell) = fb.get(x, 5)
            && cell.style.fg == Some(Color::Red)
        {
            found_colored = true;
            break;
        }
    }
    assert!(found_colored, "Expected red virtual text on diagnostic line");
}

#[test]
fn render_with_viewport_applies_underline_style() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification(&active_payload());

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 4,
        content_height: 24,
        buffer_id: Some(1),
    };
    let mut fb = FrameBuffer::new(80, 24);
    ext.render_with_viewport(&mut fb, &viewport);

    // Diagnostic spans col 2..8 → screen x = content_x + col = 4+2=6 to 4+8=12.
    // apply_style sets fg on those cells.
    let cell = fb.get(6, 5).unwrap();
    assert_eq!(cell.style.fg, Some(Color::Red));
}

#[test]
fn render_with_viewport_scrolled_past() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification(&active_payload());

    // Diagnostic at line 5, but viewport starts at line 10 → not visible.
    let viewport = ViewportContext {
        scroll_top: 10,
        content_x: 4,
        content_height: 20,
        buffer_id: Some(1),
    };
    let mut fb = FrameBuffer::new(80, 24);
    ext.render_with_viewport(&mut fb, &viewport);

    // Nothing should be rendered.
    let mut any_colored = false;
    for y in 0..24 {
        for x in 0..80 {
            if let Some(cell) = fb.get(x, y)
                && cell.style.fg == Some(Color::Red)
            {
                any_colored = true;
            }
        }
    }
    assert!(!any_colored, "No diagnostic rendering expected when scrolled past");
}

#[test]
fn render_with_viewport_multi_buffer_filters() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification(&multi_buffer_payload());

    // Focus on buffer 2 — should show hint (green), not warning (yellow).
    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 4,
        content_height: 24,
        buffer_id: Some(2),
    };
    let mut fb = FrameBuffer::new(80, 24);
    ext.render_with_viewport(&mut fb, &viewport);

    // Diagnostic at line 10, severity "hint" → green.
    let mut found_green = false;
    for x in 0..80 {
        if let Some(cell) = fb.get(x, 10)
            && cell.style.fg == Some(Color::Green)
        {
            found_green = true;
            break;
        }
    }
    assert!(found_green, "Expected green virtual text for hint diagnostic");
}

// =========================================================================
// Helper function tests
// =========================================================================

#[test]
fn test_severity_color_all_variants() {
    assert_eq!(severity_color("error"), Color::Red);
    assert_eq!(severity_color("warning"), Color::Yellow);
    assert_eq!(severity_color("information"), Color::Cyan);
    assert_eq!(severity_color("hint"), Color::Green);
    assert_eq!(severity_color("unknown"), Color::Grey);
}

#[test]
fn test_severity_prefix_all_variants() {
    assert_eq!(severity_prefix("error"), "E");
    assert_eq!(severity_prefix("warning"), "W");
    assert_eq!(severity_prefix("information"), "I");
    assert_eq!(severity_prefix("hint"), "H");
    assert_eq!(severity_prefix("unknown"), "?");
}

// =========================================================================
// Deserialization coverage
// =========================================================================

#[test]
fn payload_debug() {
    let p = DiagnosticPayload {
        active: true,
        diagnostics: vec![],
    };
    assert!(format!("{p:?}").contains("DiagnosticPayload"));
}

#[test]
fn buffer_diagnostics_debug_clone() {
    let b = BufferDiagnostics {
        buffer_id: 1,
        items: vec![],
    };
    let debug = format!("{b:?}");
    assert!(debug.contains("BufferDiagnostics"));
    #[allow(clippy::redundant_clone)]
    let c = b.clone();
    assert_eq!(c.buffer_id, 1);
}

#[test]
fn item_payload_debug_clone() {
    let item = DiagnosticItemPayload {
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 5,
        severity: "error".to_owned(),
        message: "test".to_owned(),
        source: Some("test".to_owned()),
    };
    let debug = format!("{item:?}");
    assert!(debug.contains("DiagnosticItemPayload"));
    #[allow(clippy::redundant_clone)]
    let c = item.clone();
    assert_eq!(c.message, "test");
}

#[test]
fn item_payload_no_source() {
    let item = DiagnosticItemPayload {
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 1,
        severity: "hint".to_owned(),
        message: "hint".to_owned(),
        source: None,
    };
    assert!(item.source.is_none());
}

#[test]
fn render_narrow_terminal_no_panic() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification(&active_payload());

    // Narrow terminal: content_x + 40 = 44 > width=30. Virtual text skipped.
    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 4,
        content_height: 24,
        buffer_id: Some(1),
    };
    let mut fb = FrameBuffer::new(30, 24);
    ext.render_with_viewport(&mut fb, &viewport);
    // Should not panic. Underline may still render if within width.
}

#[test]
fn render_zero_size_terminal_no_panic() {
    let mut ext = DiagnosticsExtension::new();
    ext.apply_notification(&active_payload());

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 0,
        content_height: 0,
        buffer_id: Some(1),
    };
    let mut fb = FrameBuffer::new(0, 0);
    ext.render_with_viewport(&mut fb, &viewport);
    // Should not panic on zero-size terminal.
}

#[test]
fn render_multiline_diagnostic_no_underline() {
    let mut ext = DiagnosticsExtension::new();
    // Diagnostic spanning lines 3-5 (multi-line).
    ext.apply_notification(
        r#"{"active":true,"diagnostics":[{"bufferId":1,"items":[{"startLine":3,"startCol":0,"endLine":5,"endCol":10,"severity":"error","message":"multi-line error","source":null}]}]}"#,
    );

    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 4,
        content_height: 24,
        buffer_id: Some(1),
    };
    let mut fb = FrameBuffer::new(120, 24);
    ext.render_with_viewport(&mut fb, &viewport);

    // Multi-line diagnostic: no underline at start_col, but virtual text at line 3.
    // Check that line 3 has red-colored virtual text.
    let mut found = false;
    for x in 0..120 {
        if let Some(cell) = fb.get(x, 3)
            && cell.style.fg == Some(Color::Red)
        {
            found = true;
            break;
        }
    }
    assert!(found, "Expected virtual text on multi-line diagnostic start line");
}

#[test]
fn apply_active_false_with_diagnostics() {
    let mut ext = DiagnosticsExtension::new();
    // Even if diagnostics are present, active=false should clear them.
    ext.apply_notification(r#"{"active":false,"diagnostics":[{"bufferId":1,"items":[]}]}"#);
    assert!(!ext.is_active());
    assert!(ext.buffers.is_empty());
}
