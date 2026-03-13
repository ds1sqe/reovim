use {super::*, std::path::Path};

#[test]
fn test_error_display_library_load() {
    let err = ExtensionLoadError::LibraryLoad("not found".to_string());
    assert!(err.to_string().contains("library load error"));
    assert!(err.to_string().contains("not found"));
}

#[test]
fn test_error_display_symbol_not_found() {
    let err = ExtensionLoadError::SymbolNotFound("reovim_extension_entry".to_string());
    assert!(err.to_string().contains("symbol not found"));
}

#[test]
fn test_error_display_invalid_kind() {
    let err = ExtensionLoadError::InvalidKind("null pointer".to_string());
    assert!(err.to_string().contains("kind error"));
}

#[test]
fn test_error_display_null_entry() {
    let err = ExtensionLoadError::NullEntry;
    assert!(err.to_string().contains("null"));
}

#[test]
fn test_error_source() {
    let err = ExtensionLoadError::LibraryLoad("test".to_string());
    assert!(std::error::Error::source(&err).is_none());
}

#[test]
fn test_probe_nonexistent_library() {
    let result = probe_extension_kind(Path::new("/nonexistent/libext.so"));
    assert!(result.is_err());
}

#[test]
fn test_load_nonexistent_library() {
    let result = load_extension(Path::new("/nonexistent/libext.so"));
    assert!(result.is_err());
}

#[test]
fn test_error_debug() {
    let err = ExtensionLoadError::NullEntry;
    let debug = format!("{err:?}");
    assert!(debug.contains("NullEntry"));
}
