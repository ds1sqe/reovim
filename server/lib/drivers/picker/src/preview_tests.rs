use super::*;

#[test]
fn preview_default() {
    let preview = PreviewContent::default();
    assert!(preview.lines.is_empty());
    assert!(preview.highlight_line.is_none());
    assert!(preview.file_path.is_none());
}

#[test]
fn preview_construction() {
    let preview = PreviewContent {
        lines: vec!["fn main() {".to_owned(), "}".to_owned()],
        highlight_line: Some(0),
        file_path: Some(PathBuf::from("main.rs")),
    };
    assert_eq!(preview.lines.len(), 2);
    assert_eq!(preview.highlight_line, Some(0));
    assert_eq!(preview.file_path.as_deref(), Some(std::path::Path::new("main.rs")));
}

#[test]
fn preview_clone() {
    let preview = PreviewContent {
        lines: vec!["line 1".to_owned()],
        highlight_line: Some(0),
        file_path: None,
    };
    let cloned = preview.clone();
    assert_eq!(cloned.lines, preview.lines);
    assert_eq!(cloned.highlight_line, preview.highlight_line);
}

#[test]
fn preview_debug() {
    let preview = PreviewContent::default();
    let debug = format!("{preview:?}");
    assert!(debug.contains("PreviewContent"));
}
