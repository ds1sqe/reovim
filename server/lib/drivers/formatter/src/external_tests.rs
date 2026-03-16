use std::{path::Path, time::Duration};

use {
    super::*,
    crate::{config::FormatterConfig, error::FormatError},
};

#[test]
fn test_format_with_echo() {
    let config = FormatterConfig::stdin("cat", vec![]);
    let fmt = ExternalFormatter::new(config);

    let result = fmt.format("hello world\n", Path::new("test.txt")).unwrap();
    assert_eq!(result, "hello world\n");
}

#[test]
fn test_format_transforms_content() {
    let config = FormatterConfig::stdin("tr", vec!["a-z".into(), "A-Z".into()]);
    let fmt = ExternalFormatter::new(config);

    let result = fmt.format("hello", Path::new("test.txt")).unwrap();
    assert_eq!(result, "HELLO");
}

#[test]
fn test_command_not_found() {
    let config = FormatterConfig::stdin("nonexistent_formatter_xyz_123", vec![]);
    let fmt = ExternalFormatter::new(config);

    let result = fmt.format("hello", Path::new("test.txt"));
    assert!(result.is_err());
    if let Err(FormatError::CommandNotFound(cmd)) = result {
        assert_eq!(cmd, "nonexistent_formatter_xyz_123");
    } else {
        panic!("Expected CommandNotFound");
    }
}

#[test]
fn test_command_failure() {
    let config = FormatterConfig::stdin("false", vec![]);
    let fmt = ExternalFormatter::new(config);

    let result = fmt.format("hello", Path::new("test.txt"));
    assert!(result.is_err());
    if let Err(FormatError::CommandFailed { exit_code, .. }) = result {
        assert_eq!(exit_code, Some(1));
    } else {
        panic!("Expected CommandFailed");
    }
}

#[test]
fn test_name() {
    let config = FormatterConfig::stdin("rustfmt", vec![]);
    let fmt = ExternalFormatter::new(config);
    assert_eq!(fmt.name(), "rustfmt");
}

#[test]
fn test_with_timeout() {
    let config = FormatterConfig::stdin("cat", vec![]);
    let fmt = ExternalFormatter::new(config).with_timeout(Duration::from_secs(10));
    assert_eq!(fmt.timeout, Duration::from_secs(10));
}

#[test]
fn test_path_substitution() {
    let config = FormatterConfig {
        command: "echo".to_string(),
        args: vec!["{path}".to_string()],
        stdin: false,
    };
    let fmt = ExternalFormatter::new(config);

    let result = fmt.format("", Path::new("/tmp/test.rs")).unwrap();
    assert!(result.contains("/tmp/test.rs"));
}

#[test]
fn test_timeout_fires() {
    // "sleep 10" should be killed by the 100ms timeout
    let config = FormatterConfig::stdin("sleep", vec!["10".into()]);
    let fmt = ExternalFormatter::new(config).with_timeout(Duration::from_millis(100));

    let result = fmt.format("", Path::new("test.txt"));
    assert!(result.is_err());
    assert!(matches!(result, Err(FormatError::Timeout)));
}

#[test]
fn test_supports_range_is_false() {
    let config = FormatterConfig::stdin("cat", vec![]);
    let fmt = ExternalFormatter::new(config);
    assert!(!fmt.supports_range());
}
