use super::*;

#[test]
fn test_option_spec_builder() {
    let spec = OptionSpec::new("number", "Show line numbers", OptionValue::bool(false))
        .with_short("nu")
        .with_scope(OptionScope::Window);

    assert_eq!(spec.name, "number");
    assert_eq!(spec.short_form.as_deref(), Some("nu"));
    assert_eq!(spec.scope, OptionScope::Window);
}

#[test]
fn test_option_spec_matches_name() {
    let spec = OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu");

    assert!(spec.matches_name("number"));
    assert!(spec.matches_name("nu"));
    assert!(!spec.matches_name("other"));
}

#[test]
fn test_option_spec_validate() {
    let spec = OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4))
        .with_constraint(OptionConstraint::range(1, 32));

    assert!(spec.validate(&OptionValue::int(4)).is_ok());
    assert!(spec.validate(&OptionValue::int(1)).is_ok());
    assert!(spec.validate(&OptionValue::int(32)).is_ok());
    assert!(spec.validate(&OptionValue::int(0)).is_err());
    assert!(spec.validate(&OptionValue::int(33)).is_err());
    assert!(spec.validate(&OptionValue::bool(true)).is_err()); // Type mismatch
}
