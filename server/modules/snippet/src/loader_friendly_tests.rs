use {
    super::*,
    std::{
        io::Write,
        sync::atomic::{AtomicU32, Ordering},
    },
};

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn make_temp_dir() -> TempDir {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("reovim-friendly-test-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    TempDir(dir)
}

fn write_snippet_file(dir: &Path, name: &str, content: &str) {
    let snippets_dir = dir.join("snippets");
    std::fs::create_dir_all(&snippets_dir).unwrap();
    let mut f = std::fs::File::create(snippets_dir.join(name)).unwrap();
    write!(f, "{content}").unwrap();
}

fn write_manifest(dir: &Path, content: &str) {
    let mut f = std::fs::File::create(dir.join("package.json")).unwrap();
    write!(f, "{content}").unwrap();
}

#[test]
fn test_load_manifest_single_language() {
    let dir = make_temp_dir();
    write_snippet_file(
        dir.path(),
        "rust.json",
        r#"{ "function": { "prefix": "fn", "body": "fn $1() {}" } }"#,
    );
    write_manifest(
        dir.path(),
        r#"{
            "contributes": {
                "snippets": [
                    { "language": "rust", "path": "./snippets/rust.json" }
                ]
            }
        }"#,
    );

    let provider = FriendlySnippetsProvider::load(dir.path()).unwrap();
    let snippets = provider.snippets_for_filetype("rust");
    assert_eq!(snippets.len(), 1);
    assert_eq!(snippets[0].prefix, "fn");
}

#[test]
fn test_load_manifest_multiple_languages() {
    let dir = make_temp_dir();
    write_snippet_file(
        dir.path(),
        "js.json",
        r#"{ "log": { "prefix": "log", "body": "console.log($1)" } }"#,
    );
    write_manifest(
        dir.path(),
        r#"{
            "contributes": {
                "snippets": [
                    { "language": ["javascript", "typescript"], "path": "./snippets/js.json" }
                ]
            }
        }"#,
    );

    let provider = FriendlySnippetsProvider::load(dir.path()).unwrap();
    assert_eq!(provider.snippets_for_filetype("javascript").len(), 1);
    assert_eq!(provider.snippets_for_filetype("typescript").len(), 1);
}

#[test]
fn test_load_manifest_missing_snippet_file_skipped() {
    let dir = make_temp_dir();
    write_manifest(
        dir.path(),
        r#"{
            "contributes": {
                "snippets": [
                    { "language": "rust", "path": "./snippets/nonexistent.json" }
                ]
            }
        }"#,
    );

    let provider = FriendlySnippetsProvider::load(dir.path()).unwrap();
    assert!(provider.snippets_for_filetype("rust").is_empty());
}

#[test]
fn test_load_manifest_missing_package_json_errors() {
    let dir = make_temp_dir();
    let result = FriendlySnippetsProvider::load(dir.path());
    assert!(result.is_err());
}

#[test]
fn test_load_manifest_no_contributes() {
    let dir = make_temp_dir();
    write_manifest(dir.path(), "{}");

    let provider = FriendlySnippetsProvider::load(dir.path()).unwrap();
    assert!(provider.snippets_for_filetype("rust").is_empty());
}

#[test]
fn test_load_manifest_no_snippets_key() {
    let dir = make_temp_dir();
    write_manifest(dir.path(), r#"{ "contributes": {} }"#);

    let provider = FriendlySnippetsProvider::load(dir.path()).unwrap();
    assert!(provider.snippets_for_filetype("rust").is_empty());
}

#[test]
fn test_snippet_by_prefix() {
    let dir = make_temp_dir();
    write_snippet_file(
        dir.path(),
        "rust.json",
        r#"{
            "function": { "prefix": "fn", "body": "fn $1() {}" },
            "struct": { "prefix": "st", "body": "struct $1 {}" }
        }"#,
    );
    write_manifest(
        dir.path(),
        r#"{
            "contributes": {
                "snippets": [
                    { "language": "rust", "path": "./snippets/rust.json" }
                ]
            }
        }"#,
    );

    let provider = FriendlySnippetsProvider::load(dir.path()).unwrap();
    assert!(provider.snippet_by_prefix("rust", "fn").is_some());
    assert!(provider.snippet_by_prefix("rust", "st").is_some());
    assert!(provider.snippet_by_prefix("rust", "xyz").is_none());
    assert!(provider.snippet_by_prefix("python", "fn").is_none());
}

#[test]
fn test_language_value_single() {
    let val = LanguageValue::Single("rust".to_string());
    assert_eq!(val.into_vec(), vec!["rust"]);
}

#[test]
fn test_language_value_multiple() {
    let val = LanguageValue::Multiple(vec!["js".to_string(), "ts".to_string()]);
    assert_eq!(val.into_vec(), vec!["js", "ts"]);
}
