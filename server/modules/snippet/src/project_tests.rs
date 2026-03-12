use super::*;

#[test]
fn test_find_project_root_with_cargo_toml() {
    // This crate has a Cargo.toml, so we should find it.
    let this_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/project.rs");
    let root = find_project_root(&this_file);
    assert!(root.is_some());
    let root = root.unwrap();
    assert!(root.join("Cargo.toml").exists());
}

#[test]
fn test_find_project_root_from_directory() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let root = find_project_root(&dir);
    assert!(root.is_some());
}

#[test]
fn test_find_project_root_not_found() {
    // Root "/" typically has no project markers.
    let root = find_project_root(Path::new("/nonexistent/path/file.rs"));
    assert!(root.is_none());
}

#[test]
fn test_project_snippet_dir_returns_reovim_snippets() {
    let this_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/project.rs");
    let dir = project_snippet_dir(&this_file);
    assert!(dir.is_some());
    let dir = dir.unwrap();
    assert!(dir.ends_with(".reovim/snippets"));
}

#[test]
fn test_project_snippet_dir_not_found() {
    let dir = project_snippet_dir(Path::new("/nonexistent/path/file.rs"));
    assert!(dir.is_none());
}
