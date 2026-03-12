use {super::*, reovim_driver_command::Command};

#[test]
fn trigger_metadata() {
    let cmd = Trigger;
    assert_eq!(cmd.id(), ids::TRIGGER);
    assert!(!cmd.description().is_empty());
}

#[test]
fn next_metadata() {
    let cmd = Next;
    assert_eq!(cmd.id(), ids::NEXT);
    assert!(!cmd.description().is_empty());
}

#[test]
fn prev_metadata() {
    let cmd = Prev;
    assert_eq!(cmd.id(), ids::PREV);
    assert!(!cmd.description().is_empty());
}

#[test]
fn confirm_metadata() {
    let cmd = Confirm;
    assert_eq!(cmd.id(), ids::CONFIRM);
    assert!(!cmd.description().is_empty());
}

#[test]
fn dismiss_metadata() {
    let cmd = Dismiss;
    assert_eq!(cmd.id(), ids::DISMISS);
    assert!(!cmd.description().is_empty());
}

#[test]
fn command_handlers_count() {
    let handlers = command_handlers();
    assert_eq!(handlers.len(), 5);
}

#[test]
fn command_handlers_unique_ids() {
    let handlers = command_handlers();
    let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
    let mut deduped = ids.clone();
    deduped.sort_by_key(CommandId::name);
    deduped.dedup_by_key(|id| id.name());
    assert_eq!(ids.len(), deduped.len());
}

// ========================================================================
// language_id_from_path tests
// ========================================================================

#[test]
fn language_id_rust() {
    assert_eq!(language_id_from_path("src/main.rs"), Some("rust".to_owned()));
}

#[test]
fn language_id_python() {
    assert_eq!(language_id_from_path("script.py"), Some("python".to_owned()));
    assert_eq!(language_id_from_path("stubs.pyi"), Some("python".to_owned()));
}

#[test]
fn language_id_typescript() {
    assert_eq!(language_id_from_path("app.ts"), Some("typescript".to_owned()));
    assert_eq!(language_id_from_path("Component.tsx"), Some("typescriptreact".to_owned()));
}

#[test]
fn language_id_javascript() {
    assert_eq!(language_id_from_path("index.js"), Some("javascript".to_owned()));
    assert_eq!(language_id_from_path("App.jsx"), Some("javascriptreact".to_owned()));
}

#[test]
fn language_id_c_cpp() {
    assert_eq!(language_id_from_path("main.c"), Some("c".to_owned()));
    assert_eq!(language_id_from_path("util.h"), Some("c".to_owned()));
    assert_eq!(language_id_from_path("main.cpp"), Some("cpp".to_owned()));
    assert_eq!(language_id_from_path("main.cc"), Some("cpp".to_owned()));
    assert_eq!(language_id_from_path("main.cxx"), Some("cpp".to_owned()));
    assert_eq!(language_id_from_path("header.hpp"), Some("cpp".to_owned()));
}

#[test]
fn language_id_other_languages() {
    assert_eq!(language_id_from_path("main.go"), Some("go".to_owned()));
    assert_eq!(language_id_from_path("Main.java"), Some("java".to_owned()));
    assert_eq!(language_id_from_path("init.lua"), Some("lua".to_owned()));
    assert_eq!(language_id_from_path("app.rb"), Some("ruby".to_owned()));
    assert_eq!(language_id_from_path("main.zig"), Some("zig".to_owned()));
}

#[test]
fn language_id_config_files() {
    assert_eq!(language_id_from_path("Cargo.toml"), Some("toml".to_owned()));
    assert_eq!(language_id_from_path("data.json"), Some("json".to_owned()));
    assert_eq!(language_id_from_path("config.yaml"), Some("yaml".to_owned()));
    assert_eq!(language_id_from_path("config.yml"), Some("yaml".to_owned()));
    assert_eq!(language_id_from_path("README.md"), Some("markdown".to_owned()));
    assert_eq!(language_id_from_path("doc.markdown"), Some("markdown".to_owned()));
}

#[test]
fn language_id_unknown_extension() {
    assert_eq!(language_id_from_path("file.xyz"), None);
    assert_eq!(language_id_from_path("file.wasm"), None);
}

#[test]
fn language_id_no_extension() {
    assert_eq!(language_id_from_path("Makefile"), None);
    assert_eq!(language_id_from_path("/usr/bin/cat"), None);
}

#[test]
fn language_id_nested_path() {
    assert_eq!(language_id_from_path("/home/user/project/src/lib.rs"), Some("rust".to_owned()));
}

// ========================================================================
// find_project_root tests
// ========================================================================

#[test]
fn find_project_root_from_this_crate() {
    // This crate has a Cargo.toml, so we should find it.
    let this_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands.rs");
    let root = find_project_root(&this_file);
    assert!(root.is_some());
    let root = root.unwrap();
    assert!(root.join("Cargo.toml").exists());
}

#[test]
fn find_project_root_from_directory() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let root = find_project_root(&dir);
    assert!(root.is_some());
}

#[test]
fn find_project_root_nonexistent() {
    // Root "/" has no Cargo.toml.
    let root = find_project_root(Path::new("/nonexistent/path/file.rs"));
    assert!(root.is_none());
}

// ========================================================================
// LspStartingGuard tests (#521)
// ========================================================================

#[test]
fn lsp_starting_guard_prevents_concurrent_starts() {
    let guard = LspStartingGuard::default();
    // First acquisition succeeds.
    assert!(
        guard
            .0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    );
    // Second acquisition fails — already in progress.
    assert!(
        guard
            .0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
    );
}

#[test]
fn lsp_starting_guard_resets_after_completion() {
    let guard = LspStartingGuard::default();
    guard
        .0
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .unwrap();
    // Reset.
    guard.0.store(false, Ordering::Release);
    // Now re-acquisition succeeds.
    assert!(
        guard
            .0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    );
}

#[test]
fn lsp_starting_guard_service_trait() {
    use reovim_kernel::api::v1::ServiceRegistry;
    let registry = ServiceRegistry::new();
    let guard = registry.get_or_create::<LspStartingGuard>();
    assert!(!guard.0.load(Ordering::Relaxed));
}
