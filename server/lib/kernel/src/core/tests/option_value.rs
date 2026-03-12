use super::*;

#[test]
fn test_option_value_bool() {
    let value = OptionValue::bool(true);
    assert_eq!(value.as_bool(), Some(true));
    assert_eq!(value.as_int(), None);
    assert_eq!(value.as_str(), None);
    assert_eq!(value.type_name(), "bool");
}

#[test]
fn test_option_value_int() {
    let value = OptionValue::int(42);
    assert_eq!(value.as_int(), Some(42));
    assert_eq!(value.as_bool(), None);
    assert_eq!(value.type_name(), "integer");
}

#[test]
fn test_option_value_string() {
    let value = OptionValue::string("hello");
    assert_eq!(value.as_str(), Some("hello"));
    assert_eq!(value.as_bool(), None);
    assert_eq!(value.type_name(), "string");
}

#[test]
fn test_option_value_choice() {
    let value = OptionValue::choice("dark", vec!["light".into(), "dark".into()]);
    assert_eq!(value.as_str(), Some("dark"));
    assert_eq!(value.type_name(), "choice");
}

#[test]
fn test_option_value_same_type() {
    assert!(OptionValue::bool(true).same_type(&OptionValue::bool(false)));
    assert!(OptionValue::int(1).same_type(&OptionValue::int(2)));
    assert!(!OptionValue::bool(true).same_type(&OptionValue::int(1)));
}

#[test]
fn test_option_value_display() {
    assert_eq!(OptionValue::bool(true).to_string(), "true");
    assert_eq!(OptionValue::int(42).to_string(), "42");
    assert_eq!(OptionValue::string("hello").to_string(), "hello");
}
