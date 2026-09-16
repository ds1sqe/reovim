use reovim_kernel_interface::{
    Error, ResourceId, Value,
    caller::{Call, Operation, Status},
    provider::{ResourceOps, ResourceProvider},
    resource::AccessId,
};
use reovim_kernel_runtime::{Grant, Limits, Runtime};
use std::{
    rc::Rc,
    task::{Context, Waker},
};

fn resource(runtime: &mut Runtime) -> ResourceId {
    let provider = ResourceProvider::new(
        (),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |()| Ok(()),
        },
        Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap();
    runtime.register(b"/subject", provider, None).ok().unwrap()
}
fn acquire(request: u64, resource: ResourceId) -> Call {
    let mut input = 1_u32.to_le_bytes().to_vec();
    input.extend_from_slice(&resource.epoch);
    input.extend_from_slice(&resource.serial.to_le_bytes());
    Call {
        request,
        operation: Operation::Acquire,
        subject: AccessId(0),
        command: 0,
        expected: None,
        input: Value::copy(&input, &[]).unwrap(),
    }
}

#[test]
fn retirement_fences_admission_but_keeps_receipts_and_releases_capacity() {
    let mut runtime =
        Runtime::new([0xa7; 16], Limits { callers: 1, ..Limits::default() })
            .unwrap();
    let resource = resource(&mut runtime);
    let binding = runtime
        .bind(&[Grant {
            resource,
            access: 0,
            create_access: 0,
            retain_loans: false,
        }])
        .unwrap();
    let caller = binding.caller();
    let clone = binding.caller();
    runtime.start().unwrap();
    let first = caller.reserve().unwrap();
    let reserved = caller.reserve().unwrap();
    let submitted = acquire(first, resource);
    caller.submit(submitted.clone()).unwrap();
    let before = caller.snapshot(first).unwrap();
    assert!(matches!(before, Status::Outcome(_)));
    assert_eq!(runtime.retire_binding(&binding), Err(Error::BUSY));
    assert_eq!(clone.reserve(), Err(Error::CLOSED));
    assert_eq!(clone.submit(acquire(reserved, resource)), Err(Error::CLOSED));
    assert_eq!(clone.submit(submitted), Ok(()));
    assert_eq!(clone.snapshot(first), Ok(before));
    clone.adopt(first).unwrap();
    clone.release(first).unwrap();
    clone.release(reserved).unwrap();
    assert_eq!(runtime.retire_binding(&binding), Err(Error::BUSY));
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.retire_binding(&binding).unwrap();
    runtime.retire_binding(&binding).unwrap();
    assert_eq!(caller.reserve(), Err(Error::CLOSED));
    let replacement = runtime.bind(&[]).unwrap();
    let new_caller = replacement.caller();
    let id = new_caller.reserve().unwrap();
    new_caller.release(id).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.retire_binding(&replacement).unwrap();
    runtime.close().unwrap();
}

#[test]
fn foreign_retirement_does_not_fence_the_original_caller() {
    let mut first = Runtime::new([0xa8; 16], Limits::default()).unwrap();
    let mut foreign = Runtime::new([0xa8; 16], Limits::default()).unwrap();
    let binding = first.bind(&[]).unwrap();
    first.start().unwrap();
    assert_eq!(foreign.retire_binding(&binding), Err(Error::DENIED));
    let caller = binding.caller();
    let id = caller.reserve().unwrap();
    caller.release(id).unwrap();
    first.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    first.retire_binding(&binding).unwrap();
    foreign.close().unwrap();
    first.close().unwrap();
}

#[path = "binding/imports.rs"]
mod imports;

#[path = "binding/grants.rs"]
mod grants;

#[test]
fn acquisition_reserves_the_complete_output_envelope_before_admission() {
    for (bytes, references) in [(31, 1), (32, 0)] {
        let defaults = Limits::default();
        let mut runtime = Runtime::new(
            [0xa8; 16],
            Limits {
                values: reovim_kernel_interface::declaration::ValueLimits {
                    bytes,
                    references,
                    ..defaults.values
                },
                ..defaults
            },
        )
        .unwrap();
        let subject = resource(&mut runtime);
        let caller = runtime
            .bind(&[Grant {
                resource: subject,
                access: 0,
                create_access: 0,
                retain_loans: false,
            }])
            .unwrap()
            .caller();
        runtime.start().unwrap();
        let request = caller.reserve().unwrap();
        assert_eq!(
            caller.submit(acquire(request, subject)),
            Err(Error::PRESSURE)
        );
        assert_eq!(caller.snapshot(request), Ok(Status::Reserved));
        caller.release(request).unwrap();
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
        runtime.close().unwrap();
    }
}
