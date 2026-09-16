use super::*;
use reovim_kernel_interface::{Outcome, caller::Caller};

const fn grant(resource: ResourceId) -> Grant {
    Grant { resource, access: 1, create_access: 0, retain_loans: false }
}

fn opened(caller: &Caller, resource: ResourceId) -> AccessId {
    let request = caller.reserve().unwrap();
    caller.submit(acquire(request, resource)).unwrap();
    let Status::Outcome(outcome) = caller.snapshot(request).unwrap() else {
        panic!("missing acquisition result");
    };
    let Outcome::Ok(value) = outcome.as_ref() else {
        panic!("acquisition failed");
    };
    caller.adopt(request).unwrap();
    caller.release(request).unwrap();
    value.references[0]
}
fn rights(caller: &Caller, access: AccessId, flags: u32) -> Result<(), Error> {
    let request = caller.reserve().unwrap();
    let result = caller.submit(Call {
        request,
        operation: Operation::SetAccess,
        subject: access,
        command: 0,
        expected: None,
        input: Value::copy(&flags.to_le_bytes(), &[]).unwrap(),
    });
    caller.release(request).unwrap();
    result
}

#[test]
fn live_grant_preserves_caller_custody_and_exact_authority() {
    let mut runtime = Runtime::new([0xa9; 16], Limits::default()).unwrap();
    let binding = runtime.bind(&[]).unwrap();
    let other = runtime.bind(&[]).unwrap().caller();
    let caller = binding.caller();
    runtime.start().unwrap();
    let retained = caller.reserve().unwrap();
    let resource = resource(&mut runtime);
    assert_eq!(caller.submit(acquire(retained, resource)), Err(Error::DENIED));
    runtime.grant(&binding, grant(resource)).unwrap();
    runtime.grant(&binding, grant(resource)).unwrap();
    assert_eq!(caller.snapshot(retained), Ok(Status::Reserved));
    caller.submit(acquire(retained, resource)).unwrap();
    let original = caller.snapshot(retained).unwrap();
    assert!(matches!(original, Status::Outcome(_)));
    let changed = Grant { access: 7, ..grant(resource) };
    assert_eq!(runtime.grant(&binding, changed), Err(Error::CONFLICT));
    let creation = Grant { create_access: 1, ..grant(resource) };
    assert_eq!(runtime.grant(&binding, creation), Err(Error::CONFLICT));
    let retained_loans = Grant { retain_loans: true, ..grant(resource) };
    assert_eq!(runtime.grant(&binding, retained_loans), Err(Error::CONFLICT));
    assert_eq!(caller.snapshot(retained), Ok(original));
    let opened = opened(&caller, resource);
    assert_eq!(rights(&caller, opened, 1), Ok(()));
    assert_eq!(rights(&caller, opened, 2), Err(Error::DENIED));
    let denied = other.reserve().unwrap();
    assert_eq!(other.submit(acquire(denied, resource)), Err(Error::DENIED));
    other.release(denied).unwrap();
    caller.adopt(retained).unwrap();
    caller.release(retained).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}

#[test]
fn rejected_grants_do_not_change_binding_authority_or_revive_retirement() {
    let mut runtime = Runtime::new([0xaa; 16], Limits::default()).unwrap();
    let mut foreign = Runtime::new([0xaa; 16], Limits::default()).unwrap();
    let resource = resource(&mut runtime);
    let binding = runtime.bind(&[]).unwrap();
    runtime.start().unwrap();
    assert_eq!(foreign.grant(&binding, grant(resource)), Err(Error::DENIED));
    let invalid = Grant { access: 8, ..grant(resource) };
    assert_eq!(runtime.grant(&binding, invalid), Err(Error::INVALID));
    let missing = ResourceId { serial: resource.serial + 1, ..resource };
    assert_eq!(runtime.grant(&binding, grant(missing)), Err(Error::DENIED));
    let caller = binding.caller();
    let id = caller.reserve().unwrap();
    assert_eq!(caller.submit(acquire(id, resource)), Err(Error::DENIED));
    assert_eq!(runtime.retire_binding(&binding), Err(Error::BUSY));
    assert_eq!(runtime.grant(&binding, grant(resource)), Err(Error::CLOSED));
    caller.release(id).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.retire_binding(&binding).unwrap();
    assert_eq!(runtime.grant(&binding, grant(resource)), Err(Error::CLOSED));
    runtime.close().unwrap();
    foreign.close().unwrap();
}

#[test]
fn initial_grant_capture_rejects_input_beyond_the_resource_bound() {
    let mut runtime =
        Runtime::new([0xab; 16], Limits { resources: 1, ..Limits::default() })
            .unwrap();
    let resource = resource(&mut runtime);
    assert!(matches!(
        runtime.bind(&[grant(resource); 2]),
        Err(Error::PRESSURE)
    ));
    let binding = runtime.bind(&[grant(resource)]).unwrap();
    runtime.grant(&binding, grant(resource)).unwrap();
    runtime.close().unwrap();
}
