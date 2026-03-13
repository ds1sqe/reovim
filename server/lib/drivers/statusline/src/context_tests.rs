use super::{ComponentDataContext, DiagnosticCounts};

#[test]
fn test_default() {
    let ctx = ComponentDataContext::default();
    assert!(ctx.mode.is_empty());
    assert!(ctx.filename.is_none());
    assert!(ctx.filepath.is_none());
    assert!(!ctx.modified);
    assert!(!ctx.readonly);
    assert_eq!(ctx.line, 0);
    assert_eq!(ctx.column, 0);
    assert_eq!(ctx.total_lines, 0);
    assert!(ctx.diagnostics.is_none());
}

#[test]
fn test_with_fields() {
    let ctx = ComponentDataContext {
        mode: "NORMAL".to_string(),
        filename: Some("main.rs".to_string()),
        filepath: Some("/home/user/project/src/main.rs".to_string()),
        modified: true,
        line: 42,
        column: 10,
        total_lines: 100,
        ..ComponentDataContext::default()
    };
    assert_eq!(ctx.mode, "NORMAL");
    assert_eq!(ctx.filename.as_deref(), Some("main.rs"));
    assert!(ctx.modified);
    assert_eq!(ctx.line, 42);
}

#[test]
fn test_with_diagnostics() {
    let ctx = ComponentDataContext {
        diagnostics: Some(DiagnosticCounts {
            errors: 2,
            warnings: 5,
            ..DiagnosticCounts::default()
        }),
        ..ComponentDataContext::default()
    };
    let diag = ctx.diagnostics.as_ref().unwrap();
    assert_eq!(diag.errors, 2);
    assert_eq!(diag.warnings, 5);
    assert_eq!(diag.total(), 7);
}

#[test]
fn test_clone() {
    let ctx = ComponentDataContext {
        mode: "INSERT".to_string(),
        git_branch: Some("main".to_string()),
        breadcrumb: Some("fn main > impl Foo".to_string()),
        ..ComponentDataContext::default()
    };
    #[allow(clippy::redundant_clone)]
    let cloned = ctx.clone();
    assert_eq!(cloned.mode, "INSERT");
    assert_eq!(cloned.git_branch.as_deref(), Some("main"));
    assert_eq!(cloned.breadcrumb.as_deref(), Some("fn main > impl Foo"));
}

#[test]
fn test_debug() {
    let ctx = ComponentDataContext::default();
    let debug = format!("{ctx:?}");
    assert!(debug.contains("ComponentDataContext"));
}

#[test]
fn test_terminal_dimensions() {
    let ctx = ComponentDataContext {
        terminal_width: 120,
        terminal_height: 40,
        ..ComponentDataContext::default()
    };
    assert_eq!(ctx.terminal_width, 120);
    assert_eq!(ctx.terminal_height, 40);
}

#[test]
fn test_encoding_fields() {
    let ctx = ComponentDataContext {
        encoding: "utf-8".to_string(),
        line_ending: "unix".to_string(),
        ..ComponentDataContext::default()
    };
    assert_eq!(ctx.encoding, "utf-8");
    assert_eq!(ctx.line_ending, "unix");
}

#[test]
fn test_mode_subtype() {
    let ctx = ComponentDataContext {
        mode: "VISUAL".to_string(),
        mode_subtype: Some("LINE".to_string()),
        ..ComponentDataContext::default()
    };
    assert_eq!(ctx.mode_subtype.as_deref(), Some("LINE"));
}

#[test]
fn test_filetype() {
    let ctx = ComponentDataContext {
        filetype: Some("rust".to_string()),
        ..ComponentDataContext::default()
    };
    assert_eq!(ctx.filetype.as_deref(), Some("rust"));
}
