use reovim_kernel_interface::{
    Error, Value,
    caller::{Call, Operation},
    resource::AccessId,
};

fn executable() -> Call {
    Call {
        request: 1,
        operation: Operation::Exec,
        subject: AccessId(2),
        command: 0,
        expected: None,
        input: Value::unit(),
    }
}

#[test]
fn command_is_local_to_control() {
    let mut call = executable();
    assert_eq!(call.validate(), Ok(()));
    call.command = 1;
    assert_eq!(call.validate(), Err(Error::INVALID));
    call.operation = Operation::Ioctl;
    assert_eq!(call.validate(), Ok(()));
    call.command = 0;
    assert_eq!(call.validate(), Err(Error::INVALID));
}

#[test]
fn acquisition_alone_has_no_subject() {
    let mut call = executable();
    call.subject = AccessId(0);
    assert_eq!(call.validate(), Err(Error::INVALID));
    call.operation = Operation::Acquire;
    assert_eq!(call.validate(), Ok(()));
    call.subject = AccessId(2);
    assert_eq!(call.validate(), Err(Error::INVALID));
    call.subject = AccessId(0);
    call.request = 0;
    assert_eq!(call.validate(), Err(Error::INVALID));
}

#[test]
fn retries_compare_access_tokens_and_bytes() {
    let mut first = executable();
    first.input = Value::copy(&[0, 0, 0, 0], &[AccessId(3)]).unwrap();
    let mut retry = first.clone();
    assert_eq!(first, retry);
    retry.input.references[0] = AccessId(4);
    assert_ne!(first, retry);
    retry = first.clone();
    retry.input.data[0] = 1;
    assert_ne!(first, retry);
}

#[test]
fn common_codes_have_no_extension_escape() {
    assert_eq!(Operation::try_from(10), Ok(Operation::Exec));
    assert_eq!(Operation::try_from(11), Ok(Operation::Ioctl));
    assert_eq!(Operation::try_from(21), Ok(Operation::SetAccess));
    assert_eq!(Operation::try_from(0), Err(Error::UNSUPPORTED));
    assert_eq!(Operation::try_from(22), Err(Error::UNSUPPORTED));
    assert_eq!(Operation::try_from(u32::MAX), Err(Error::UNSUPPORTED));
}
