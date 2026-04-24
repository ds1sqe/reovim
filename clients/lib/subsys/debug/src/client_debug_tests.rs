use super::*;

#[test]
fn probe_stores_driver_name_and_description_null_terminated() {
    let p = DebugProbe::new("debug-poc", "Phase 0 debug-surface PoC", &[], &[]);
    assert_eq!(&p.driver_name[..9], b"debug-poc");
    assert_eq!(p.driver_name[9], 0);
    assert_eq!(&p.description[..25], b"Phase 0 debug-surface PoC");
    assert_eq!(p.description[25], 0);
}

#[test]
fn probe_truncates_oversized_driver_name_and_description() {
    let long_name = "x".repeat(200);
    let long_desc = "y".repeat(300);
    let p = DebugProbe::new(&long_name, &long_desc, &[], &[]);
    assert_eq!(p.driver_name[62], b'x');
    assert_eq!(p.driver_name[63], 0);
    assert_eq!(p.description[190], b'y');
    assert_eq!(p.description[191], 0);
}

#[test]
fn probe_records_observe_and_drive_schemas() {
    let p = DebugProbe::new("d", "desc", &["frames", "events"], &["echo"]);
    assert_eq!(p.observe_schemas_count, 2);
    assert_eq!(&p.observe_schemas[0][..6], b"frames");
    assert_eq!(p.observe_schemas[0][6], 0);
    assert_eq!(&p.observe_schemas[1][..6], b"events");
    assert_eq!(p.observe_schemas[1][6], 0);
    assert_eq!(p.drive_schemas_count, 1);
    assert_eq!(&p.drive_schemas[0][..4], b"echo");
    assert_eq!(p.drive_schemas[0][4], 0);
}

#[test]
fn probe_truncates_schema_list_to_max_schemas() {
    let many: Vec<&str> = (0..20).map(|_| "s").collect();
    let p = DebugProbe::new("d", "desc", &many, &many);
    assert_eq!(p.observe_schemas_count, u32::try_from(MAX_SCHEMAS).unwrap());
    assert_eq!(p.drive_schemas_count, u32::try_from(MAX_SCHEMAS).unwrap());
}

#[test]
fn probe_truncates_schema_name_to_slot_capacity() {
    let long = "s".repeat(200);
    let p = DebugProbe::new("d", "desc", &[long.as_str()], &[long.as_str()]);
    assert_eq!(p.observe_schemas[0][SCHEMA_NAME_LEN - 2], b's');
    assert_eq!(p.observe_schemas[0][SCHEMA_NAME_LEN - 1], 0);
    assert_eq!(p.drive_schemas[0][SCHEMA_NAME_LEN - 2], b's');
    assert_eq!(p.drive_schemas[0][SCHEMA_NAME_LEN - 1], 0);
}

#[test]
fn probe_all_zero_when_empty() {
    let p = DebugProbe::new("", "", &[], &[]);
    assert_eq!(p.driver_name[0], 0);
    assert_eq!(p.description[0], 0);
    assert_eq!(p.observe_schemas_count, 0);
    assert_eq!(p.drive_schemas_count, 0);
}

#[test]
fn error_display_writes_inner_message() {
    let e = DebugError("boom".to_owned());
    assert_eq!(format!("{e}"), "boom");
}

#[test]
fn error_is_std_error() {
    fn assert_err<E: std::error::Error>(_: &E) {}
    let e = DebugError("x".to_owned());
    assert_err(&e);
}
