use super::*;

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_workspace_module_dir_ends_with_debug() {
    let dir = workspace_module_dir();
    // Under cargo-llvm-cov the target dir is target/llvm-cov-target/debug,
    // under normal cargo it's target/debug.
    assert!(
        dir.ends_with("debug"),
        "Expected path ending with debug, got: {}",
        dir.display()
    );
}

#[test]
fn test_workspace_module_dir_is_absolute() {
    let dir = workspace_module_dir();
    assert!(dir.is_absolute(), "Expected absolute path, got: {}", dir.display());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_binary_path_ends_with_reovim_server() {
    let path = binary_path();
    #[cfg(windows)]
    let expected = "reovim-server.exe";
    #[cfg(not(windows))]
    let expected = "reovim-server";
    assert!(
        path.file_name().is_some_and(|n| n == expected),
        "Expected path ending with {expected}, got: {}",
        path.display()
    );
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_binary_and_module_share_workspace_root() {
    // Both paths should be under the same workspace root.
    // Under cargo-llvm-cov, binary may fall back to target/debug/ while
    // modules resolve to target/llvm-cov-target/debug/ via current_exe().
    let binary = binary_path();
    let modules = workspace_module_dir();

    let find_workspace = |p: &Path| {
        p.ancestors()
            .find(|a| a.join("Cargo.toml").exists())
            .map(Path::to_path_buf)
    };

    let binary_ws = find_workspace(&binary);
    let module_ws = find_workspace(&modules);
    assert_eq!(
        binary_ws, module_ws,
        "Binary workspace ({binary_ws:?}) should equal module workspace ({module_ws:?})"
    );
}
