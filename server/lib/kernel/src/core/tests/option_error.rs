use super::*;

// ========== OptionError Display tests ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_error_not_found_display() {
    let err = OptionError::NotFound("tabwidth".to_string());
    assert_eq!(format!("{err}"), "option not found: tabwidth");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_error_already_exists_display() {
    let err = OptionError::AlreadyExists("number".to_string());
    assert_eq!(format!("{err}"), "option already exists: number");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_error_validation_failed_display() {
    let err = OptionError::ValidationFailed {
        name: "tabwidth".to_string(),
        reason: "must be positive".to_string(),
    };
    assert_eq!(format!("{err}"), "validation failed for 'tabwidth': must be positive");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_error_type_mismatch_display() {
    let err = OptionError::TypeMismatch {
        name: "number".to_string(),
        expected: "bool",
        got: "integer",
    };
    assert_eq!(format!("{err}"), "type mismatch for 'number': expected bool, got integer");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_error_alias_conflict_display() {
    let err = OptionError::AliasConflict("nu".to_string());
    assert_eq!(format!("{err}"), "alias conflicts with existing name: nu");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_error_scope_mismatch_display() {
    let err = OptionError::ScopeMismatch {
        name: "number".to_string(),
        option_scope: OptionScope::Global,
        requested: OptionScopeId::Buffer(crate::mm::BufferId::from_raw(1)),
    };
    let display = format!("{err}");
    assert!(display.contains("scope mismatch for 'number'"));
    assert!(display.contains("global"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_error_debug() {
    let err = OptionError::NotFound("test".to_string());
    let debug = format!("{err:?}");
    assert!(debug.contains("NotFound"));
    assert!(debug.contains("test"));
}

#[test]
fn test_option_error_clone() {
    let err = OptionError::NotFound("test".to_string());
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn test_option_error_eq() {
    let err1 = OptionError::NotFound("a".to_string());
    let err2 = OptionError::NotFound("a".to_string());
    let err3 = OptionError::NotFound("b".to_string());
    assert_eq!(err1, err2);
    assert_ne!(err1, err3);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_option_error_is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(OptionError::NotFound("test".to_string()));
    let _ = format!("{err}");
}

// ========== SetResult tests ==========

#[test]
fn test_set_result_with_old_value() {
    let result = SetResult {
        old_value: Some(OptionValue::bool(false)),
        new_value: OptionValue::bool(true),
    };
    assert_eq!(result.old_value, Some(OptionValue::bool(false)));
    assert_eq!(result.new_value, OptionValue::bool(true));
}

#[test]
fn test_set_result_without_old_value() {
    let result = SetResult {
        old_value: None,
        new_value: OptionValue::int(4),
    };
    assert!(result.old_value.is_none());
    assert_eq!(result.new_value, OptionValue::int(4));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_set_result_debug() {
    let result = SetResult {
        old_value: None,
        new_value: OptionValue::bool(true),
    };
    let debug = format!("{result:?}");
    assert!(debug.contains("SetResult"));
    assert!(debug.contains("old_value"));
    assert!(debug.contains("new_value"));
}

#[test]
fn test_set_result_clone() {
    let result = SetResult {
        old_value: Some(OptionValue::string("old")),
        new_value: OptionValue::string("new"),
    };
    let cloned = result.clone();
    assert_eq!(cloned.old_value, result.old_value);
    assert_eq!(cloned.new_value, result.new_value);
}
