use super::*;

#[test]
fn test_detect_by_extension() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("main.rs"), "rust");
    assert_eq!(registry.filetype_id("script.py"), "python");
    assert_eq!(registry.filetype_id("app.js"), "javascript");
    assert_eq!(registry.filetype_id("config.toml"), "toml");
}

#[test]
fn test_detect_by_filename() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("Makefile"), "make");
    assert_eq!(registry.filetype_id("Dockerfile"), "dockerfile");
    assert_eq!(registry.filetype_id(".gitignore"), "gitignore");
}

#[test]
fn test_detect_with_path() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("/home/user/project/src/main.rs"), "rust");
    assert_eq!(registry.filetype_id("./scripts/build.sh"), "bash");
}

#[test]
fn test_unknown_extension() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("file.unknown"), "");
    assert_eq!(registry.filetype_id("no_extension"), "");
}

#[test]
fn test_case_insensitive_extension() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("main.RS"), "rust");
    assert_eq!(registry.filetype_id("script.PY"), "python");
    assert_eq!(registry.filetype_id("config.TOML"), "toml");
}

#[test]
fn test_filetype_info() {
    let registry = FiletypeRegistry::new();

    let info = registry.detect("main.rs").unwrap();
    assert_eq!(info.id, "rust");
    assert_eq!(info.display_name, "Rust");
    assert_eq!(info.icon, Some(""));
}

#[test]
fn test_global_registry() {
    assert_eq!(filetype_id("test.rs"), "rust");
    assert!(detect_filetype("test.py").is_some());
}

#[test]
fn test_empty_registry() {
    let registry = FiletypeRegistry::empty();

    assert_eq!(registry.extension_count(), 0);
    assert_eq!(registry.filename_count(), 0);
    assert!(registry.detect("main.rs").is_none());
}

#[test]
fn test_custom_registration() {
    let mut registry = FiletypeRegistry::empty();

    registry.register_extension("custom", FiletypeInfo::new("custom", "Custom"));
    registry.register_filename("CustomFile", FiletypeInfo::new("customfile", "Custom File"));

    assert_eq!(registry.filetype_id("test.custom"), "custom");
    assert_eq!(registry.filetype_id("CustomFile"), "customfile");
}

#[test]
fn test_icon_detection() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.icon("main.rs"), Some(""));
    assert_eq!(registry.icon("file.txt"), None);
    assert_eq!(registry.icon("unknown.xyz"), None);
}

#[test]
fn test_display_name() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.display_name("main.rs"), "Rust");
    assert_eq!(registry.display_name("script.py"), "Python");
    assert_eq!(registry.display_name("unknown.xyz"), "");
}

#[test]
fn test_has_methods() {
    let registry = FiletypeRegistry::new();

    assert!(registry.has_extension("rs"));
    assert!(registry.has_extension("py"));
    assert!(!registry.has_extension("unknown"));

    assert!(registry.has_filename("Makefile"));
    assert!(registry.has_filename("Dockerfile"));
    assert!(!registry.has_filename("RandomFile"));
}

#[test]
fn test_filetype_registry_default() {
    let registry = FiletypeRegistry::default();
    // Default should behave same as new()
    assert!(registry.has_extension("rs"));
    assert!(registry.extension_count() > 0);
}

#[test]
fn test_detect_cpp_extensions() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("main.cpp"), "cpp");
    assert_eq!(registry.filetype_id("main.cc"), "cpp");
    assert_eq!(registry.filetype_id("main.cxx"), "cpp");
    assert_eq!(registry.filetype_id("header.hpp"), "cpp");
    assert_eq!(registry.filetype_id("header.hxx"), "cpp");
}

#[test]
fn test_detect_c_extensions() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("main.c"), "c");
    assert_eq!(registry.filetype_id("header.h"), "c");
}

#[test]
fn test_detect_python_extensions() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("script.py"), "python");
    assert_eq!(registry.filetype_id("stub.pyi"), "python");
    assert_eq!(registry.filetype_id("script.pyw"), "python");
}

#[test]
fn test_detect_javascript_typescript_extensions() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("app.js"), "javascript");
    assert_eq!(registry.filetype_id("module.mjs"), "javascript");
    assert_eq!(registry.filetype_id("module.cjs"), "javascript");
    assert_eq!(registry.filetype_id("app.ts"), "typescript");
    assert_eq!(registry.filetype_id("component.jsx"), "javascriptreact");
    assert_eq!(registry.filetype_id("component.tsx"), "typescriptreact");
}

#[test]
fn test_detect_go() {
    let registry = FiletypeRegistry::new();
    assert_eq!(registry.filetype_id("main.go"), "go");
}

#[test]
fn test_detect_lua() {
    let registry = FiletypeRegistry::new();
    assert_eq!(registry.filetype_id("init.lua"), "lua");
}

#[test]
fn test_detect_shell_extensions() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("script.sh"), "bash");
    assert_eq!(registry.filetype_id("script.bash"), "bash");
    assert_eq!(registry.filetype_id("script.zsh"), "zsh");
    assert_eq!(registry.filetype_id("script.fish"), "fish");
}

#[test]
fn test_detect_data_formats() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("config.json"), "json");
    assert_eq!(registry.filetype_id("config.toml"), "toml");
    assert_eq!(registry.filetype_id("config.yaml"), "yaml");
    assert_eq!(registry.filetype_id("config.yml"), "yaml");
    assert_eq!(registry.filetype_id("data.xml"), "xml");
}

#[test]
fn test_detect_markup_extensions() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("readme.md"), "markdown");
    assert_eq!(registry.filetype_id("doc.markdown"), "markdown");
    assert_eq!(registry.filetype_id("page.html"), "html");
    assert_eq!(registry.filetype_id("page.htm"), "html");
    assert_eq!(registry.filetype_id("style.css"), "css");
    assert_eq!(registry.filetype_id("style.scss"), "scss");
    assert_eq!(registry.filetype_id("style.sass"), "sass");
}

#[test]
fn test_detect_database_extensions() {
    let registry = FiletypeRegistry::new();
    assert_eq!(registry.filetype_id("query.sql"), "sql");
}

#[test]
fn test_detect_vim_extension() {
    let registry = FiletypeRegistry::new();
    assert_eq!(registry.filetype_id("init.vim"), "vim");
}

#[test]
fn test_detect_text_extension() {
    let registry = FiletypeRegistry::new();
    let info = registry.detect("notes.txt").unwrap();
    assert_eq!(info.id, "text");
    assert_eq!(info.display_name, "Text");
    assert_eq!(info.icon, None); // Text has no icon
}

#[test]
fn test_detect_java_kotlin() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("Main.java"), "java");
    assert_eq!(registry.filetype_id("App.kt"), "kotlin");
    assert_eq!(registry.filetype_id("build.kts"), "kotlin");
}

#[test]
fn test_detect_ruby_extensions() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("app.rb"), "ruby");
    assert_eq!(registry.filetype_id("template.erb"), "eruby");
}

#[test]
fn test_detect_php() {
    let registry = FiletypeRegistry::new();
    assert_eq!(registry.filetype_id("index.php"), "php");
}

#[test]
fn test_detect_swift() {
    let registry = FiletypeRegistry::new();
    assert_eq!(registry.filetype_id("App.swift"), "swift");
}

#[test]
fn test_detect_zig() {
    let registry = FiletypeRegistry::new();
    assert_eq!(registry.filetype_id("main.zig"), "zig");
}

#[test]
fn test_detect_elixir() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("app.ex"), "elixir");
    assert_eq!(registry.filetype_id("test.exs"), "elixir");
}

#[test]
fn test_detect_special_filenames() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.filetype_id("makefile"), "make");
    assert_eq!(registry.filetype_id("GNUmakefile"), "make");
    assert_eq!(registry.filetype_id("Containerfile"), "dockerfile");
    assert_eq!(registry.filetype_id(".gitattributes"), "gitattributes");
    assert_eq!(registry.filetype_id("Cargo.toml"), "toml");
    assert_eq!(registry.filetype_id("Cargo.lock"), "toml");
    assert_eq!(registry.filetype_id("package.json"), "json");
    assert_eq!(registry.filetype_id("tsconfig.json"), "json");
}

#[test]
fn test_filetype_info_new() {
    let info = FiletypeInfo::new("test", "Test Lang");
    assert_eq!(info.id, "test");
    assert_eq!(info.display_name, "Test Lang");
    assert_eq!(info.icon, None);
}

#[test]
fn test_filetype_info_with_icon() {
    let info = FiletypeInfo::with_icon("test", "Test Lang", "X");
    assert_eq!(info.id, "test");
    assert_eq!(info.display_name, "Test Lang");
    assert_eq!(info.icon, Some("X"));
}

#[test]
fn test_filetype_info_equality() {
    let a = FiletypeInfo::new("rust", "Rust");
    let b = FiletypeInfo::new("rust", "Rust");
    let c = FiletypeInfo::new("python", "Python");

    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn test_filetype_info_clone() {
    let original = FiletypeInfo::with_icon("rust", "Rust", "R");
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn test_registry_extension_count() {
    let registry = FiletypeRegistry::new();
    // Should have many extensions registered
    assert!(registry.extension_count() > 30);
}

#[test]
fn test_registry_filename_count() {
    let registry = FiletypeRegistry::new();
    // Should have several filenames registered
    assert!(registry.filename_count() >= 10);
}

#[test]
fn test_custom_registration_overrides() {
    let mut registry = FiletypeRegistry::new();

    // Override the existing "rs" extension
    registry.register_extension("rs", FiletypeInfo::new("custom_rust", "Custom Rust"));
    assert_eq!(registry.filetype_id("test.rs"), "custom_rust");
}

#[test]
fn test_detect_with_directory_path() {
    let registry = FiletypeRegistry::new();
    // Filename match in a nested path
    assert_eq!(registry.filetype_id("/home/user/project/Dockerfile"), "dockerfile");
}

#[test]
fn test_detect_filename_priority_over_extension() {
    let registry = FiletypeRegistry::new();
    // "Cargo.toml" should match filename first, not just ".toml" extension
    let info = registry.detect("Cargo.toml").unwrap();
    assert_eq!(info.display_name, "Cargo.toml"); // Filename match gives "Cargo.toml"
}

#[test]
fn test_global_registry_detect_filetype() {
    // Test the convenience functions
    let info = detect_filetype("main.rs");
    assert!(info.is_some());
    let info = info.unwrap();
    assert_eq!(info.id, "rust");
}

#[test]
fn test_global_registry_unknown() {
    assert!(detect_filetype("file.xyzabc").is_none());
    assert_eq!(filetype_id("file.xyzabc"), "");
}

#[test]
fn test_display_name_for_known_types() {
    let registry = FiletypeRegistry::new();

    assert_eq!(registry.display_name("main.go"), "Go");
    assert_eq!(registry.display_name("init.lua"), "Lua");
    assert_eq!(registry.display_name("query.sql"), "SQL");
}

#[test]
fn test_icon_for_various_types() {
    let registry = FiletypeRegistry::new();

    // Types with icons
    assert!(registry.icon("main.rs").is_some());
    assert!(registry.icon("app.py").is_some());
    assert!(registry.icon("main.go").is_some());

    // Text has no icon
    assert!(registry.icon("notes.txt").is_none());
}
