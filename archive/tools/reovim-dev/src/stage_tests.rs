use super::*;

#[test]
fn classify_reovim_driver_prefix_routes_to_driver() {
    assert!(matches!(classify("libreovim_driver_abi_poc"), Some(Kind::Driver)));
    assert!(matches!(classify("reovim_driver_text_ffi"), Some(Kind::Driver)));
}

#[test]
fn classify_reovim_module_prefix_routes_to_module() {
    assert!(matches!(classify("libreovim_module_vim"), Some(Kind::Module)));
    assert!(matches!(classify("libreovim_client_module_landing"), Some(Kind::Module)));
}

#[test]
fn classify_unknown_prefix_returns_none() {
    assert!(classify("libsomething_else").is_none());
    assert!(classify("libstd").is_none());
}

#[test]
fn stage_all_errors_when_target_debug_missing() {
    let tmp = tempfile::tempdir().unwrap();
    // no `target/debug/` underneath
    let err = stage_all(tmp.path()).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
}

#[test]
fn stage_all_skips_non_reovim_cdylibs_and_non_cdylib_files() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("target").join("debug");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join(format!("libfoo.{}", library_extension())), []).unwrap();
    fs::write(target.join("README.md"), b"skip me").unwrap();

    let report = stage_all(tmp.path()).unwrap();
    assert_eq!(report.staged_count(), 0);
    // staging dirs still created
    assert!(report.staging_root().join("driver").is_dir());
    assert!(report.staging_root().join("modules").is_dir());
}

#[test]
fn stage_all_is_idempotent_over_symlink_recreation() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("target").join("debug");
    fs::create_dir_all(&target).unwrap();
    let cdylib = target.join(format!("libreovim_driver_fake.{}", library_extension()));
    fs::write(&cdylib, b"fake cdylib bytes").unwrap();

    let first = stage_all(tmp.path()).unwrap();
    assert_eq!(first.staged_count(), 1);
    // Second run must succeed without complaining about the
    // existing symlink.
    let second = stage_all(tmp.path()).unwrap();
    assert_eq!(second.staged_count(), 1);
}

#[test]
fn scan_staging_on_empty_tree_returns_empty_json() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("target").join("reovim-dev").join("driver")).unwrap();
    fs::create_dir_all(tmp.path().join("target").join("reovim-dev").join("modules")).unwrap();
    let result = scan_staging(tmp.path());
    assert!(result.driver.is_empty());
    assert!(result.modules.is_empty());
}
