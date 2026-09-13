use reovim_service::request::{Error, Request};

#[test]
fn restored_receipt_keeps_scope_and_delivery_state() {
    let client = "0123456789abcdef0123456789abcdef";
    let mut request = Request::new(client, u64::MAX, &["read".into()]).unwrap();
    request.send().unwrap();
    let mut restored = Request::restore(client, request.checkpoint()).unwrap();
    assert_eq!(restored.id(), u64::MAX);
    assert_eq!(restored.send(), Err(Error::State));
    assert_eq!(restored.recover(), request.recover());
    let mut invalid = request.checkpoint();
    invalid.adopted = true;
    assert!(Request::restore(client, invalid).is_err());
    let other = "abcdef0123456789abcdef0123456789";
    assert!(Request::restore(other, request.checkpoint()).is_err());
}

const CLIENT: &str = "0123456789abcdef0123456789abcdef";

fn request() -> Request {
    Request::new(
        CLIENT,
        9_007_199_254_740_993,
        &["write".into(), "@27".into(), "source.splice".into(), "1".into()],
    )
    .unwrap()
}

#[test]
fn lost_delivery_recovers_exact_identity_without_replay() {
    let mut request = request();
    assert_eq!(request.recover(), Err(Error::State));
    assert_eq!(
        request.send().unwrap(),
        concat!(
            "v1\t0123456789abcdef0123456789abcdef\tcall\t",
            "9007199254740993\twrite\t@27\tsource.splice\t1",
        ),
    );
    assert_eq!(request.send(), Err(Error::State));
    assert_eq!(
        request.recover().unwrap(),
        "v1\t0123456789abcdef0123456789abcdef\tresult\t9007199254740993",
    );
    assert_eq!(request.release(), Err(Error::State));
    request.record("ok\tsource.splice\t1\t0100\t@29".into()).unwrap();
    assert_eq!(request.release(), Err(Error::State));
    request.adopt().unwrap();
    let release = request.release().unwrap();
    assert_eq!(request.released("error\tbusy"), Err(Error::State));
    assert_eq!(request.release().unwrap(), release);
    request.released("ok").unwrap();
    assert_eq!(request.recover(), Err(Error::State));
    assert_eq!(request.response(), Some("ok\tsource.splice\t1\t0100\t@29"));
}

#[test]
fn unknown_effect_retains_recovery_and_cannot_be_adopted_as_failure() {
    let mut request = request();
    request.send().unwrap();
    let reply = "error\tunknown\t8\t@41\t7265636f766572";
    request.record(reply.into()).unwrap();
    assert_eq!(request.response(), Some(reply));
    assert_eq!(request.adopt(), Err(Error::Unknown));
    assert_eq!(request.release(), Err(Error::State));
    assert!(request.recover().unwrap().contains("\tresult\t"));
    assert_eq!(request.record("ok".into()), Err(Error::Invalid));
    assert_eq!(request.response(), Some(reply));
}

#[test]
fn malformed_control_reply_does_not_discard_delivery_custody() {
    let mut request = request();
    request.send().unwrap();
    for bad in [
        "error\tcontrol error",
        "error\tfailed\tbad\t\t",
        "error\tfailed\t8\t@bad\t",
        "error\tunknown\t8\t\tzz",
        "ok\n",
    ] {
        assert_eq!(request.record(bad.into()), Err(Error::Invalid));
        assert_eq!(request.response(), None);
        assert!(request.recover().is_ok());
    }
    request.record("error\trejected\t2\t\t".into()).unwrap();
    request.adopt().unwrap();
    assert!(request.release().is_ok());
}

#[test]
fn invalid_frame_never_becomes_a_transmittable_request() {
    for fields in [vec![], vec!["read\tother".into()], vec!["\n".into()]] {
        assert!(matches!(
            Request::new(CLIENT, 1, &fields),
            Err(Error::Invalid)
        ));
    }
    assert!(matches!(
        Request::new("not-a-client", 1, &["resolve".into()]),
        Err(Error::Invalid),
    ));
    assert!(matches!(
        Request::new(CLIENT, 1, &["x".repeat(reovim_service::MAX_LINE)]),
        Err(Error::Invalid),
    ));
}

#[test]
fn extensible_error_codes_keep_the_full_interface_width() {
    let mut request = request();
    request.send().unwrap();
    request.record("error\tfailed\t18446744073709551615\t\t".into()).unwrap();
    request.adopt().unwrap();
    assert!(request.release().is_ok());
}
