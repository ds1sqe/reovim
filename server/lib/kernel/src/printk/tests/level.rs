use {super::super::*, std::str::FromStr};

#[test]
fn test_level_ordering() {
    // Error is most severe (lowest value)
    assert!(Level::Error < Level::Warn);
    assert!(Level::Warn < Level::Info);
    assert!(Level::Info < Level::Debug);
    assert!(Level::Debug < Level::Trace);
}

#[test]
fn test_level_as_str() {
    assert_eq!(Level::Error.as_str(), "ERROR");
    assert_eq!(Level::Warn.as_str(), "WARN");
    assert_eq!(Level::Info.as_str(), "INFO");
    assert_eq!(Level::Debug.as_str(), "DEBUG");
    assert_eq!(Level::Trace.as_str(), "TRACE");
}

#[test]
fn test_level_from_str() {
    // Exact matches
    assert_eq!(Level::from_str("error"), Ok(Level::Error));
    assert_eq!(Level::from_str("warn"), Ok(Level::Warn));
    assert_eq!(Level::from_str("info"), Ok(Level::Info));
    assert_eq!(Level::from_str("debug"), Ok(Level::Debug));
    assert_eq!(Level::from_str("trace"), Ok(Level::Trace));

    // Case insensitive
    assert_eq!(Level::from_str("ERROR"), Ok(Level::Error));
    assert_eq!(Level::from_str("Warn"), Ok(Level::Warn));
    assert_eq!(Level::from_str("INFO"), Ok(Level::Info));

    // Aliases
    assert_eq!(Level::from_str("err"), Ok(Level::Error));
    assert_eq!(Level::from_str("warning"), Ok(Level::Warn));

    // Unknown
    assert!(Level::from_str("unknown").is_err());
    assert!(Level::from_str("").is_err());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_level_display() {
    assert_eq!(format!("{}", Level::Error), "ERROR");
    assert_eq!(format!("{}", Level::Info), "INFO");
}

#[test]
fn test_level_is_at_least() {
    // Error is at least as severe as everything
    assert!(Level::Error.is_at_least(Level::Error));
    assert!(Level::Error.is_at_least(Level::Warn));
    assert!(Level::Error.is_at_least(Level::Info));
    assert!(Level::Error.is_at_least(Level::Debug));
    assert!(Level::Error.is_at_least(Level::Trace));

    // Info is at least as severe as Info, Debug, Trace
    assert!(Level::Info.is_at_least(Level::Info));
    assert!(Level::Info.is_at_least(Level::Debug));
    assert!(Level::Info.is_at_least(Level::Trace));
    assert!(!Level::Info.is_at_least(Level::Error));
    assert!(!Level::Info.is_at_least(Level::Warn));

    // Trace is only at least as severe as itself
    assert!(Level::Trace.is_at_least(Level::Trace));
    assert!(!Level::Trace.is_at_least(Level::Debug));
}

#[test]
fn test_level_default() {
    assert_eq!(Level::default(), Level::Info);
}

#[test]
fn test_level_all() {
    assert_eq!(Level::ALL.len(), 5);
    assert_eq!(Level::ALL[0], Level::Error);
    assert_eq!(Level::ALL[4], Level::Trace);
}

#[test]
fn test_parse_level_error() {
    let err = ParseLevelError;
    assert_eq!(format!("{err}"), "unknown log level");
}
