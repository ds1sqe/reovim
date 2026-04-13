use super::*;

#[test]
fn test_stdin_constructor() {
    let config = FormatterConfig::stdin("rustfmt", vec!["--edition".into(), "2024".into()]);
    assert_eq!(config.command, "rustfmt");
    assert_eq!(config.args, vec!["--edition", "2024"]);
    assert!(config.stdin);
}

#[test]
fn test_debug() {
    let config = FormatterConfig::stdin("black", vec!["-".into()]);
    let debug = format!("{config:?}");
    assert!(debug.contains("black"));
}

#[test]
fn test_clone() {
    let config = FormatterConfig::stdin("gofmt", vec![]);
    let cloned = config.clone();
    assert_eq!(config, cloned);
}

#[test]
fn test_eq() {
    let a = FormatterConfig::stdin("rustfmt", vec![]);
    let b = FormatterConfig::stdin("rustfmt", vec![]);
    assert_eq!(a, b);
}

#[test]
fn test_ne() {
    let a = FormatterConfig::stdin("rustfmt", vec![]);
    let b = FormatterConfig::stdin("black", vec![]);
    assert_ne!(a, b);
}
