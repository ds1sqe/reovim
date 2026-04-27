use super::*;

#[test]
fn classify_open_error_routes_permission_denied_to_io() {
    let out = classify_open_error(
        Path::new("/x"),
        "libloading: cannot open: Permission denied".to_owned(),
    );
    assert!(matches!(out, ScanEntryError::Io { .. }));
}

#[test]
fn classify_open_error_routes_not_a_shared_object_to_not_a_library() {
    let out = classify_open_error(Path::new("/x"), "libloading: not a shared object".to_owned());
    assert!(matches!(out, ScanEntryError::NotALibrary { .. }));
}

#[test]
fn classify_open_error_routes_file_too_short_to_not_a_library() {
    let out = classify_open_error(Path::new("/x"), "libloading: file too short".to_owned());
    assert!(matches!(out, ScanEntryError::NotALibrary { .. }));
}

#[test]
fn classify_open_error_routes_unrecognized_text_to_dlopen_failed() {
    // Text that matches neither permission-denied nor format
    // failure patterns — in the wild this would be a missing
    // `DT_NEEDED` dep or a similar runtime rejection.
    let out =
        classify_open_error(Path::new("/x"), "libloading: undefined reference to `foo`".to_owned());
    assert!(matches!(
        &out,
        ScanEntryError::DlopenFailed { source_text, .. } if source_text.contains("undefined reference")
    ));
}

#[test]
fn scan_report_into_entries_drains_the_vec() {
    // Direct unit exercise of `into_entries`; end-to-end callers
    // (driver-loader's `from_path_scan`) use it too but their
    // coverage counts in a different crate's report.
    let report = scan_paths(&[std::path::PathBuf::from("/does-not-exist")]);
    let entries = report.into_entries();
    assert!(entries.is_empty());
}
