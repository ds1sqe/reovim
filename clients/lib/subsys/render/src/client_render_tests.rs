use super::*;

#[test]
fn probe_stores_kind_and_name_null_terminated() {
    let p = ClientRenderDriverProbe::new("tui", "TUI Render");
    assert_eq!(&p.kind[..3], b"tui");
    assert_eq!(p.kind[3], 0);
    assert_eq!(&p.name[..10], b"TUI Render");
    assert_eq!(p.name[10], 0);
}

#[test]
fn probe_truncates_oversized_inputs() {
    let long_kind = "x".repeat(200);
    let long_name = "n".repeat(300);
    let p = ClientRenderDriverProbe::new(&long_kind, &long_name);
    assert_eq!(p.kind[62], b'x');
    assert_eq!(p.kind[63], 0);
    assert_eq!(p.name[126], b'n');
    assert_eq!(p.name[127], 0);
}

#[test]
fn error_display_writes_inner_message() {
    let e = ClientRenderError("boom".to_owned());
    assert_eq!(format!("{e}"), "boom");
}

#[test]
fn error_is_std_error() {
    fn assert_err<E: std::error::Error>(_: &E) {}
    let e = ClientRenderError("x".to_owned());
    assert_err(&e);
}
