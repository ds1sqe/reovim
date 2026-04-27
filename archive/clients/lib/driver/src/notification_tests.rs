use super::{parse_notification, parse_notification_field};

#[derive(serde::Deserialize, Debug, PartialEq)]
struct SmokePayload {
    flag: bool,
}

#[test]
fn re_export_resolves_parse_notification() {
    let payload: Option<SmokePayload> = parse_notification(r#"{"flag":true}"#);
    assert_eq!(payload, Some(SmokePayload { flag: true }));
}

#[test]
fn re_export_resolves_parse_notification_field() {
    let value: Option<bool> = parse_notification_field(r#"{"flag":true}"#, "flag");
    assert_eq!(value, Some(true));
}
