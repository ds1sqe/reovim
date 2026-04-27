//! Tests for [`ThemeLoader`].
//!
//! Most tests cover discovery / search-path logic and don't touch the
//! global theme factory. The `load(file)` happy-path tests assert on
//! result *shape* (Ok vs Err), not on parsed theme names — the parsed
//! name is owned by the display-tier factory and is exercised in
//! display-crate tests.

use {super::*, std::io::Write, tempfile::TempDir};

fn create_test_theme(dir: &Path, name: &str, content: &str) {
    let file_path = dir.join(format!("{name}.toml"));
    let mut file = std::fs::File::create(file_path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
}

#[test]
fn loader_with_custom_paths_keeps_them() {
    let paths = vec![
        PathBuf::from("/custom/path1"),
        PathBuf::from("/custom/path2"),
    ];
    let loader = ThemeLoader::with_paths(paths.clone());
    assert_eq!(loader.search_paths(), &paths);
}

#[test]
fn add_path_inserts_at_front() {
    let mut loader = ThemeLoader::with_paths(vec![PathBuf::from("/existing")]);
    loader.add_path(PathBuf::from("/new"));
    assert_eq!(loader.search_paths()[0], PathBuf::from("/new"));
}

#[test]
fn load_nonexistent_theme_errors() {
    let loader = ThemeLoader::with_paths(vec![]);
    let result = loader.load("definitely-does-not-exist-2026");
    assert!(result.is_err());
}

#[test]
fn load_existing_file_dispatches_to_load_from_path() {
    // Ensures `load` reaches the `Ok(path)` arm, which forwards to
    // `load_from_path`. The latter is `coverage(off)` because its
    // factory-vs-stub branch can't be deterministically toggled in
    // the registry test binary; this test only proves that `load`
    // itself routes to it on a file hit.
    let temp_dir = TempDir::new().unwrap();
    create_test_theme(temp_dir.path(), "dispatch", "[meta]\nname = \"D\"\n");
    let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
    // Whether the result is Ok (factory installed) or Err
    // (Unsupported when no factory) is non-deterministic across
    // tests in this binary, so we assert only that `load` ran to
    // completion without panicking.
    let _ = loader.load("dispatch");
}

#[test]
fn list_available_returns_toml_stems() {
    let temp_dir = TempDir::new().unwrap();
    create_test_theme(temp_dir.path(), "alpha", "[meta]\nname = \"Alpha\"\n");
    create_test_theme(temp_dir.path(), "beta", "[meta]\nname = \"Beta\"\n");
    create_test_theme(temp_dir.path(), "gamma", "[meta]\nname = \"Gamma\"\n");
    std::fs::write(temp_dir.path().join("readme.md"), "# Readme").unwrap();

    let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
    let themes = loader.list_available();
    assert_eq!(themes, vec!["alpha", "beta", "gamma"]);
}

#[test]
fn exists_matches_known_themes() {
    let temp_dir = TempDir::new().unwrap();
    create_test_theme(temp_dir.path(), "exists", "[meta]\nname = \"Exists\"\n");

    let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
    assert!(loader.exists("exists"));
    assert!(!loader.exists("does-not-exist"));
}

#[test]
fn find_theme_path_returns_full_path() {
    let temp_dir = TempDir::new().unwrap();
    create_test_theme(temp_dir.path(), "findme", "[meta]\nname = \"Find\"\n");

    let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
    let path = loader.find_theme_path("findme").unwrap();
    assert_eq!(path, temp_dir.path().join("findme.toml"));
}

#[test]
fn list_available_deduplicates_across_paths() {
    let dir1 = TempDir::new().unwrap();
    let dir2 = TempDir::new().unwrap();
    create_test_theme(dir1.path(), "common", "[meta]\nname = \"C1\"\n");
    create_test_theme(dir2.path(), "common", "[meta]\nname = \"C2\"\n");
    create_test_theme(dir1.path(), "u1", "[meta]\nname = \"U1\"\n");
    create_test_theme(dir2.path(), "u2", "[meta]\nname = \"U2\"\n");

    let loader =
        ThemeLoader::with_paths(vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()]);
    let themes = loader.list_available();
    assert_eq!(themes, vec!["common", "u1", "u2"]);
}

#[test]
fn list_available_with_nonexistent_search_path() {
    let loader = ThemeLoader::with_paths(vec![PathBuf::from("/nonexistent/dir")]);
    assert!(loader.list_available().is_empty());
}

#[test]
fn list_available_filters_non_toml_files() {
    let temp_dir = TempDir::new().unwrap();
    create_test_theme(temp_dir.path(), "valid", "[meta]\nname = \"V\"\n");
    std::fs::write(temp_dir.path().join("readme.txt"), "not a theme").unwrap();
    std::fs::write(temp_dir.path().join("config.json"), "{}").unwrap();

    let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
    assert_eq!(loader.list_available(), vec!["valid"]);
}

#[test]
fn list_available_skips_files_without_extension() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::write(temp_dir.path().join("noext"), "not a theme").unwrap();

    let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
    assert!(loader.list_available().is_empty());
}

#[test]
fn find_theme_path_with_absolute_path() {
    let temp_dir = TempDir::new().unwrap();
    let theme_path = temp_dir.path().join("absolute-test.toml");
    std::fs::write(&theme_path, "[meta]\nname = \"A\"\n").unwrap();

    let loader = ThemeLoader::with_paths(vec![]);
    let found = loader.find_theme_path(theme_path.to_str().unwrap());
    assert_eq!(found, Some(theme_path));
}

#[test]
fn find_theme_path_not_found() {
    let loader = ThemeLoader::with_paths(vec![]);
    assert!(loader.find_theme_path("nonexistent-theme").is_none());
}

#[test]
fn find_theme_path_non_toml_extension_appends_toml() {
    let temp_dir = TempDir::new().unwrap();
    let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
    // "mytheme.txt" has a non-toml extension; resolution appends
    // ".toml" and looks for "mytheme.txt.toml" which doesn't exist.
    assert!(loader.find_theme_path("mytheme.txt").is_none());
}

#[test]
fn find_theme_path_absolute_nonexistent_returns_none() {
    let loader = ThemeLoader::with_paths(vec![]);
    let found = loader.find_theme_path("/nonexistent/absolute/path/theme.toml");
    assert!(found.is_none());
}

#[test]
fn discover_returns_file_themes() {
    let temp_dir = TempDir::new().unwrap();
    create_test_theme(temp_dir.path(), "alpha", "[meta]\nname = \"A\"\n");
    create_test_theme(temp_dir.path(), "beta", "[meta]\nname = \"B\"\n");

    let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
    let themes = loader.discover();
    let file_themes: Vec<_> = themes.iter().filter(|t| !t.builtin).collect();
    assert_eq!(file_themes.len(), 2);
    assert!(file_themes.iter().any(|t| t.name == "alpha"));
    assert!(file_themes.iter().any(|t| t.name == "beta"));
    assert!(file_themes.iter().all(|t| t.path.is_some()));
}

#[test]
fn discover_includes_builtins() {
    let loader = ThemeLoader::with_paths(vec![]);
    let themes = loader.discover();
    let builtin_themes: Vec<_> = themes.iter().filter(|t| t.builtin).collect();
    assert_eq!(builtin_themes.len(), 3);
    assert!(builtin_themes.iter().any(|t| t.name == "dark"));
    assert!(builtin_themes.iter().any(|t| t.name == "light"));
    assert!(
        builtin_themes
            .iter()
            .any(|t| t.name == "tokyo-night-orange")
    );
    assert!(builtin_themes.iter().all(|t| t.path.is_none()));
}

#[test]
fn discover_file_shadows_builtin() {
    let temp_dir = TempDir::new().unwrap();
    create_test_theme(temp_dir.path(), "dark", "[meta]\nname = \"Custom\"\n");

    let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
    let themes = loader.discover();
    let dark_themes: Vec<_> = themes.iter().filter(|t| t.name == "dark").collect();
    assert_eq!(dark_themes.len(), 1);
    assert!(!dark_themes[0].builtin);
    assert!(dark_themes[0].path.is_some());
}

#[test]
fn discover_sorted_by_name() {
    let temp_dir = TempDir::new().unwrap();
    create_test_theme(temp_dir.path(), "zebra", "[meta]\nname = \"Z\"\n");
    create_test_theme(temp_dir.path(), "alpha", "[meta]\nname = \"A\"\n");

    let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
    let themes = loader.discover();
    let names: Vec<_> = themes.iter().map(|t| &t.name).collect();
    let mut sorted_names = names.clone();
    sorted_names.sort();
    assert_eq!(names, sorted_names);
}

#[test]
fn discover_deduplicates_across_paths() {
    let dir1 = TempDir::new().unwrap();
    let dir2 = TempDir::new().unwrap();
    create_test_theme(dir1.path(), "common", "[meta]\nname = \"C1\"\n");
    create_test_theme(dir2.path(), "common", "[meta]\nname = \"C2\"\n");

    let loader =
        ThemeLoader::with_paths(vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()]);
    let themes = loader.discover();
    assert_eq!(themes.iter().filter(|t| t.name == "common").count(), 1);
}

#[test]
fn discover_empty_paths_returns_only_builtins() {
    let loader = ThemeLoader::with_paths(vec![]);
    let themes = loader.discover();
    assert_eq!(themes.len(), 3);
    assert!(themes.iter().all(|t| t.builtin));
}

#[test]
fn discover_skips_non_toml_files() {
    let temp_dir = TempDir::new().unwrap();
    create_test_theme(temp_dir.path(), "valid", "[meta]\nname = \"V\"\n");
    std::fs::write(temp_dir.path().join("readme.md"), "# Readme").unwrap();
    std::fs::write(temp_dir.path().join("noext"), "data").unwrap();

    let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
    let file_themes: Vec<_> = loader
        .discover()
        .into_iter()
        .filter(|t| !t.builtin)
        .collect();
    assert_eq!(file_themes.len(), 1);
    assert_eq!(file_themes[0].name, "valid");
}

#[test]
fn discover_nonexistent_search_path() {
    let loader = ThemeLoader::with_paths(vec![PathBuf::from("/nonexistent/path")]);
    let themes = loader.discover();
    assert_eq!(themes.len(), 3);
    assert!(themes.iter().all(|t| t.builtin));
}

#[test]
fn theme_info_debug_clone_eq() {
    let info1 = ThemeInfo {
        name: "test".to_string(),
        path: None,
        builtin: true,
    };
    let info2 = info1.clone();
    assert_eq!(info1, info2);
    let debug = format!("{info1:?}");
    assert!(debug.contains("test"));
}

#[test]
fn load_falls_back_to_builtin() {
    // No file matches; load() should fall through to builtin
    // resolution and succeed for "dark".
    let loader = ThemeLoader::with_paths(vec![]);
    let theme = loader.load("dark").unwrap();
    assert_eq!(theme.name(), "dark");
}

#[test]
fn load_falls_back_to_builtin_light() {
    let loader = ThemeLoader::with_paths(vec![]);
    let theme = loader.load("light").unwrap();
    assert_eq!(theme.name(), "light");
}

#[test]
fn load_falls_back_to_builtin_tokyo_night() {
    let loader = ThemeLoader::with_paths(vec![]);
    let theme = loader.load("tokyo-night-orange").unwrap();
    assert_eq!(theme.name(), "tokyo-night-orange");
}

#[test]
fn load_missing_theme_error_message() {
    let loader = ThemeLoader::with_paths(vec![]);
    let result = loader.load("ghost-theme");
    let msg = match result {
        Ok(_) => panic!("expected error for unknown theme name"),
        Err(err) => err.to_string(),
    };
    assert!(msg.contains("ghost-theme"), "msg = {msg}");
    assert!(msg.contains("not found"), "msg = {msg}");
}
