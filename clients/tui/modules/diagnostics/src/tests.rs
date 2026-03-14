use reovim_client_driver::{BufferId, ClientModule, VirtualLinePosition};

use super::*;

fn active_payload() -> String {
    r#"{"active":true,"diagnostics":[{"bufferId":1,"items":[{"startLine":5,"startCol":2,"endLine":5,"endCol":8,"severity":"error","message":"type mismatch","source":"rust-analyzer"}]}]}"#.to_owned()
}

fn multi_buffer_payload() -> String {
    r#"{"active":true,"diagnostics":[{"bufferId":1,"items":[{"startLine":3,"startCol":0,"endLine":3,"endCol":5,"severity":"warning","message":"unused variable","source":"rustc"}]},{"bufferId":2,"items":[{"startLine":10,"startCol":1,"endLine":10,"endCol":4,"severity":"hint","message":"consider using let","source":null}]}]}"#.to_owned()
}

// =========================================================================
// Construction and identity
// =========================================================================

#[test]
fn new_inactive() {
    let m = DiagnosticsModule::new();
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.id(), "diagnostics");
    assert_eq!(m.kind(), "diagnostics");
    assert_eq!(m.name(), "Diagnostics");
}

#[test]
fn default_inactive() {
    let m = DiagnosticsModule::default();
    assert!(!m.has_buffer_contrib());
}

#[test]
fn version() {
    let m = DiagnosticsModule::new();
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

// =========================================================================
// on_notification
// =========================================================================

#[test]
fn notification_activates() {
    let mut m = DiagnosticsModule::new();
    m.on_buffer_focus(BufferId(1));
    m.on_notification(&active_payload());
    assert!(m.has_buffer_contrib());
    assert_eq!(m.buffers.len(), 1);
}

#[test]
fn notification_deactivates() {
    let mut m = DiagnosticsModule::new();
    m.on_buffer_focus(BufferId(1));
    m.on_notification(&active_payload());
    assert!(m.has_buffer_contrib());

    m.on_notification(r#"{"active":false}"#);
    assert!(!m.has_buffer_contrib());
    assert!(m.buffers.is_empty());
}

#[test]
fn notification_invalid_json() {
    let mut m = DiagnosticsModule::new();
    m.on_notification("not json{{{");
    assert!(!m.has_buffer_contrib());
}

#[test]
fn notification_multi_buffer() {
    let mut m = DiagnosticsModule::new();
    m.on_notification(&multi_buffer_payload());
    assert!(m.has_buffer_contrib());
    assert_eq!(m.buffers.len(), 2);
}

#[test]
fn notification_overwrites_previous() {
    let mut m = DiagnosticsModule::new();
    m.on_notification(&active_payload());
    assert_eq!(m.buffers.len(), 1);

    m.on_notification(&multi_buffer_payload());
    assert_eq!(m.buffers.len(), 2);
}

#[test]
fn notification_active_false_with_diagnostics() {
    let mut m = DiagnosticsModule::new();
    m.on_notification(r#"{"active":false,"diagnostics":[{"bufferId":1,"items":[]}]}"#);
    assert!(!m.has_buffer_contrib());
    assert!(m.buffers.is_empty());
}

// =========================================================================
// inline_decorations
// =========================================================================

#[test]
fn decorations_single_line_diagnostic() {
    let mut m = DiagnosticsModule::new();
    m.on_buffer_focus(BufferId(1));
    m.on_notification(&active_payload());

    let decs = m.inline_decorations(5);
    assert_eq!(decs.len(), 1);
    assert_eq!(decs[0].col_start, 2);
    assert_eq!(decs[0].col_end, 8);
    assert_eq!(decs[0].style.fg, Some(Color::Red));
}

#[test]
fn decorations_empty_for_other_line() {
    let mut m = DiagnosticsModule::new();
    m.on_buffer_focus(BufferId(1));
    m.on_notification(&active_payload());

    assert!(m.inline_decorations(0).is_empty());
    assert!(m.inline_decorations(99).is_empty());
}

#[test]
fn decorations_multiline_no_underline() {
    let mut m = DiagnosticsModule::new();
    m.on_buffer_focus(BufferId(1));
    m.on_notification(
        r#"{"active":true,"diagnostics":[{"bufferId":1,"items":[{"startLine":3,"startCol":0,"endLine":5,"endCol":10,"severity":"error","message":"multi-line","source":null}]}]}"#,
    );

    // Multi-line diagnostic: no underline decorations
    assert!(m.inline_decorations(3).is_empty());
    assert!(m.inline_decorations(5).is_empty());
}

#[test]
fn decorations_inactive() {
    let m = DiagnosticsModule::new();
    assert!(m.inline_decorations(0).is_empty());
}

#[test]
fn decorations_no_active_buffer() {
    let mut m = DiagnosticsModule::new();
    // Don't call on_buffer_focus — no active buffer
    m.on_notification(&active_payload());

    // Active but no buffer focused → no decorations
    assert!(m.inline_decorations(5).is_empty());
}

#[test]
fn decorations_wrong_buffer() {
    let mut m = DiagnosticsModule::new();
    m.on_buffer_focus(BufferId(99));
    m.on_notification(&active_payload());

    // Buffer 99 has no diagnostics
    assert!(m.inline_decorations(5).is_empty());
}

#[test]
fn decorations_buffer_focus_rebuilds() {
    let mut m = DiagnosticsModule::new();
    m.on_notification(&multi_buffer_payload());

    // Focus buffer 1 — should get warning decorations at line 3
    m.on_buffer_focus(BufferId(1));
    let decs = m.inline_decorations(3);
    assert_eq!(decs.len(), 1);
    assert_eq!(decs[0].style.fg, Some(Color::Yellow));

    // Switch focus to buffer 2 — should get hint decorations at line 10
    m.on_buffer_focus(BufferId(2));
    assert!(m.inline_decorations(3).is_empty());
    let decs2 = m.inline_decorations(10);
    assert_eq!(decs2.len(), 1);
    assert_eq!(decs2[0].style.fg, Some(Color::Green));
}

// =========================================================================
// virtual_lines
// =========================================================================

#[test]
fn virtual_lines_for_diagnostic() {
    let mut m = DiagnosticsModule::new();
    m.on_buffer_focus(BufferId(1));
    m.on_notification(&active_payload());

    let vlines = m.virtual_lines();
    assert_eq!(vlines.len(), 1);
    assert_eq!(vlines[0].buffer_line, 5);
    assert_eq!(vlines[0].position, VirtualLinePosition::After);
    assert!(vlines[0].content.contains("E:"));
    assert!(vlines[0].content.contains("type mismatch"));
    assert_eq!(vlines[0].style.fg, Some(Color::Red));
}

#[test]
fn virtual_lines_empty_when_inactive() {
    let m = DiagnosticsModule::new();
    assert!(m.virtual_lines().is_empty());
}

#[test]
fn virtual_lines_multiline_diagnostic() {
    let mut m = DiagnosticsModule::new();
    m.on_buffer_focus(BufferId(1));
    m.on_notification(
        r#"{"active":true,"diagnostics":[{"bufferId":1,"items":[{"startLine":3,"startCol":0,"endLine":5,"endCol":10,"severity":"warning","message":"spans lines","source":null}]}]}"#,
    );

    let vlines = m.virtual_lines();
    assert_eq!(vlines.len(), 1);
    assert_eq!(vlines[0].buffer_line, 3);
    assert!(vlines[0].content.contains("W:"));
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

// =========================================================================
// Lifecycle defaults
// =========================================================================

#[test]
fn tick_returns_false() {
    let mut m = DiagnosticsModule::new();
    assert!(!m.tick());
}

#[test]
fn cursor_position_none() {
    let m = DiagnosticsModule::new();
    assert!(m.cursor_position(80, 24).is_none());
}

#[test]
fn has_annotations_false() {
    let m = DiagnosticsModule::new();
    assert!(!m.has_annotations());
}

#[test]
fn has_chrome_false() {
    let m = DiagnosticsModule::new();
    assert!(!m.has_chrome());
}
