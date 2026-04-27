use super::*;

#[test]
fn test_icon_def_get() {
    let icon = IconDef::new("N", "U", "A");

    assert_eq!(icon.get(IconSet::Nerd), "N");
    assert_eq!(icon.get(IconSet::Unicode), "U");
    assert_eq!(icon.get(IconSet::Ascii), "A");
}

#[test]
fn test_icon_registry_default() {
    let registry = IconRegistry::default();
    assert_eq!(registry.icon_set(), IconSet::Nerd);
    assert_eq!(registry.provider_count(), 0);
}

#[test]
fn test_icon_registry_with_provider() {
    let mut registry = IconRegistry::new(IconSet::Nerd);
    registry.register(Box::new(BuiltinFileIconProvider));

    assert_eq!(registry.provider_count(), 1);

    // Test file icon lookup
    let rust_icon = registry.file_icon("main.rs", Some("rs"));
    assert_eq!(rust_icon, file_icons::RUST.nerd);

    // Unknown extension falls back to default
    let unknown_icon = registry.file_icon("file.xyz", Some("xyz"));
    assert_eq!(unknown_icon, file_icons::FILE.nerd);
}

#[test]
fn test_builtin_provider_file_icons() {
    let provider = BuiltinFileIconProvider;

    assert!(provider.file_icon("test.rs", Some("rs")).is_some());
    assert!(provider.file_icon("test.py", Some("py")).is_some());
    assert!(provider.file_icon("test.xyz", Some("xyz")).is_none());
}

#[test]
fn test_builtin_provider_kind_icons() {
    let provider = BuiltinFileIconProvider;

    assert!(provider.kind_icon("error").is_some());
    assert!(provider.kind_icon("warning").is_some());
    assert!(provider.kind_icon("unknown").is_none());
}

// =========================================================================
// Extended icon tests
// =========================================================================

#[test]
fn test_icon_set_default() {
    assert_eq!(IconSet::default(), IconSet::Nerd);
}

#[test]
fn test_icon_registry_file_icon_no_providers() {
    let registry = IconRegistry::new(IconSet::Nerd);
    // No providers, should fall back to default file icon
    let icon = registry.file_icon("test.rs", Some("rs"));
    assert_eq!(icon, file_icons::FILE.nerd);
}

#[test]
fn test_icon_registry_dir_icon() {
    let mut registry = IconRegistry::new(IconSet::Nerd);
    registry.register(Box::new(BuiltinFileIconProvider));

    // .git directory
    let icon = registry.dir_icon(".git");
    assert_eq!(icon, file_icons::GIT.nerd);

    // Unknown directory falls back to default folder
    let icon = registry.dir_icon("src");
    assert_eq!(icon, file_icons::FOLDER.nerd);
}

#[test]
fn test_icon_registry_dir_icon_no_providers() {
    let registry = IconRegistry::new(IconSet::Unicode);
    let icon = registry.dir_icon("any");
    assert_eq!(icon, file_icons::FOLDER.unicode);
}

#[test]
fn test_icon_registry_kind_icon() {
    let mut registry = IconRegistry::new(IconSet::Nerd);
    registry.register(Box::new(BuiltinFileIconProvider));

    let icon = registry.kind_icon("error");
    assert_eq!(icon, ui_icons::ERROR.nerd);

    let icon = registry.kind_icon("warning");
    assert_eq!(icon, ui_icons::WARNING.nerd);

    let icon = registry.kind_icon("info");
    assert_eq!(icon, ui_icons::INFO.nerd);

    let icon = registry.kind_icon("hint");
    assert_eq!(icon, ui_icons::HINT.nerd);

    // Unknown kind returns empty
    let icon = registry.kind_icon("unknown_kind");
    assert_eq!(icon, "");
}

#[test]
fn test_icon_registry_kind_icon_no_providers() {
    let registry = IconRegistry::new(IconSet::Nerd);
    let icon = registry.kind_icon("error");
    assert_eq!(icon, "");
}

#[test]
fn test_icon_registry_set_icon_set() {
    let mut registry = IconRegistry::new(IconSet::Nerd);
    registry.register(Box::new(BuiltinFileIconProvider));

    // Initially Nerd
    let icon = registry.file_icon("test.rs", Some("rs"));
    assert_eq!(icon, file_icons::RUST.nerd);

    // Switch to Unicode
    registry.set_icon_set(IconSet::Unicode);
    let icon = registry.file_icon("test.rs", Some("rs"));
    assert_eq!(icon, file_icons::RUST.unicode);

    // Switch to ASCII
    registry.set_icon_set(IconSet::Ascii);
    let icon = registry.file_icon("test.rs", Some("rs"));
    assert_eq!(icon, file_icons::RUST.ascii);
}

#[test]
fn test_builtin_provider_all_file_extensions() {
    let provider = BuiltinFileIconProvider;

    // All supported extensions
    assert!(provider.file_icon("test.rs", Some("rs")).is_some());
    assert!(provider.file_icon("test.js", Some("js")).is_some());
    assert!(provider.file_icon("test.mjs", Some("mjs")).is_some());
    assert!(provider.file_icon("test.cjs", Some("cjs")).is_some());
    assert!(provider.file_icon("test.ts", Some("ts")).is_some());
    assert!(provider.file_icon("test.mts", Some("mts")).is_some());
    assert!(provider.file_icon("test.cts", Some("cts")).is_some());
    assert!(provider.file_icon("test.py", Some("py")).is_some());
    assert!(provider.file_icon("test.pyi", Some("pyi")).is_some());
    assert!(provider.file_icon("test.md", Some("md")).is_some());
    assert!(
        provider
            .file_icon("test.markdown", Some("markdown"))
            .is_some()
    );
    assert!(provider.file_icon("test.json", Some("json")).is_some());
    assert!(provider.file_icon("test.toml", Some("toml")).is_some());
    assert!(provider.file_icon("test.yaml", Some("yaml")).is_some());
    assert!(provider.file_icon("test.yml", Some("yml")).is_some());

    // No extension should return None
    assert!(provider.file_icon("test", None).is_none());
}

#[test]
fn test_builtin_provider_kind_icons_all() {
    let provider = BuiltinFileIconProvider;

    assert!(provider.kind_icon("error").is_some());
    assert!(provider.kind_icon("warning").is_some());
    assert!(provider.kind_icon("warn").is_some());
    assert!(provider.kind_icon("info").is_some());
    assert!(provider.kind_icon("information").is_some());
    assert!(provider.kind_icon("hint").is_some());
}

#[test]
fn test_builtin_provider_metadata() {
    let provider = BuiltinFileIconProvider;
    assert_eq!(provider.name(), "builtin-file");
    assert_eq!(provider.priority(), 0);
}

#[test]
fn test_builtin_provider_dir_icon_git() {
    let provider = BuiltinFileIconProvider;
    assert!(provider.dir_icon(".git").is_some());
    assert!(provider.dir_icon("src").is_none());
}

/// Test priority-based provider ordering.
#[test]
fn test_icon_registry_provider_priority() {
    struct HighPriorityProvider;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl IconProvider for HighPriorityProvider {
        fn name(&self) -> &'static str {
            "high-priority"
        }
        fn priority(&self) -> u8 {
            100
        }
        fn file_icon(&self, _filename: &str, extension: Option<&str>) -> Option<&'static IconDef> {
            if extension == Some("rs") {
                Some(&ui_icons::CHECK) // Return a different icon to verify priority
            } else {
                None
            }
        }
        fn dir_icon(&self, _dirname: &str) -> Option<&'static IconDef> {
            None
        }
        fn kind_icon(&self, _kind: &str) -> Option<&'static IconDef> {
            None
        }
    }

    let mut registry = IconRegistry::new(IconSet::Nerd);
    registry.register(Box::new(BuiltinFileIconProvider));
    registry.register(Box::new(HighPriorityProvider));

    // High-priority provider should win for .rs files
    let icon = registry.file_icon("test.rs", Some("rs"));
    assert_eq!(icon, ui_icons::CHECK.nerd);

    // For other files, builtin should handle it
    let icon = registry.file_icon("test.py", Some("py"));
    assert_eq!(icon, file_icons::PYTHON.nerd);
}

#[test]
fn test_file_icons_constants() {
    // Verify file icon constants exist and have all three variants
    let icons = [
        &file_icons::FILE,
        &file_icons::FOLDER,
        &file_icons::FOLDER_OPEN,
        &file_icons::RUST,
        &file_icons::JAVASCRIPT,
        &file_icons::TYPESCRIPT,
        &file_icons::PYTHON,
        &file_icons::MARKDOWN,
        &file_icons::JSON,
        &file_icons::TOML,
        &file_icons::YAML,
        &file_icons::GIT,
    ];

    for icon in icons {
        assert!(!icon.nerd.is_empty());
        assert!(!icon.unicode.is_empty());
        assert!(!icon.ascii.is_empty());
    }
}

#[test]
fn test_ui_icons_constants() {
    let icons = [
        &ui_icons::ERROR,
        &ui_icons::WARNING,
        &ui_icons::INFO,
        &ui_icons::HINT,
        &ui_icons::MODIFIED,
        &ui_icons::READONLY,
        &ui_icons::SEARCH,
        &ui_icons::CLOSE,
        &ui_icons::CHECK,
        &ui_icons::ARROW_RIGHT,
        &ui_icons::ARROW_DOWN,
    ];

    for icon in icons {
        assert!(!icon.nerd.is_empty());
        assert!(!icon.unicode.is_empty());
        assert!(!icon.ascii.is_empty());
    }
}
