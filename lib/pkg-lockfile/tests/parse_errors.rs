//! Error-surface tests: malformed lockfile inputs hit distinct
//! [`LockfileError`] variants.

use reovim_pkg_lockfile::{Lockfile, LockfileError};

#[test]
fn missing_version_fails_to_parse() {
    let src = r#"
[[package]]
name = "x"
version = "1.0.0"
[package.source]
kind = "local-path"
path = "/x"
"#;
    let err = Lockfile::from_toml_str(src).expect_err("missing version must fail");
    assert!(matches!(err, LockfileError::TomlDe(_)));
}

#[test]
fn sha256_too_short_errors() {
    let src = r#"
version = 1

[[package]]
name = "x"
version = "1.0.0"
sha256 = "deadbeef"

[package.source]
kind = "local-path"
path = "/x"
"#;
    let err = Lockfile::from_toml_str(src).expect_err("short sha must fail");
    match err {
        LockfileError::InvalidSha256 { name, value } => {
            assert_eq!(name, "x");
            assert_eq!(value, "deadbeef");
        }
        other => panic!("expected InvalidSha256, got {other:?}"),
    }
}

#[test]
fn sha256_bad_charset_errors() {
    let src = r#"
version = 1

[[package]]
name = "x"
version = "1.0.0"
sha256 = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789"

[package.source]
kind = "local-path"
path = "/x"
"#;
    // Uppercase hex is not accepted; the field must be lowercase.
    let err = Lockfile::from_toml_str(src).expect_err("uppercase sha must fail");
    assert!(matches!(err, LockfileError::InvalidSha256 { .. }));
}

#[test]
fn unknown_source_kind_fails_to_parse() {
    let src = r#"
version = 1

[[package]]
name = "x"
version = "1.0.0"

[package.source]
kind = "http"
url = "https://example"
"#;
    let err = Lockfile::from_toml_str(src).expect_err("unknown kind must fail");
    assert!(matches!(err, LockfileError::TomlDe(_)));
}

#[test]
fn source_kind_local_path_requires_path() {
    let src = r#"
version = 1

[[package]]
name = "x"
version = "1.0.0"

[package.source]
kind = "local-path"
url = "https://example"
"#;
    // `kind = "local-path"` demands `path`, rejects `url`.
    let err = Lockfile::from_toml_str(src).expect_err("missing path must fail");
    assert!(matches!(err, LockfileError::TomlDe(_)));
}

#[test]
fn source_kind_registry_requires_url() {
    let src = r#"
version = 1

[[package]]
name = "x"
version = "1.0.0"

[package.source]
kind = "registry"
path = "/x"
"#;
    let err = Lockfile::from_toml_str(src).expect_err("missing url must fail");
    assert!(matches!(err, LockfileError::TomlDe(_)));
}

#[test]
fn bad_dependencies_field_fails_to_parse() {
    let src = r#"
version = 1

[[package]]
name = "x"
version = "1.0.0"
dependencies = "not an array"

[package.source]
kind = "local-path"
path = "/x"
"#;
    let err = Lockfile::from_toml_str(src).expect_err("bad deps must fail");
    assert!(matches!(err, LockfileError::TomlDe(_)));
}

#[test]
fn error_display_mentions_package_name() {
    let src = r#"
version = 1

[[package]]
name = "the-broken-one"
version = "1.0.0"
sha256 = "short"

[package.source]
kind = "local-path"
path = "/x"
"#;
    let err = Lockfile::from_toml_str(src).expect_err("short sha must fail");
    let msg = err.to_string();
    assert!(msg.contains("the-broken-one"), "error should name the offending package: {msg}");
}
