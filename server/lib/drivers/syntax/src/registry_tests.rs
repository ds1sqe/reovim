use super::*;

#[test]
fn test_language_info_new() {
    let info = LanguageInfo::new("rust", "Rust");

    assert_eq!(info.id, "rust");
    assert_eq!(info.name, "Rust");
    assert!(info.extensions.is_empty());
    assert!(info.mime_types.is_empty());
    assert!(!info.has_comments());
}

#[test]
fn test_language_info_with_extensions() {
    let info = LanguageInfo::new("rust", "Rust").with_extensions(["rs"]);

    assert_eq!(info.extensions, vec!["rs"]);
    assert!(info.has_extensions());
}

#[test]
fn test_language_info_with_mime_types() {
    let info = LanguageInfo::new("rust", "Rust").with_mime_types(["text/x-rust"]);

    assert_eq!(info.mime_types, vec!["text/x-rust"]);
}

#[test]
fn test_language_info_with_comments() {
    let info = LanguageInfo::new("rust", "Rust")
        .with_comments(CommentTokens::with_block("//", "/*", "*/"));

    assert!(info.has_comments());
    assert!(info.comment_tokens.has_line_comment());
    assert!(info.comment_tokens.has_block_comment());
}

#[test]
fn test_language_info_matches_extension() {
    let info = LanguageInfo::new("rust", "Rust").with_extensions(["rs"]);

    assert!(info.matches_extension("rs"));
    assert!(info.matches_extension("RS")); // Case insensitive
    assert!(info.matches_extension("Rs"));
    assert!(!info.matches_extension("py"));
}

#[test]
fn test_language_info_matches_mime() {
    let info = LanguageInfo::new("rust", "Rust").with_mime_types(["text/x-rust"]);

    assert!(info.matches_mime("text/x-rust"));
    assert!(info.matches_mime("TEXT/X-RUST")); // Case insensitive
    assert!(!info.matches_mime("text/plain"));
}

#[test]
fn test_comment_tokens_line_only() {
    let tokens = CommentTokens::line_only("#");

    assert_eq!(tokens.line, Some("#".to_string()));
    assert_eq!(tokens.block_start, None);
    assert_eq!(tokens.block_end, None);
    assert!(tokens.has_line_comment());
    assert!(!tokens.has_block_comment());
    assert!(tokens.has_any());
}

#[test]
fn test_comment_tokens_with_block() {
    let tokens = CommentTokens::with_block("//", "/*", "*/");

    assert_eq!(tokens.line, Some("//".to_string()));
    assert_eq!(tokens.block_start, Some("/*".to_string()));
    assert_eq!(tokens.block_end, Some("*/".to_string()));
    assert!(tokens.has_line_comment());
    assert!(tokens.has_block_comment());
    assert!(tokens.has_any());
}

#[test]
fn test_comment_tokens_block_only() {
    let tokens = CommentTokens::block_only("<!--", "-->");

    assert_eq!(tokens.line, None);
    assert_eq!(tokens.block_start, Some("<!--".to_string()));
    assert_eq!(tokens.block_end, Some("-->".to_string()));
    assert!(!tokens.has_line_comment());
    assert!(tokens.has_block_comment());
    assert!(tokens.has_any());
}

#[test]
fn test_comment_tokens_default() {
    let tokens = CommentTokens::default();

    assert_eq!(tokens.line, None);
    assert_eq!(tokens.block_start, None);
    assert_eq!(tokens.block_end, None);
    assert!(!tokens.has_line_comment());
    assert!(!tokens.has_block_comment());
    assert!(!tokens.has_any());
}

#[test]
fn test_language_info_no_extensions() {
    let info = LanguageInfo::new("test", "Test");
    assert!(!info.has_extensions());
    assert!(!info.matches_extension("rs"));
}

#[test]
fn test_language_info_no_mime_types() {
    let info = LanguageInfo::new("test", "Test");
    assert!(!info.matches_mime("text/plain"));
}

#[test]
fn test_language_info_multiple_extensions() {
    let info = LanguageInfo::new("python", "Python").with_extensions(["py", "pyw", "pyi"]);

    assert!(info.matches_extension("py"));
    assert!(info.matches_extension("pyw"));
    assert!(info.matches_extension("pyi"));
    assert!(!info.matches_extension("rs"));
}

#[test]
fn test_language_info_chaining() {
    let info = LanguageInfo::new("rust", "Rust")
        .with_extensions(["rs"])
        .with_mime_types(["text/x-rust"])
        .with_comments(CommentTokens::with_block("//", "/*", "*/"));

    assert_eq!(info.id, "rust");
    assert_eq!(info.name, "Rust");
    assert!(info.has_extensions());
    assert!(info.has_comments());
    assert!(info.matches_extension("rs"));
    assert!(info.matches_mime("text/x-rust"));
}

#[test]
fn test_comment_tokens_equality() {
    let a = CommentTokens::line_only("//");
    let b = CommentTokens::line_only("//");
    let c = CommentTokens::line_only("#");

    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn test_comment_tokens_partial_block() {
    // Only block_start without block_end should not count as block comment
    let tokens = CommentTokens {
        line: None,
        block_start: Some("/*".to_string()),
        block_end: None,
    };
    assert!(!tokens.has_block_comment());
    assert!(!tokens.has_any());
}

/// Simple registry for testing the trait.
struct TestRegistry {
    languages: Vec<LanguageInfo>,
}

impl TestRegistry {
    fn new(languages: Vec<LanguageInfo>) -> Self {
        Self { languages }
    }
}

impl LanguageRegistry for TestRegistry {
    fn detect_from_path(&self, path: &str) -> Option<String> {
        let ext = path.rsplit('.').next()?;
        self.languages
            .iter()
            .find(|info| info.matches_extension(ext))
            .map(|info| info.id.clone())
    }

    fn detect_from_mime(&self, mime: &str) -> Option<String> {
        self.languages
            .iter()
            .find(|info| info.matches_mime(mime))
            .map(|info| info.id.clone())
    }

    fn get_info(&self, language_id: &str) -> Option<&LanguageInfo> {
        self.languages.iter().find(|info| info.id == language_id)
    }

    fn is_registered(&self, language_id: &str) -> bool {
        self.languages.iter().any(|info| info.id == language_id)
    }

    fn language_ids(&self) -> Vec<String> {
        self.languages.iter().map(|info| info.id.clone()).collect()
    }
}

#[test]
fn test_language_registry_detect_from_path() {
    let registry = TestRegistry::new(vec![
        LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
        LanguageInfo::new("python", "Python").with_extensions(["py"]),
    ]);

    assert_eq!(registry.detect_from_path("main.rs"), Some("rust".to_string()));
    assert_eq!(registry.detect_from_path("script.py"), Some("python".to_string()));
    assert_eq!(registry.detect_from_path("file.txt"), None);
}

#[test]
fn test_language_registry_detect_from_mime() {
    let registry =
        TestRegistry::new(vec![LanguageInfo::new("rust", "Rust").with_mime_types(["text/x-rust"])]);

    assert_eq!(registry.detect_from_mime("text/x-rust"), Some("rust".to_string()));
    assert_eq!(registry.detect_from_mime("text/plain"), None);
}

#[test]
fn test_language_registry_get_info() {
    let registry = TestRegistry::new(vec![LanguageInfo::new("rust", "Rust")]);

    let info = registry.get_info("rust");
    assert!(info.is_some());
    assert_eq!(info.unwrap().name, "Rust");

    assert!(registry.get_info("unknown").is_none());
}

#[test]
fn test_language_registry_is_registered() {
    let registry = TestRegistry::new(vec![LanguageInfo::new("rust", "Rust")]);

    assert!(registry.is_registered("rust"));
    assert!(!registry.is_registered("python"));
}

#[test]
fn test_language_registry_len_and_empty() {
    let empty = TestRegistry::new(vec![]);
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);

    let non_empty = TestRegistry::new(vec![LanguageInfo::new("rust", "Rust")]);
    assert!(!non_empty.is_empty());
    assert_eq!(non_empty.len(), 1);
}

// ========================================================================
// DefaultLanguageRegistry Tests
// ========================================================================

#[test]
fn test_default_registry_detect_from_path() {
    let registry = DefaultLanguageRegistry::new(vec![
        LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
        LanguageInfo::new("markdown", "Markdown").with_extensions(["md", "markdown"]),
    ]);

    assert_eq!(registry.detect_from_path("main.rs"), Some("rust".to_string()));
    assert_eq!(registry.detect_from_path("README.md"), Some("markdown".to_string()));
    assert_eq!(registry.detect_from_path("notes.markdown"), Some("markdown".to_string()));
    assert_eq!(registry.detect_from_path("file.txt"), None);
}

#[test]
fn test_default_registry_case_insensitive_extension() {
    let registry = DefaultLanguageRegistry::new(vec![
        LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
    ]);

    assert_eq!(registry.detect_from_path("MAIN.RS"), Some("rust".to_string()));
    assert_eq!(registry.detect_from_path("lib.Rs"), Some("rust".to_string()));
}

#[test]
fn test_default_registry_detect_from_path_no_extension() {
    let registry = DefaultLanguageRegistry::new(vec![
        LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
    ]);

    assert_eq!(registry.detect_from_path("Makefile"), None);
    assert_eq!(registry.detect_from_path(""), None);
}

#[test]
fn test_default_registry_detect_from_path_with_directories() {
    let registry = DefaultLanguageRegistry::new(vec![
        LanguageInfo::new("rust", "Rust").with_extensions(["rs"]),
    ]);

    assert_eq!(
        registry.detect_from_path("/home/user/project/src/main.rs"),
        Some("rust".to_string())
    );
    assert_eq!(registry.detect_from_path("src/lib.rs"), Some("rust".to_string()));
}

#[test]
fn test_default_registry_detect_from_mime() {
    let registry = DefaultLanguageRegistry::new(vec![
        LanguageInfo::new("rust", "Rust").with_mime_types(["text/x-rust"]),
    ]);

    assert_eq!(registry.detect_from_mime("text/x-rust"), Some("rust".to_string()));
    assert_eq!(registry.detect_from_mime("TEXT/X-RUST"), Some("rust".to_string()));
    assert_eq!(registry.detect_from_mime("text/plain"), None);
}

#[test]
fn test_default_registry_get_info() {
    let registry = DefaultLanguageRegistry::new(vec![
        LanguageInfo::new("rust", "Rust")
            .with_extensions(["rs"])
            .with_comments(CommentTokens::with_block("//", "/*", "*/")),
    ]);

    let info = registry.get_info("rust").unwrap();
    assert_eq!(info.id, "rust");
    assert_eq!(info.name, "Rust");
    assert!(info.has_comments());

    assert!(registry.get_info("python").is_none());
}

#[test]
fn test_default_registry_is_registered() {
    let registry = DefaultLanguageRegistry::new(vec![LanguageInfo::new("rust", "Rust")]);

    assert!(registry.is_registered("rust"));
    assert!(!registry.is_registered("python"));
}

#[test]
fn test_default_registry_language_ids() {
    let registry = DefaultLanguageRegistry::new(vec![
        LanguageInfo::new("rust", "Rust"),
        LanguageInfo::new("markdown", "Markdown"),
    ]);

    let ids = registry.language_ids();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"rust".to_string()));
    assert!(ids.contains(&"markdown".to_string()));
}

#[test]
fn test_default_registry_len_and_empty() {
    let empty = DefaultLanguageRegistry::new(vec![]);
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);

    let non_empty = DefaultLanguageRegistry::new(vec![LanguageInfo::new("rust", "Rust")]);
    assert!(!non_empty.is_empty());
    assert_eq!(non_empty.len(), 1);
}

#[test]
fn test_default_registry_debug() {
    let registry = DefaultLanguageRegistry::new(vec![LanguageInfo::new("rust", "Rust")]);
    let debug = format!("{registry:?}");
    assert!(debug.contains("DefaultLanguageRegistry"));
    assert!(debug.contains("rust"));
}
