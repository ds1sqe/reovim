use super::*;

fn make_entry(id: u64, name: &str) -> BufferEntrySnapshot {
    BufferEntrySnapshot {
        id,
        name: name.to_string(),
        path: None,
        modified: false,
        filetype: None,
    }
}

// ============================================================================
// BufferEntrySnapshot
// ============================================================================

#[test]
fn entry_snapshot_clone_eq() {
    let entry = BufferEntrySnapshot {
        id: 1,
        name: String::from("main.rs"),
        path: Some(String::from("/src/main.rs")),
        modified: false,
        filetype: Some(String::from("rust")),
    };
    let cloned = entry.clone();
    assert_eq!(entry, cloned);
}

#[test]
fn entry_snapshot_debug() {
    let entry = make_entry(1, "test");
    let debug = format!("{entry:?}");
    assert!(debug.contains("test"));
}

// ============================================================================
// BufferListService
// ============================================================================

#[test]
fn service_default_empty() {
    let svc = BufferListService::default();
    assert!(svc.snapshot().is_empty());
}

#[test]
fn service_update_and_snapshot() {
    let svc = BufferListService::default();
    let entries = vec![make_entry(1, "a.rs"), make_entry(2, "b.rs")];
    svc.update(entries.clone());
    assert_eq!(svc.snapshot(), entries);
}

#[test]
fn service_update_replaces() {
    let svc = BufferListService::default();
    svc.update(vec![make_entry(1, "a.rs")]);
    svc.update(vec![make_entry(2, "b.rs")]);
    let snap = svc.snapshot();
    assert_eq!(snap.len(), 1);
    assert_eq!(snap[0].id, 2);
}

#[test]
fn service_add() {
    let svc = BufferListService::default();
    svc.add(make_entry(1, "a.rs"));
    svc.add(make_entry(2, "b.rs"));
    assert_eq!(svc.snapshot().len(), 2);
}

#[test]
fn service_remove() {
    let svc = BufferListService::default();
    svc.add(make_entry(1, "a.rs"));
    svc.add(make_entry(2, "b.rs"));
    svc.remove(1);
    let snap = svc.snapshot();
    assert_eq!(snap.len(), 1);
    assert_eq!(snap[0].id, 2);
}

#[test]
fn service_remove_nonexistent() {
    let svc = BufferListService::default();
    svc.add(make_entry(1, "a.rs"));
    svc.remove(99);
    assert_eq!(svc.snapshot().len(), 1);
}

#[test]
fn service_set_modified() {
    let svc = BufferListService::default();
    svc.add(make_entry(1, "a.rs"));
    svc.set_modified(1, true);
    assert!(svc.snapshot()[0].modified);
}

#[test]
fn service_set_modified_nonexistent() {
    let svc = BufferListService::default();
    svc.set_modified(99, true);
    assert!(svc.snapshot().is_empty());
}

#[test]
fn service_set_path() {
    let svc = BufferListService::default();
    svc.add(make_entry(1, "[No Name]"));
    svc.set_path(1, "/home/user/src/main.rs".to_string());
    let snap = svc.snapshot();
    assert_eq!(snap[0].name, "main.rs");
    assert_eq!(snap[0].path.as_deref(), Some("/home/user/src/main.rs"));
    assert_eq!(snap[0].filetype.as_deref(), Some("rust"));
}

#[test]
fn service_set_path_nonexistent() {
    let svc = BufferListService::default();
    svc.set_path(99, "/tmp/x.rs".to_string());
    assert!(svc.snapshot().is_empty());
}

// ============================================================================
// Helpers
// ============================================================================

#[test]
fn path_to_name_extracts_filename() {
    assert_eq!(path_to_name("/home/user/src/main.rs"), "main.rs");
    assert_eq!(path_to_name("main.rs"), "main.rs");
    assert_eq!(path_to_name("/a/b/c"), "c");
}

#[test]
fn guess_filetype_known() {
    assert_eq!(guess_filetype("main.rs").as_deref(), Some("rust"));
    assert_eq!(guess_filetype("app.py").as_deref(), Some("python"));
    assert_eq!(guess_filetype("index.ts").as_deref(), Some("typescript"));
    assert_eq!(guess_filetype("main.go").as_deref(), Some("go"));
    assert_eq!(guess_filetype("page.html").as_deref(), Some("html"));
    assert_eq!(guess_filetype("config.toml").as_deref(), Some("toml"));
    assert_eq!(guess_filetype("data.json").as_deref(), Some("json"));
    assert_eq!(guess_filetype("readme.md").as_deref(), Some("markdown"));
    assert_eq!(guess_filetype("script.sh").as_deref(), Some("sh"));
    assert_eq!(guess_filetype("main.c").as_deref(), Some("c"));
    assert_eq!(guess_filetype("main.cpp").as_deref(), Some("cpp"));
    assert_eq!(guess_filetype("app.java").as_deref(), Some("java"));
    assert_eq!(guess_filetype("app.rb").as_deref(), Some("ruby"));
    assert_eq!(guess_filetype("style.css").as_deref(), Some("css"));
    assert_eq!(guess_filetype("config.yaml").as_deref(), Some("yaml"));
    assert_eq!(guess_filetype("data.xml").as_deref(), Some("xml"));
    assert_eq!(guess_filetype("init.lua").as_deref(), Some("lua"));
    assert_eq!(guess_filetype("init.vim").as_deref(), Some("vim"));
    assert_eq!(guess_filetype("app.tsx").as_deref(), Some("typescriptreact"));
    assert_eq!(guess_filetype("app.jsx").as_deref(), Some("javascriptreact"));
    assert_eq!(guess_filetype("main.h").as_deref(), Some("c"));
    assert_eq!(guess_filetype("main.hpp").as_deref(), Some("cpp"));
    assert_eq!(guess_filetype("script.bash").as_deref(), Some("sh"));
    assert_eq!(guess_filetype("script.zsh").as_deref(), Some("sh"));
}

#[test]
fn guess_filetype_unknown() {
    assert_eq!(guess_filetype("file.xyz"), None);
    assert_eq!(guess_filetype("noext"), None);
}
