use super::*;

#[test]
fn test_constraint_range() {
    let constraint = OptionConstraint::range(1, 10);

    // Valid values
    assert!(constraint.validate(&OptionValue::int(1)).is_ok());
    assert!(constraint.validate(&OptionValue::int(5)).is_ok());
    assert!(constraint.validate(&OptionValue::int(10)).is_ok());

    // Invalid values
    assert!(constraint.validate(&OptionValue::int(0)).is_err());
    assert!(constraint.validate(&OptionValue::int(11)).is_err());
}

#[test]
fn test_constraint_string_length() {
    let constraint = OptionConstraint::string_length(2, 5);

    assert!(constraint.validate(&OptionValue::string("ab")).is_ok());
    assert!(constraint.validate(&OptionValue::string("abcde")).is_ok());
    assert!(constraint.validate(&OptionValue::string("a")).is_err());
    assert!(constraint.validate(&OptionValue::string("abcdef")).is_err());
}

#[test]
fn test_constraint_choice() {
    let value = OptionValue::choice("valid", vec!["valid".into(), "other".into()]);
    let constraint = OptionConstraint::none();
    assert!(constraint.validate(&value).is_ok());
}

// === min/max constructors ===

#[test]
fn test_constraint_min_only() {
    let constraint = OptionConstraint::min(5);
    assert!(constraint.validate(&OptionValue::int(5)).is_ok());
    assert!(constraint.validate(&OptionValue::int(100)).is_ok());
    assert!(constraint.validate(&OptionValue::int(4)).is_err());
}

#[test]
fn test_constraint_max_only() {
    let constraint = OptionConstraint::max(10);
    assert!(constraint.validate(&OptionValue::int(10)).is_ok());
    assert!(constraint.validate(&OptionValue::int(0)).is_ok());
    assert!(constraint.validate(&OptionValue::int(11)).is_err());
}

// === ConstraintError Display coverage ===

#[test]
fn test_constraint_error_below_minimum_display() {
    let err = ConstraintError::BelowMinimum { value: 3, min: 5 };
    let msg = err.to_string();
    assert!(msg.contains('3'));
    assert!(msg.contains("below minimum"));
    assert!(msg.contains('5'));
}

#[test]
fn test_constraint_error_above_maximum_display() {
    let err = ConstraintError::AboveMaximum { value: 20, max: 10 };
    let msg = err.to_string();
    assert!(msg.contains("20"));
    assert!(msg.contains("above maximum"));
    assert!(msg.contains("10"));
}

#[test]
fn test_constraint_error_string_too_short_display() {
    let err = ConstraintError::StringTooShort { len: 1, min: 3 };
    let msg = err.to_string();
    assert!(msg.contains('1'));
    assert!(msg.contains("below minimum"));
    assert!(msg.contains('3'));
}

#[test]
fn test_constraint_error_string_too_long_display() {
    let err = ConstraintError::StringTooLong { len: 10, max: 5 };
    let msg = err.to_string();
    assert!(msg.contains("10"));
    assert!(msg.contains("above maximum"));
    assert!(msg.contains('5'));
}

#[test]
fn test_constraint_error_invalid_choice_display() {
    let err = ConstraintError::InvalidChoice {
        value: "bad".to_string(),
        choices: vec!["good".to_string(), "ok".to_string()],
    };
    let msg = err.to_string();
    assert!(msg.contains("bad"));
    assert!(msg.contains("not a valid choice"));
}

#[test]
fn test_constraint_error_is_std_error() {
    let err: Box<dyn std::error::Error> =
        Box::new(ConstraintError::BelowMinimum { value: 1, min: 5 });
    assert!(err.to_string().contains("below minimum"));
}

// === invalid choice validation ===

#[test]
fn test_constraint_invalid_choice_validation() {
    // Construct the Choice variant directly to bypass debug_assert in choice()
    let value = OptionValue::Choice {
        value: "invalid".into(),
        choices: vec!["a".into(), "b".into()],
    };
    let constraint = OptionConstraint::none();
    let result = constraint.validate(&value);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ConstraintError::InvalidChoice { .. }));
}

// === bool passthrough ===

#[test]
fn test_constraint_bool_passthrough() {
    // Range constraint should not affect booleans
    let constraint = OptionConstraint::range(1, 10);
    assert!(constraint.validate(&OptionValue::Bool(true)).is_ok());
    assert!(constraint.validate(&OptionValue::Bool(false)).is_ok());
}
