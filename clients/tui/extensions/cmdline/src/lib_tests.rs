use reovim_driver_display::FrameBuffer;

use super::*;

#[test]
fn test_new_is_inactive() {
    let ext = CmdlineExtension::new();
    assert!(!ext.is_active());
    assert_eq!(ext.kind(), "cmdline");
}

#[test]
fn test_default_is_inactive() {
    let ext = CmdlineExtension::default();
    assert!(!ext.is_active());
}

#[test]
fn test_apply_notification_activates() {
    let mut ext = CmdlineExtension::new();
    let data = r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#;
    ext.apply_notification(data);

    assert!(ext.is_active());
    assert_eq!(ext.prompt, ":");
    assert_eq!(ext.input, "wq");
    assert_eq!(ext.cursor, 2);
}

#[test]
fn test_apply_notification_deactivates() {
    let mut ext = CmdlineExtension::new();
    ext.apply_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);
    assert!(ext.is_active());

    ext.apply_notification(r#"{"active":false}"#);
    assert!(!ext.is_active());
}

#[test]
fn test_apply_notification_invalid_json() {
    let mut ext = CmdlineExtension::new();
    ext.apply_notification("not json");
    assert!(!ext.is_active());
}

#[test]
fn test_apply_notification_search_prompt() {
    let mut ext = CmdlineExtension::new();
    ext.apply_notification(r#"{"active":true,"prompt":"/","input":"hello","cursor":5}"#);
    assert_eq!(ext.prompt, "/");
    assert_eq!(ext.input, "hello");
}

#[test]
fn test_apply_notification_with_completions() {
    let mut ext = CmdlineExtension::new();
    ext.apply_notification(
        r#"{"active":true,"prompt":":","input":"w","cursor":1,"completions":["write","wq","wall"],"completion_index":0}"#,
    );
    assert_eq!(ext.completions.len(), 3);
    assert_eq!(ext.completions[0], "write");
    assert_eq!(ext.completion_index, Some(0));
}

#[test]
fn test_apply_notification_client_id_zero_is_local() {
    // client_id check is done by the engine, not the extension.
    // Extension just parses data. This tests that parsing works.
    let mut ext = CmdlineExtension::new();
    ext.apply_notification(r#"{"active":true,"prompt":":","input":"q","cursor":1}"#);
    assert!(ext.is_active());
}

#[test]
fn test_render_shows_popup() {
    let mut ext = CmdlineExtension::new();
    ext.apply_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Check border at py=1
    let pw = popup_width(80);
    let px = popup_x(80, pw);
    assert_eq!(fb.get(px, 1).unwrap().char, '\u{256D}'); // top-left corner
    assert_eq!(fb.get(px + pw - 1, 1).unwrap().char, '\u{256E}'); // top-right corner

    // Check prompt character
    let content_x = px + 2;
    assert_eq!(fb.get(content_x, 2).unwrap().char, ':');
}

#[test]
fn test_render_cursor_in_middle() {
    let mut ext = CmdlineExtension::new();
    ext.apply_notification(r#"{"active":true,"prompt":":","input":"hello","cursor":2}"#);

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Cursor should be at position 2 in the input (on 'l')
    let pw = popup_width(80);
    let px = popup_x(80, pw);
    let content_x = px + 2;
    let cursor_x = content_x + 1 + 2; // prompt_len(1) + cursor(2)

    // The cell should have cursor style (white bg)
    let cell = fb.get(cursor_x, 2).unwrap();
    assert_eq!(cell.char, 'l'); // character preserved
    assert_eq!(cell.style.bg, Some(Color::White)); // cursor bg
}

#[test]
fn test_render_with_completions() {
    let mut ext = CmdlineExtension::new();
    ext.apply_notification(
        r#"{"active":true,"prompt":":","input":"w","cursor":1,"completions":["write","wq"],"completion_index":0}"#,
    );

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Check that the popup height includes completion rows
    let pw = popup_width(80);
    let px = popup_x(80, pw);
    // Height = 3 (border+content+border) + 2 completions = 5
    // Bottom border should be at y=1+4=5
    assert_eq!(fb.get(px, 5).unwrap().char, '\u{2570}'); // bottom-left

    // Selected item should have indicator
    let content_x = px + 2;
    assert_eq!(fb.get(content_x, 3).unwrap().char, '\u{25B8}'); // selected indicator
}

#[test]
fn test_render_narrow_terminal() {
    let mut ext = CmdlineExtension::new();
    ext.apply_notification(r#"{"active":true,"prompt":":","input":"test","cursor":4}"#);

    // Narrow terminal (32 wide)
    let mut fb = FrameBuffer::new(32, 24);
    ext.render(&mut fb);

    // Should still render without panic
    let pw = popup_width(32);
    let px = popup_x(32, pw);
    assert_eq!(fb.get(px, 1).unwrap().char, '\u{256D}');
}

#[test]
fn test_cursor_position_when_active() {
    let mut ext = CmdlineExtension::new();
    ext.apply_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);

    let pos = ext.cursor_position(80, 24);
    assert!(pos.is_some());
    let (x, y) = pos.unwrap();
    // y should be 2 (popup at y=1, content at y=2)
    assert_eq!(y, 2);
    // x = popup_x + 2 (border) + 1 (prompt ":") + 2 (cursor pos)
    let pw = popup_width(80);
    let px = popup_x(80, pw);
    assert_eq!(x, px + 2 + 1 + 2);
}

#[test]
fn test_cursor_position_when_inactive() {
    let ext = CmdlineExtension::new();
    assert!(ext.cursor_position(80, 24).is_none());
}

#[test]
fn test_render_overflow_narrow_popup() {
    let mut ext = CmdlineExtension::new();
    // Long input + completions that exceed popup content width
    ext.apply_notification(
        r#"{"active":true,"prompt":":","input":"this_is_a_very_long_command_that_overflows","cursor":42,"completions":["this_is_a_very_long_completion_candidate"],"completion_index":0}"#,
    );

    // Very narrow terminal — content overflows trigger break branches
    let mut fb = FrameBuffer::new(16, 24);
    ext.render(&mut fb);
    // Should render without panic — overflow branches hit
}

#[test]
fn test_trait_object() {
    let ext: Box<dyn TuiExtension> = Box::new(CmdlineExtension::new());
    assert_eq!(ext.kind(), "cmdline");
    assert!(!ext.is_active());
}
