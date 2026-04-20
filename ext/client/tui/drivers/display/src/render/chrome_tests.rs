use super::*;

#[test]
fn test_tab_info_new() {
    let tab = TabInfo::new("test.rs");
    assert_eq!(tab.name, "test.rs");
    assert!(!tab.modified);
    assert!(tab.path.is_none());
}

#[test]
fn test_tab_info_builder() {
    let tab = TabInfo::new("test.rs")
        .modified(true)
        .path("/home/user/test.rs");

    assert!(tab.modified);
    assert_eq!(tab.path, Some("/home/user/test.rs".to_string()));
}

#[test]
fn test_tab_info_display_text() {
    let tab = TabInfo::new("test.rs");
    assert_eq!(tab.display_text(), " test.rs ");

    let modified_tab = TabInfo::new("test.rs").modified(true);
    assert_eq!(modified_tab.display_text(), " test.rs [+] ");
}

#[test]
fn test_render_tabline() {
    let mut buffer = FrameBuffer::new(80, 24);
    let tabs = vec![
        TabInfo::new("file1.rs"),
        TabInfo::new("file2.rs").modified(true),
    ];

    let default = Style::default();
    let styles = TablineStyles::new(&default, &default, &default);
    render_tabline(&mut buffer, &tabs, 0, 0, 80, &styles);

    // Check first tab content
    assert_eq!(buffer.get(1, 0).unwrap().char, 'f');
    assert_eq!(buffer.get(2, 0).unwrap().char, 'i');
}

#[test]
fn test_render_tabline_empty() {
    let mut buffer = FrameBuffer::new(80, 24);

    let default = Style::default();
    let styles = TablineStyles::new(&default, &default, &default);
    render_tabline(&mut buffer, &[], 0, 0, 80, &styles);

    // Should be filled with spaces
    assert_eq!(buffer.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_render_statusline() {
    let mut buffer = FrameBuffer::new(80, 24);

    render_statusline(&mut buffer, 23, "NORMAL", "file.rs", "1:1", 80, &Style::default());

    // Check left section
    assert_eq!(buffer.get(0, 23).unwrap().char, 'N');

    // Check right section (at end)
    assert_eq!(buffer.get(77, 23).unwrap().char, '1');
    assert_eq!(buffer.get(78, 23).unwrap().char, ':');
    assert_eq!(buffer.get(79, 23).unwrap().char, '1');
}

#[test]
fn test_render_statusline_simple() {
    let mut buffer = FrameBuffer::new(80, 24);

    render_statusline_simple(&mut buffer, 23, "LEFT", "RIGHT", 80, &Style::default());

    assert_eq!(buffer.get(0, 23).unwrap().char, 'L');
    assert_eq!(buffer.get(75, 23).unwrap().char, 'R');
}

#[test]
fn test_render_tabline_overflow_tab_start() {
    // Width is very small so that a tab's x position starts beyond width
    let mut buffer = FrameBuffer::new(5, 2);
    let tabs = vec![TabInfo::new("longname1.rs"), TabInfo::new("longname2.rs")];

    // First tab " longname1.rs " is 14 chars wide, so second tab
    // starts at x=14 which is >= width(5). The loop should break at line 82.
    let default = Style::default();
    let styles = TablineStyles::new(&default, &default, &default);
    render_tabline(&mut buffer, &tabs, 0, 0, 5, &styles);

    // First tab partially rendered, second tab not rendered at all
    assert_eq!(buffer.get(1, 0).unwrap().char, 'l');
}

#[test]
fn test_render_tabline_overflow_mid_tab() {
    // Width cuts off in the middle of a tab's characters
    let mut buffer = FrameBuffer::new(4, 2);
    let tabs = vec![TabInfo::new("abcdefgh")];

    // Tab text is " abcdefgh " (10 chars). Width is 4, so chars after x=3
    // should trigger the inner break at line 94.
    let default = Style::default();
    let styles = TablineStyles::new(&default, &default, &default);
    render_tabline(&mut buffer, &tabs, 0, 0, 4, &styles);

    // Only first 4 characters of " abcdefgh " should be rendered
    assert_eq!(buffer.get(0, 0).unwrap().char, ' ');
    assert_eq!(buffer.get(1, 0).unwrap().char, 'a');
    assert_eq!(buffer.get(2, 0).unwrap().char, 'b');
    assert_eq!(buffer.get(3, 0).unwrap().char, 'c');
}

#[test]
fn test_render_statusline_left_overflow() {
    // Left text is long enough to be truncated by center/right position
    let mut buffer = FrameBuffer::new(10, 2);

    // With width=10, right "RIGHTRIGHT" starts at x=0, center "" at x=5
    // Left text "LEFTLEFTLEFT" should break when reaching center_x/right_x
    render_statusline(&mut buffer, 0, "LEFTLEFTLEFT", "", "RRRRRRRRRR", 10, &Style::default());

    // Left should be fully truncated since right fills the entire width
    // Right section starts at x=0 (10 - 10 = 0)
    assert_eq!(buffer.get(0, 0).unwrap().char, 'R');
}

#[test]
fn test_render_statusline_center_hits_right_not_width() {
    // Center text reaches right_x (first condition true) but not width (second condition false)
    // Line 160: `if x >= right_x || x >= width` - exercise first true, second false
    let mut buffer = FrameBuffer::new(30, 2);

    // right "RR" at x=28, center text "CCCCCCCCCCCC" (12 chars) starts at (30-12)/2=9
    // Center rendering should stop at x=28 (right_x), not x=30 (width)
    render_statusline(&mut buffer, 0, "", "CCCCCCCCCCCC", "RR", 30, &Style::default());

    // Center chars should appear from x=9
    assert_eq!(buffer.get(9, 0).unwrap().char, 'C');
    // Right section should still appear at x=28
    assert_eq!(buffer.get(28, 0).unwrap().char, 'R');
    assert_eq!(buffer.get(29, 0).unwrap().char, 'R');
}

#[test]
fn test_render_statusline_center_overflow() {
    // Center text overflows into right section area
    let mut buffer = FrameBuffer::new(20, 2);

    // right "RR" at x=18, center "CCCCCCCCCCCCCCCCCCCC" (20 chars) at ~x=0
    // The center loop should break when x >= right_x (line 161)
    render_statusline(&mut buffer, 0, "", "CCCCCCCCCCCCCCCCCCCC", "RR", 20, &Style::default());

    // Right section should still appear at x=18
    assert_eq!(buffer.get(18, 0).unwrap().char, 'R');
    assert_eq!(buffer.get(19, 0).unwrap().char, 'R');
}

#[test]
fn test_render_statusline_right_overflow() {
    // Right text is wider than the buffer
    let mut buffer = FrameBuffer::new(5, 2);

    // right "RIGHTTEXT" (9 chars), width=5. right_x = 5-9 = 0 (saturating).
    // Right loop should break at x >= width (line 171)
    render_statusline(&mut buffer, 0, "", "", "RIGHTTEXT", 5, &Style::default());

    // Only first 5 chars of right text should render
    assert_eq!(buffer.get(0, 0).unwrap().char, 'R');
    assert_eq!(buffer.get(4, 0).unwrap().char, 'T');
}
