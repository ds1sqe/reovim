use super::*;

#[test]
fn test_log_config_default() {
    let config = LogConfig::default();
    assert_eq!(config.level, Level::Info);
    assert!(matches!(config.output, LogOutput::Stderr));
    assert!(matches!(config.format, LogFormat::Plain));
    assert!(config.file_path.is_none());
    assert!(matches!(config.rotation, RotationPolicy::Never));
}

#[test]
fn test_log_config_custom() {
    let config = LogConfig {
        level: Level::Debug,
        output: LogOutput::File,
        format: LogFormat::Json,
        file_path: Some(PathBuf::from("/tmp/reovim.log")),
        rotation: RotationPolicy::Daily,
    };

    assert_eq!(config.level, Level::Debug);
    assert!(matches!(config.output, LogOutput::File));
    assert!(matches!(config.format, LogFormat::Json));
    assert_eq!(config.file_path, Some(PathBuf::from("/tmp/reovim.log")));
    assert!(matches!(config.rotation, RotationPolicy::Daily));
}

#[test]
fn test_log_output_default() {
    let output = LogOutput::default();
    assert!(matches!(output, LogOutput::Stderr));
}

#[test]
fn test_log_format_default() {
    let format = LogFormat::default();
    assert!(matches!(format, LogFormat::Plain));
}

#[test]
fn test_rotation_policy_default() {
    let rotation = RotationPolicy::default();
    assert!(matches!(rotation, RotationPolicy::Never));
}

#[test]
fn test_log_config_clone() {
    let config = LogConfig {
        level: Level::Warn,
        output: LogOutput::Stdout,
        format: LogFormat::Pretty,
        file_path: None,
        rotation: RotationPolicy::Hourly,
    };

    // Clone and consume original to verify Clone actually works
    let cloned = config.clone();
    drop(config);

    // Verify cloned values
    assert_eq!(cloned.level, Level::Warn);
    assert!(matches!(cloned.output, LogOutput::Stdout));
    assert!(matches!(cloned.format, LogFormat::Pretty));
    assert!(matches!(cloned.rotation, RotationPolicy::Hourly));
}

#[test]
fn test_log_config_debug() {
    let config = LogConfig::default();
    let debug_str = format!("{config:?}");
    assert!(debug_str.contains("LogConfig"));
    assert!(debug_str.contains("Info"));
    assert!(debug_str.contains("Stderr"));
    assert!(debug_str.contains("Plain"));
    assert!(debug_str.contains("Never"));
}

#[test]
fn test_log_output_all_variants_debug() {
    let stderr = LogOutput::Stderr;
    assert!(format!("{stderr:?}").contains("Stderr"));

    let stdout = LogOutput::Stdout;
    assert!(format!("{stdout:?}").contains("Stdout"));

    let file = LogOutput::File;
    assert!(format!("{file:?}").contains("File"));
}

#[test]
fn test_log_format_all_variants_debug() {
    let plain = LogFormat::Plain;
    assert!(format!("{plain:?}").contains("Plain"));

    let json = LogFormat::Json;
    assert!(format!("{json:?}").contains("Json"));

    let pretty = LogFormat::Pretty;
    assert!(format!("{pretty:?}").contains("Pretty"));
}

#[test]
fn test_rotation_policy_all_variants_debug() {
    let never = RotationPolicy::Never;
    assert!(format!("{never:?}").contains("Never"));

    let daily = RotationPolicy::Daily;
    assert!(format!("{daily:?}").contains("Daily"));

    let hourly = RotationPolicy::Hourly;
    assert!(format!("{hourly:?}").contains("Hourly"));
}

#[test]
fn test_log_output_clone() {
    let original = LogOutput::Stdout;
    let cloned = original;
    assert!(matches!(cloned, LogOutput::Stdout));
}

#[test]
fn test_log_format_clone() {
    let original = LogFormat::Json;
    let cloned = original;
    assert!(matches!(cloned, LogFormat::Json));
}

#[test]
fn test_rotation_policy_clone() {
    let original = RotationPolicy::Daily;
    let cloned = original;
    assert!(matches!(cloned, RotationPolicy::Daily));
}

#[test]
fn test_log_config_with_file_path() {
    let config = LogConfig {
        level: Level::Trace,
        output: LogOutput::File,
        format: LogFormat::Json,
        file_path: Some(PathBuf::from("/var/log/reovim/editor.log")),
        rotation: RotationPolicy::Daily,
    };

    assert_eq!(config.level, Level::Trace);
    assert!(matches!(config.output, LogOutput::File));
    assert!(matches!(config.format, LogFormat::Json));
    assert_eq!(config.file_path, Some(PathBuf::from("/var/log/reovim/editor.log")));
    assert!(matches!(config.rotation, RotationPolicy::Daily));
}

#[test]
fn test_log_config_all_levels() {
    for level in Level::ALL {
        let config = LogConfig {
            level,
            ..Default::default()
        };
        assert_eq!(config.level, level);
    }
}

#[test]
fn test_log_config_clone_with_file_path() {
    let config = LogConfig {
        level: Level::Error,
        output: LogOutput::File,
        format: LogFormat::Plain,
        file_path: Some(PathBuf::from("/tmp/test.log")),
        rotation: RotationPolicy::Hourly,
    };

    let cloned = config;
    assert_eq!(cloned.file_path, Some(PathBuf::from("/tmp/test.log")));
    assert!(matches!(cloned.rotation, RotationPolicy::Hourly));
}
