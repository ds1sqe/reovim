use super::{parse_notification, parse_notification_field};

#[derive(serde::Deserialize, Debug, PartialEq)]
struct TestPayload {
    active: bool,
    name: String,
}

#[test]
fn parse_valid_notification() {
    let data = r#"{"active": true, "name": "test"}"#;
    let result = parse_notification::<TestPayload>(data);
    assert_eq!(
        result,
        Some(TestPayload {
            active: true,
            name: "test".to_string(),
        })
    );
}

#[test]
fn parse_invalid_json_returns_none() {
    let result = parse_notification::<TestPayload>("not json");
    assert!(result.is_none());
}

#[test]
fn parse_wrong_schema_returns_none() {
    let data = r#"{"foo": 42}"#;
    let result = parse_notification::<TestPayload>(data);
    assert!(result.is_none());
}

#[test]
fn parse_empty_string_returns_none() {
    let result = parse_notification::<TestPayload>("");
    assert!(result.is_none());
}

#[test]
fn parse_field_string() {
    let data = r#"{"mode": "normal", "line": 42}"#;
    let result = parse_notification_field::<String>(data, "mode");
    assert_eq!(result, Some("normal".to_string()));
}

#[test]
fn parse_field_number() {
    let data = r#"{"mode": "normal", "line": 42}"#;
    let result = parse_notification_field::<i64>(data, "line");
    assert_eq!(result, Some(42));
}

#[test]
fn parse_field_missing_returns_none() {
    let data = r#"{"mode": "normal"}"#;
    let result = parse_notification_field::<String>(data, "missing");
    assert!(result.is_none());
}

#[test]
fn parse_field_wrong_type_returns_none() {
    let data = r#"{"mode": "normal"}"#;
    let result = parse_notification_field::<i64>(data, "mode");
    assert!(result.is_none());
}

#[test]
fn parse_field_invalid_json_returns_none() {
    let result = parse_notification_field::<String>("not json", "field");
    assert!(result.is_none());
}

#[derive(serde::Deserialize, Debug, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}

#[test]
fn parse_field_nested_object() {
    let data = r#"{"data": {"x": 1, "y": 2}}"#;
    let result = parse_notification_field::<Point>(data, "data");
    assert_eq!(result, Some(Point { x: 1, y: 2 }));
}

#[test]
fn parse_field_bool() {
    let data = r#"{"active": true}"#;
    let result = parse_notification_field::<bool>(data, "active");
    assert_eq!(result, Some(true));
}
