use super::*;

#[test]
fn library_open_display_renders_path_and_source() {
    let err = LoaderError::LibraryOpen {
        path: PathBuf::from("/nope.so"),
        source_text: "no such file".to_owned(),
    };
    let rendered = err.to_string();
    assert!(rendered.contains("/nope.so"), "path missing: {rendered}");
    assert!(rendered.contains("no such file"), "source text missing: {rendered}");
}

#[test]
fn symbol_not_found_display_renders_symbol_and_source() {
    let err = LoaderError::SymbolNotFound {
        symbol: "missing".to_owned(),
        source_text: "undefined symbol".to_owned(),
    };
    let rendered = err.to_string();
    assert!(rendered.contains("missing"), "symbol missing: {rendered}");
    assert!(rendered.contains("undefined symbol"), "source missing: {rendered}");
}

#[test]
fn loader_error_implements_std_error() {
    fn assert_std_error<E: std::error::Error>(_e: &E) {}
    let err = LoaderError::LibraryOpen {
        path: PathBuf::from("/x"),
        source_text: "y".to_owned(),
    };
    assert_std_error(&err);
}
