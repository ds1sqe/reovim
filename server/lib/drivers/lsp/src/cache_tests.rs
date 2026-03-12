use lsp_types::{DiagnosticSeverity, Position, Range};

use super::*;

fn make_uri(path: &str) -> Uri {
    path.parse().expect("test URI should parse")
}

fn make_diagnostic(message: &str, severity: DiagnosticSeverity) -> Diagnostic {
    Diagnostic {
        range: Range::new(Position::new(0, 0), Position::new(0, 10)),
        severity: Some(severity),
        message: message.to_string(),
        ..Default::default()
    }
}

#[test]
fn test_store_and_get() {
    let cache = DiagnosticCache::new();
    let uri = make_uri("file:///test.rs");
    let diagnostics = vec![make_diagnostic("test error", DiagnosticSeverity::ERROR)];

    cache.store(&uri, Some(1), diagnostics);

    let result = cache.get(&uri).unwrap();
    assert_eq!(result.version, Some(1));
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].message, "test error");
}

#[test]
fn test_get_nonexistent() {
    let cache = DiagnosticCache::new();
    let uri = make_uri("file:///nonexistent.rs");

    assert!(cache.get(&uri).is_none());
}

#[test]
fn test_remove() {
    let cache = DiagnosticCache::new();
    let uri = make_uri("file:///test.rs");
    let diagnostics = vec![make_diagnostic("error", DiagnosticSeverity::ERROR)];

    cache.store(&uri, None, diagnostics);
    assert!(cache.has(&uri));

    cache.remove(&uri);
    assert!(!cache.has(&uri));
}

#[test]
fn test_clear() {
    let cache = DiagnosticCache::new();

    for i in 0..5 {
        let uri = make_uri(&format!("file:///test{i}.rs"));
        cache.store(&uri, None, vec![]);
    }

    assert_eq!(cache.len(), 5);

    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn test_update_replaces() {
    let cache = DiagnosticCache::new();
    let uri = make_uri("file:///test.rs");

    cache.store(&uri, Some(1), vec![make_diagnostic("old", DiagnosticSeverity::ERROR)]);

    cache.store(&uri, Some(2), vec![make_diagnostic("new", DiagnosticSeverity::WARNING)]);

    let result = cache.get(&uri).unwrap();
    assert_eq!(result.version, Some(2));
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].message, "new");
}

#[test]
fn test_get_all() {
    let cache = DiagnosticCache::new();

    let uri1 = make_uri("file:///a.rs");
    let uri2 = make_uri("file:///b.rs");

    cache.store(&uri1, None, vec![]);
    cache.store(&uri2, None, vec![]);

    let all = cache.get_all();
    assert_eq!(all.len(), 2);
    assert!(all.contains_key(uri1.as_str()));
    assert!(all.contains_key(uri2.as_str()));
}

#[test]
fn test_last_updated_initially_none() {
    let cache = DiagnosticCache::new();
    assert!(cache.last_updated().is_none());
}

#[test]
fn test_last_updated_after_store() {
    let cache = DiagnosticCache::new();
    let uri = make_uri("file:///test.rs");
    cache.store(&uri, None, vec![]);
    assert!(cache.last_updated().is_some());
}

#[test]
fn test_default_impl() {
    let cache = DiagnosticCache::default();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
    assert!(cache.last_updated().is_none());
}

#[test]
fn test_debug_impl() {
    let cache = DiagnosticCache::new();
    let debug = format!("{cache:?}");
    assert!(debug.contains("DiagnosticCache"));
}

#[test]
fn test_has() {
    let cache = DiagnosticCache::new();
    let uri = make_uri("file:///test.rs");
    assert!(!cache.has(&uri));
    cache.store(&uri, None, vec![]);
    assert!(cache.has(&uri));
}

#[test]
fn test_store_with_none_version() {
    let cache = DiagnosticCache::new();
    let uri = make_uri("file:///test.rs");
    cache.store(&uri, None, vec![]);
    let result = cache.get(&uri).unwrap();
    assert_eq!(result.version, None);
}

#[test]
fn test_remove_preserves_last_updated() {
    let cache = DiagnosticCache::new();
    let uri1 = make_uri("file:///a.rs");
    let uri2 = make_uri("file:///b.rs");
    cache.store(&uri1, None, vec![]);
    cache.store(&uri2, None, vec![]);

    let before = cache.last_updated();
    cache.remove(&uri1);
    let after = cache.last_updated();
    // last_updated should be preserved after remove
    assert_eq!(before, after);
}

#[test]
fn test_clear_resets_last_updated() {
    let cache = DiagnosticCache::new();
    let uri = make_uri("file:///test.rs");
    cache.store(&uri, None, vec![]);
    assert!(cache.last_updated().is_some());
    cache.clear();
    assert!(cache.last_updated().is_none());
}

#[test]
fn test_buffer_diagnostics_default() {
    let diag = BufferDiagnostics::default();
    assert!(diag.version.is_none());
    assert!(diag.diagnostics.is_empty());
}

#[test]
fn test_buffer_diagnostics_clone() {
    let diag = BufferDiagnostics {
        version: Some(5),
        diagnostics: vec![make_diagnostic("test", DiagnosticSeverity::ERROR)],
    };
    #[allow(clippy::redundant_clone)]
    let cloned = diag.clone();
    assert_eq!(cloned.version, Some(5));
    assert_eq!(cloned.diagnostics.len(), 1);
}
