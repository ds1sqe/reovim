use reovim_kernel_interface::{
    Error, Outcome, ResourceId, Value,
    caller::{Call, Expected, Operation, Status},
    resource::AccessId,
};
use reovim_service::protocol::{Action, Frame, Reply};
use std::rc::Rc;

const TOKEN: [u8; 16] = [0x12; 16];

#[test]
fn independent_call_fields_keep_full_ids_and_reference_order() {
    let line = concat!(
        "v1\t12121212121212121212121212121212\tcall\t",
        "9007199254740993\t11\t27\t2\t",
        "34343434343434343434343434343434\t41\t18446744073709551615\t",
        "0000000001000000\t9,7"
    );
    let expected = Frame {
        caller: TOKEN,
        action: Action::Call(Call {
            request: 9_007_199_254_740_993,
            operation: Operation::Ioctl,
            subject: AccessId(27),
            command: 2,
            expected: Some(Expected {
                subject: ResourceId { epoch: [0x34; 16], serial: 41 },
                revision: u64::MAX,
            }),
            input: Value::copy(
                &[0, 0, 0, 0, 1, 0, 0, 0],
                &[AccessId(9), AccessId(7)],
            )
            .unwrap(),
        }),
    };
    assert_eq!(Frame::decode(line), Ok(expected.clone()));
    assert_eq!(expected.encode().unwrap(), line);
}

#[test]
fn rejects_retired_or_ambiguous_calls_before_dispatch() {
    for body in [
        "call\t1\twrite\t@27\tsource.splice\t1",
        "call\t01\t10\t27\t0\t-\t0\t0\t-\t-",
        "call\t1\t10\t27\t2\t-\t0\t0\t-\t-",
        "call\t1\t11\t27\t0\t-\t0\t0\t-\t-",
        "call\t1\t10\t27\t0\t-\t0\t1\t-\t-",
        "call\t1\t10\t27\t0\t-\t0\t0\tFF\t-",
        "call\t1\t10\t27\t0\t-\t0\t0\t-\t0",
        "result\t0",
        "reserve\textra",
        "reserve",
        "reserve\t0",
        "reserve\t01",
    ] {
        let line = format!("v1\t{}\t{body}", reovim_service::hex(&TOKEN));
        assert!(Frame::decode(&line).is_err(), "accepted {line}");
    }
}

#[test]
fn unknown_progress_preserves_error_identity_without_returning_access() {
    let line = concat!(
        "unknown\t4294967295\t",
        "34343434343434343434343434343434\t41\t1\t2e00000000000000"
    );
    let reply = Reply::decode(line).unwrap();
    let Reply::Status(Status::Outcome(outcome)) = &reply else {
        panic!("expected outcome")
    };
    assert_eq!(
        outcome.as_ref(),
        &Outcome::Unknown {
            error: Error::with_id(
                u32::MAX,
                ResourceId { epoch: [0x34; 16], serial: 41 }
            ),
            progress: Some(Value::copy(&46_u64.to_le_bytes(), &[]).unwrap()),
        }
    );
    assert_eq!(reply.encode().unwrap(), line);
    let invalid = Reply::Status(Status::Outcome(Rc::new(Outcome::Unknown {
        error: Error::PROVIDER,
        progress: Some(Value::copy(&[0; 4], &[AccessId(1)]).unwrap()),
    })));
    assert_eq!(invalid.encode(), Err(Error::INVALID));
}

#[test]
fn absent_progress_is_distinct_from_present_unit() {
    let absent = Reply::decode("unknown\t11\t-\t0\t0\t-").unwrap();
    let present = Reply::decode("unknown\t11\t-\t0\t1\t-").unwrap();
    assert_ne!(absent, present);
    assert_eq!(absent.encode().unwrap(), "unknown\t11\t-\t0\t0\t-");
    assert_eq!(present.encode().unwrap(), "unknown\t11\t-\t0\t1\t-");
    assert!(Reply::decode("unknown\t11\t-\t0\t0\t01").is_err());
    assert!(Reply::decode("failed\t4294967296\t-\t0").is_err());
    assert!(Reply::decode("failed\t0\t-\t0").is_err());
}

#[test]
fn frame_bound_precedes_parsing_and_encoding() {
    let oversized = "x".repeat(reovim_service::MAX_LINE);
    assert_eq!(Frame::decode(&oversized), Err(Error::PRESSURE));
    let value = Value::copy(oversized.as_bytes(), &[]).unwrap();
    let reply = Reply::Status(Status::Outcome(Rc::new(Outcome::Ok(value))));
    assert_eq!(reply.encode(), Err(Error::PRESSURE));
}

#[test]
fn reservation_sequence_is_exact_and_distinct_from_the_allocated_id() {
    let frame = Frame { caller: TOKEN, action: Action::Reserve(u64::MAX) };
    let encoded = frame.encode().unwrap();
    assert!(encoded.ends_with("\treserve\t18446744073709551615"));
    assert_eq!(Frame::decode(&encoded), Ok(frame));
    assert_eq!(
        Frame { caller: TOKEN, action: Action::Reserve(0) }.encode(),
        Err(Error::INVALID)
    );
}
