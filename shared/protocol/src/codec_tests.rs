use {super::*, crate::v1::types::Position};

#[test]
fn test_roundtrip() {
    let pos = Position::new(10, 5);
    let json = to_json(&pos).unwrap();
    let decoded: Position = from_json(&json).unwrap();
    assert_eq!(pos, decoded);
}

#[test]
fn test_to_value() {
    let pos = Position::new(10, 5);
    let value = to_value(&pos).unwrap();
    assert_eq!(value["line"], 10);
    assert_eq!(value["column"], 5);
}

#[test]
fn test_from_value() {
    let value = serde_json::json!({"line": 10, "column": 5});
    let pos: Position = from_value(value).unwrap();
    assert_eq!(pos.line, 10);
    assert_eq!(pos.column, 5);
}

#[test]
fn test_to_json_pretty() {
    let pos = Position::new(3, 7);
    let pretty = to_json_pretty(&pos).unwrap();
    assert!(pretty.contains('\n'));
    assert!(pretty.contains("\"line\": 3"));
    assert!(pretty.contains("\"column\": 7"));
}

#[test]
fn test_from_json_error() {
    let result: Result<Position, _> = from_json("not json");
    assert!(result.is_err());
}

#[test]
fn test_from_value_error() {
    let value = serde_json::json!("not a position");
    let result: Result<Position, _> = from_value(value);
    assert!(result.is_err());
}

#[test]
fn test_to_json_empty_struct() {
    let val = serde_json::json!({});
    let json = to_json(&val).unwrap();
    assert_eq!(json, "{}");
}

#[test]
fn test_to_value_and_back() {
    let pos = Position::new(0, 0);
    let value = to_value(&pos).unwrap();
    let decoded: Position = from_value(value).unwrap();
    assert_eq!(decoded, pos);
}
