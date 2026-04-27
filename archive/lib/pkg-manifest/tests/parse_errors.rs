//! Error-surface tests: five malformed inputs hit five distinct
//! [`ManifestError`] variants (or distinct TOML-deserialization
//! failures for schema-level misshapes).

use reovim_pkg_manifest::{Manifest, ManifestError};

#[test]
fn empty_input_fails_to_parse() {
    let err = Manifest::from_toml_str("").expect_err("empty TOML must fail");
    assert!(matches!(err, ManifestError::TomlDe(_)));
}

#[test]
fn missing_package_name_fails_to_parse() {
    let src = r#"
[package]
reovim-version = "^0.15"
"#;
    let err = Manifest::from_toml_str(src).expect_err("missing package.name must fail");
    assert!(matches!(err, ManifestError::TomlDe(_)));
}

#[test]
fn lazy_entry_without_trigger_errors() {
    let src = r#"
[package]
name = "x"
reovim-version = "^0.15"

[lazy]
vim-text = {}
"#;
    let err = Manifest::from_toml_str(src).expect_err("empty lazy entry must fail");
    match err {
        ManifestError::InvalidLazyTrigger { dep } => assert_eq!(dep, "vim-text"),
        other => panic!("expected InvalidLazyTrigger, got {other:?}"),
    }
}

#[test]
fn lazy_entry_with_two_triggers_errors() {
    let src = r#"
[package]
name = "x"
reovim-version = "^0.15"

[lazy]
vim-text = { on-domain = "text", on-event = "ready" }
"#;
    let err = Manifest::from_toml_str(src).expect_err("two triggers must fail");
    assert!(matches!(err, ManifestError::InvalidLazyTrigger { .. }));
}

#[test]
fn lazy_entry_with_eager_false_errors() {
    // `eager = false` is not a valid opt-out; load policy is explicit.
    let src = r#"
[package]
name = "x"
reovim-version = "^0.15"

[lazy]
vim-text = { eager = false }
"#;
    let err = Manifest::from_toml_str(src).expect_err("eager=false must fail");
    assert!(matches!(err, ManifestError::InvalidLazyTrigger { .. }));
}

#[test]
fn malformed_dependency_shape_fails_to_parse() {
    // `features` must be an array of strings; a single string fails.
    let src = r#"
[package]
name = "x"
reovim-version = "^0.15"

[dependencies]
bad-dep = { version = "1.0", features = "ripgrep" }
"#;
    let err = Manifest::from_toml_str(src).expect_err("features must be an array");
    assert!(matches!(err, ManifestError::TomlDe(_)));
}

#[test]
fn unknown_package_kind_fails_to_parse() {
    let src = r#"
[package]
name = "x"
kind = "both"
reovim-version = "^0.15"
"#;
    let err = Manifest::from_toml_str(src).expect_err("unknown kind must fail");
    assert!(matches!(err, ManifestError::TomlDe(_)));
}

#[test]
fn error_messages_are_user_facing() {
    // `ManifestError::InvalidLazyTrigger` exposes the offending dep
    // name in its Display output so callers can surface it to users
    // without unwrapping the variant.
    let src = r#"
[package]
name = "x"
reovim-version = "^0.15"

[lazy]
bad-entry = {}
"#;
    let err = Manifest::from_toml_str(src).expect_err("must fail");
    let msg = err.to_string();
    assert!(msg.contains("bad-entry"), "error Display should mention dep name, got: {msg}");
}

#[test]
fn parse_trigger_rejects_garbage() {
    use reovim_pkg_manifest::parse_trigger;
    let err = parse_trigger("not-a-real-trigger").expect_err("must fail");
    assert!(
        matches!(err, ManifestError::MalformedTrigger { ref value } if value == "not-a-real-trigger")
    );
    let msg = err.to_string();
    assert!(
        msg.contains("not-a-real-trigger"),
        "error Display should echo the bad value, got: {msg}"
    );
}

#[test]
fn parse_trigger_rejects_eager_with_payload() {
    use reovim_pkg_manifest::parse_trigger;
    let err = parse_trigger("eager:extra").expect_err("must fail");
    assert!(matches!(err, ManifestError::MalformedTrigger { .. }));
}
