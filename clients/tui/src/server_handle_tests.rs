use std::collections::HashMap;

use reovim_client_driver::OptionValue;
use reovim_protocol::v2::{self, option_value::Value};

use super::convert_options;

fn make_proto_option(value: Value) -> v2::OptionValue {
    v2::OptionValue {
        value: Some(value),
    }
}

#[test]
fn convert_options_bool() {
    let mut map = HashMap::new();
    map.insert("number".to_string(), make_proto_option(Value::BoolValue(true)));
    let result = convert_options(&map);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "number");
    assert_eq!(result[0].1, OptionValue::Bool(true));
}

#[test]
fn convert_options_int() {
    let mut map = HashMap::new();
    map.insert("tabstop".to_string(), make_proto_option(Value::IntValue(4)));
    let result = convert_options(&map);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "tabstop");
    assert_eq!(result[0].1, OptionValue::Integer(4));
}

#[test]
fn convert_options_string() {
    let mut map = HashMap::new();
    map.insert(
        "colorscheme".to_string(),
        make_proto_option(Value::StringValue("dark".to_string())),
    );
    let result = convert_options(&map);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "colorscheme");
    assert_eq!(result[0].1, OptionValue::String("dark".to_string()));
}

#[test]
fn convert_options_empty() {
    let map = HashMap::new();
    let result = convert_options(&map);
    assert!(result.is_empty());
}

#[test]
fn convert_options_none_value_skipped() {
    let mut map = HashMap::new();
    map.insert("broken".to_string(), v2::OptionValue { value: None });
    let result = convert_options(&map);
    assert!(result.is_empty());
}

#[test]
fn convert_options_mixed() {
    let mut map = HashMap::new();
    map.insert("flag".to_string(), make_proto_option(Value::BoolValue(false)));
    map.insert("count".to_string(), make_proto_option(Value::IntValue(42)));
    map.insert(
        "name".to_string(),
        make_proto_option(Value::StringValue("test".to_string())),
    );
    let result = convert_options(&map);
    assert_eq!(result.len(), 3);

    let by_name: HashMap<String, OptionValue> = result.into_iter().collect();
    assert_eq!(by_name["flag"], OptionValue::Bool(false));
    assert_eq!(by_name["count"], OptionValue::Integer(42));
    assert_eq!(by_name["name"], OptionValue::String("test".to_string()));
}
