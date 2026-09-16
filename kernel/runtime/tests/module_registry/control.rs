use super::*;
use reovim_kernel_interface::{
    Outcome, Value,
    call::Cancellation,
    caller::{Call, Caller, Operation, Status},
    module::control,
    resource::AccessId,
};
use reovim_kernel_runtime::Grant;

fn drive(runtime: &mut Runtime) {
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
}
pub fn call(
    caller: &Caller,
    operation: Operation,
    subject: AccessId,
    command: u32,
    data: &[u8],
) -> Call {
    Call {
        request: caller.reserve().unwrap(),
        operation,
        subject,
        command,
        expected: None,
        input: Value::copy(data, &[]).unwrap(),
    }
}
pub fn outcome(caller: &Caller, id: u64) -> Rc<Outcome> {
    let Status::Outcome(outcome) = caller.snapshot(id).unwrap() else {
        panic!("result not delivered");
    };
    outcome
}
pub fn invoke(
    runtime: &mut Runtime,
    caller: &Caller,
    operation: Operation,
    subject: AccessId,
    command: u32,
    data: &[u8],
) -> Value {
    let call = call(caller, operation, subject, command, data);
    let id = call.request;
    caller.submit(call).unwrap();
    for _ in 0..4 {
        drive(runtime);
    }
    let result = outcome(caller, id);
    let Outcome::Ok(value) = result.as_ref() else { panic!("call failed") };
    caller.adopt(id).unwrap();
    caller.release(id).unwrap();
    value.clone()
}
pub fn bind(runtime: &mut Runtime, instance: ResourceId) -> (Caller, AccessId) {
    bind_decl(runtime, instance, &control::declaration().unwrap())
}
pub fn bind_decl(
    runtime: &mut Runtime,
    instance: ResourceId,
    definition: &reovim_kernel_interface::declaration::Declaration,
) -> (Caller, AccessId) {
    let caller = runtime
        .bind(&[Grant {
            resource: instance,
            access: 5,
            create_access: 0,
            retain_loans: false,
        }])
        .unwrap()
        .caller();
    let mut locator = 1_u32.to_le_bytes().to_vec();
    locator.extend_from_slice(&instance.epoch);
    locator.extend_from_slice(&instance.serial.to_le_bytes());
    let access =
        invoke(runtime, &caller, Operation::Acquire, AccessId(0), 0, &locator)
            .references[0];
    let mut page = 0_u64.to_le_bytes().to_vec();
    page.extend_from_slice(&4096_u64.to_le_bytes());
    let offered =
        invoke(runtime, &caller, Operation::Interfaces, access, 0, &page);
    let length =
        u32::from_le_bytes(offered.data[4..8].try_into().unwrap()) as usize;
    assert_eq!(&offered.data[8..8 + length], definition.bytes());
    let mut input =
        u32::try_from(definition.bytes().len()).unwrap().to_le_bytes().to_vec();
    input.extend_from_slice(definition.bytes());
    let bound = invoke(runtime, &caller, Operation::Bind, access, 0, &input)
        .references[0];
    (caller, bound)
}

#[test]
fn ordinary_caller_discovers_controls_and_recovers_the_same_unload_result() {
    let (mut runtime, root, imports) = base();
    let constructors = Rc::new(Cell::new(0));
    register(
        &mut runtime,
        root,
        &imports,
        definition(1, "one", 1, 1, vec![], &constructors),
    );
    let selected =
        runtime.select_module(root, "one", 1, ModuleScope::Global).unwrap();
    let instance = runtime.activate_selected(&selected.modules()[0]).unwrap();
    drive(&mut runtime);
    let (caller, bound) = bind(&mut runtime, instance);
    let denied = call(&caller, Operation::Ioctl, bound, control::STATUS, &[]);
    assert_eq!(caller.submit(denied.clone()), Err(Error::DENIED));
    caller.release(denied.request).unwrap();
    invoke(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        bound,
        0,
        &1_u32.to_le_bytes(),
    );
    assert_eq!(
        invoke(
            &mut runtime,
            &caller,
            Operation::Ioctl,
            bound,
            control::STATUS,
            &[]
        )
        .data
        .as_ref(),
        &2_u32.to_le_bytes()
    );
    let denied = call(&caller, Operation::Ioctl, bound, control::UNLOAD, &[]);
    assert_eq!(caller.submit(denied.clone()), Err(Error::DENIED));
    caller.release(denied.request).unwrap();
    invoke(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        bound,
        0,
        &5_u32.to_le_bytes(),
    );
    let cancelled =
        call(&caller, Operation::Ioctl, bound, control::UNLOAD, &[]);
    caller.submit(cancelled.clone()).unwrap();
    drive(&mut runtime);
    assert_eq!(caller.cancel(cancelled.request), Ok(Cancellation::Accepted));
    for _ in 0..3 {
        drive(&mut runtime);
    }
    assert_eq!(
        outcome(&caller, cancelled.request).as_ref(),
        &Outcome::Failed(Error::CANCELLED)
    );
    caller.release(cancelled.request).unwrap();
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Ready));
    let unload = call(&caller, Operation::Ioctl, bound, control::UNLOAD, &[]);
    caller.submit(unload.clone()).unwrap();
    drive(&mut runtime);
    drive(&mut runtime);
    assert_eq!(caller.cancel(unload.request), Ok(Cancellation::TooLate));
    drive(&mut runtime);
    drive(&mut runtime);
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Closed));
    caller.submit(unload.clone()).unwrap();
    assert_eq!(
        outcome(&caller, unload.request).as_ref(),
        &Outcome::Ok(Value::unit())
    );
    let replacement =
        runtime.activate_selected(&selected.modules()[0]).unwrap();
    assert_ne!(replacement, instance);
    drive(&mut runtime);
    caller.submit(unload.clone()).unwrap();
    assert_eq!(runtime.module_status(replacement), Ok(Lifecycle::Ready));
    caller.adopt(unload.request).unwrap();
    caller.release(unload.request).unwrap();
    assert_eq!(
        invoke(
            &mut runtime,
            &caller,
            Operation::Ioctl,
            bound,
            control::STATUS,
            &[]
        )
        .data
        .as_ref(),
        &4_u32.to_le_bytes()
    );
    drive(&mut runtime);
    runtime.close().unwrap();
}
