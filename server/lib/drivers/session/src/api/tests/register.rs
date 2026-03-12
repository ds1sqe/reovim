
use super::*;


#[test]
fn test_register_content_creation() {
    let char_content = RegisterContent::characterwise("hello");
    assert!(char_content.is_characterwise());
    assert!(!char_content.is_linewise());
    assert_eq!(char_content.text, "hello");

    let line_content = RegisterContent::linewise("world\n");
    assert!(line_content.is_linewise());
    assert!(!line_content.is_characterwise());
}

#[test]
fn test_yank_type_default() {
    let content = RegisterContent::default();
    assert!(content.is_characterwise());
    assert!(content.is_empty());
}