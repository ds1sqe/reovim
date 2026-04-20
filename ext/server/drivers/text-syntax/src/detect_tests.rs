use std::path::Path;

use super::*;

#[test]
fn detect_rust() {
    assert_eq!(language_id_from_path(Path::new("main.rs")), Some("rust"));
}

#[test]
fn detect_markdown() {
    assert_eq!(language_id_from_path(Path::new("README.md")), Some("markdown"));
    assert_eq!(language_id_from_path(Path::new("doc.markdown")), Some("markdown"));
}

#[test]
fn detect_python() {
    assert_eq!(language_id_from_path(Path::new("app.py")), Some("python"));
    assert_eq!(language_id_from_path(Path::new("stubs.pyi")), Some("python"));
}

#[test]
fn detect_go() {
    assert_eq!(language_id_from_path(Path::new("main.go")), Some("go"));
}

#[test]
fn detect_c() {
    assert_eq!(language_id_from_path(Path::new("foo.c")), Some("c"));
    assert_eq!(language_id_from_path(Path::new("foo.h")), Some("c"));
}

#[test]
fn detect_bash() {
    assert_eq!(language_id_from_path(Path::new("run.sh")), Some("bash"));
    assert_eq!(language_id_from_path(Path::new("script.bash")), Some("bash"));
}

#[test]
fn detect_json() {
    assert_eq!(language_id_from_path(Path::new("config.json")), Some("json"));
}

#[test]
fn detect_toml() {
    assert_eq!(language_id_from_path(Path::new("Cargo.toml")), Some("toml"));
}

#[test]
fn detect_javascript() {
    assert_eq!(language_id_from_path(Path::new("app.js")), Some("javascript"));
    assert_eq!(language_id_from_path(Path::new("module.mjs")), Some("javascript"));
    assert_eq!(language_id_from_path(Path::new("require.cjs")), Some("javascript"));
}

#[test]
fn detect_typescript() {
    assert_eq!(language_id_from_path(Path::new("app.ts")), Some("typescript"));
    assert_eq!(language_id_from_path(Path::new("module.mts")), Some("typescript"));
    assert_eq!(language_id_from_path(Path::new("require.cts")), Some("typescript"));
}

#[test]
fn detect_unknown() {
    assert_eq!(language_id_from_path(Path::new("image.png")), None);
}

#[test]
fn detect_no_extension() {
    assert_eq!(language_id_from_path(Path::new("Makefile")), None);
}
