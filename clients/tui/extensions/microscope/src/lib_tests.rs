use {reovim_arch::Color, reovim_driver_display::Style};

use super::*;

struct MockBackend {
    width: u16,
    height: u16,
}

#[allow(clippy::cast_possible_truncation)]
impl RenderBackend for MockBackend {
    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    fn set_cell(&mut self, _x: u16, _y: u16, _ch: char, _style: &Style) {}

    fn apply_style(&mut self, _x: u16, _y: u16, _style: &Style) {}

    fn write_str(&mut self, x: u16, _y: u16, text: &str, _style: &Style) -> u16 {
        text.len().min((self.width.saturating_sub(x)) as usize) as u16
    }

    fn clear(&mut self) {}

    fn overlay_bg(&mut self, _x: u16, _y: u16, _bg: Color) {}
}

#[test]
fn extension_kind() {
    let ext = MicroscopeExtension::new();
    assert_eq!(ext.kind(), "microscope");
}

#[test]
fn initially_inactive() {
    let ext = MicroscopeExtension::new();
    assert!(!ext.is_active());
}

#[test]
fn default_impl() {
    let ext = MicroscopeExtension::default();
    assert!(!ext.is_active());
}

#[test]
fn apply_notification_active() {
    let mut ext = MicroscopeExtension::new();
    ext.apply_notification(
        r#"{"active":true,"query":"main","cursor":4,"selected":0,"scrollOffset":0,"pickerName":"files","pickerTitle":"Files","prompt":"> ","items":[{"display":"main.rs"}],"totalCount":10,"matchedCount":1}"#,
    );
    assert!(ext.is_active());
    assert_eq!(ext.data.query, "main");
    assert_eq!(ext.data.cursor, 4);
    assert_eq!(ext.data.picker_title, "Files");
    assert_eq!(ext.data.items.len(), 1);
    assert_eq!(ext.data.total_count, 10);
}

#[test]
fn apply_notification_inactive() {
    let mut ext = MicroscopeExtension::new();
    ext.apply_notification(r#"{"active":true,"query":"x","cursor":1,"selected":0,"scrollOffset":0,"pickerName":"f","pickerTitle":"F","prompt":"> ","items":[],"totalCount":0,"matchedCount":0}"#);
    assert!(ext.is_active());

    ext.apply_notification(r#"{"active":false}"#);
    assert!(!ext.is_active());
}

#[test]
fn apply_notification_invalid_json() {
    let mut ext = MicroscopeExtension::new();
    ext.apply_notification("not json");
    assert!(!ext.is_active());
}

#[test]
fn apply_notification_with_preview() {
    let mut ext = MicroscopeExtension::new();
    ext.apply_notification(
        r#"{"active":true,"query":"","cursor":0,"selected":0,"scrollOffset":0,"pickerName":"f","pickerTitle":"F","prompt":"> ","items":[],"totalCount":0,"matchedCount":0,"preview":{"lines":["fn main()","{}"],"highlightLine":0}}"#,
    );
    assert!(ext.data.preview.is_some());
    let preview = ext.data.preview.as_ref().unwrap();
    assert_eq!(preview.lines.len(), 2);
    assert_eq!(preview.highlight_line, Some(0));
}

#[test]
fn apply_notification_with_item_detail() {
    let mut ext = MicroscopeExtension::new();
    ext.apply_notification(
        r#"{"active":true,"query":"","cursor":0,"selected":0,"scrollOffset":0,"pickerName":"f","pickerTitle":"F","prompt":"> ","items":[{"display":"main.rs","detail":"src/main.rs"}],"totalCount":1,"matchedCount":1}"#,
    );
    assert_eq!(ext.data.items.len(), 1);
    assert_eq!(ext.data.items[0].detail.as_deref(), Some("src/main.rs"));
}

#[test]
fn cursor_position_inactive() {
    let ext = MicroscopeExtension::new();
    assert!(ext.cursor_position(80, 24).is_none());
}

#[test]
fn cursor_position_active() {
    let mut ext = MicroscopeExtension::new();
    ext.apply_notification(
        r#"{"active":true,"query":"ab","cursor":2,"selected":0,"scrollOffset":0,"pickerName":"f","pickerTitle":"F","prompt":"> ","items":[],"totalCount":0,"matchedCount":0}"#,
    );
    let pos = ext.cursor_position(80, 24);
    assert!(pos.is_some());
}

#[test]
fn cursor_position_too_small_screen() {
    let mut ext = MicroscopeExtension::new();
    ext.apply_notification(
        r#"{"active":true,"query":"","cursor":0,"selected":0,"scrollOffset":0,"pickerName":"f","pickerTitle":"F","prompt":"> ","items":[],"totalCount":0,"matchedCount":0}"#,
    );
    // Very small screen.
    let pos = ext.cursor_position(10, 3);
    assert!(pos.is_none());
}

#[test]
fn render_active() {
    let mut ext = MicroscopeExtension::new();
    ext.apply_notification(
        r#"{"active":true,"query":"test","cursor":4,"selected":0,"scrollOffset":0,"pickerName":"files","pickerTitle":"Files","prompt":"> ","items":[{"display":"main.rs"}],"totalCount":10,"matchedCount":1}"#,
    );
    let mut backend = MockBackend {
        width: 80,
        height: 24,
    };
    ext.render(&mut backend);
}

#[test]
fn render_too_small_screen() {
    let mut ext = MicroscopeExtension::new();
    ext.apply_notification(
        r#"{"active":true,"query":"","cursor":0,"selected":0,"scrollOffset":0,"pickerName":"f","pickerTitle":"F","prompt":"> ","items":[],"totalCount":0,"matchedCount":0}"#,
    );
    let mut backend = MockBackend {
        width: 10,
        height: 3,
    };
    // Should return early without panicking.
    ext.render(&mut backend);
}
