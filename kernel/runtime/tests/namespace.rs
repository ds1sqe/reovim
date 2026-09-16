use reovim_kernel_interface::{
    Error, Outcome, ResourceId, Value,
    caller::{Call, Caller, Operation, Status},
    provider::{ResourceOps, ResourceProvider},
    resource::AccessId,
};
use reovim_kernel_runtime::{Grant, Limits, Runtime};
use std::{
    rc::Rc,
    task::{Context, Waker},
};

fn call(
    runtime: &mut Runtime,
    caller: &Caller,
    operation: Operation,
    subject: AccessId,
    data: &[u8],
) -> Result<Value, Error> {
    let request = caller.reserve()?;
    let submitted = caller.submit(Call {
        request,
        operation,
        subject,
        command: 0,
        expected: None,
        input: Value::copy(data, &[])?,
    });
    if let Err(error) = submitted {
        caller.release(request)?;
        runtime.advance(&mut Context::from_waker(Waker::noop()))?;
        return Err(error);
    }
    let Status::Outcome(outcome) =
        caller.result(request, &mut Context::from_waker(Waker::noop()))?
    else {
        panic!("common operation pending");
    };
    let Outcome::Ok(value) = outcome.as_ref() else {
        panic!("call failed: {outcome:?}");
    };
    let value = value.clone();
    caller.adopt(request)?;
    caller.release(request)?;
    runtime.advance(&mut Context::from_waker(Waker::noop()))?;
    Ok(value)
}
fn acquire(
    runtime: &mut Runtime,
    caller: &Caller,
    id: ResourceId,
) -> Result<Value, Error> {
    let mut locator = 1_u32.to_le_bytes().to_vec();
    locator.extend_from_slice(&id.epoch);
    locator.extend_from_slice(&id.serial.to_le_bytes());
    call(runtime, caller, Operation::Acquire, AccessId(0), &locator)
}
fn provider(runtime: &mut Runtime, path: &[u8]) -> ResourceId {
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
    runtime.register(path, provider, None).ok().unwrap()
}
fn page(maximum: u64) -> Vec<u8> {
    let mut bytes = 0_u64.to_le_bytes().to_vec();
    bytes.extend_from_slice(&maximum.to_le_bytes());
    bytes
}
fn flags(value: &Value) -> u32 {
    u32::from_le_bytes(value.data[28..32].try_into().unwrap())
}
fn rows(value: &Value) -> Vec<(Vec<u8>, ResourceId)> {
    let count = u32::from_le_bytes(value.data[..4].try_into().unwrap());
    let mut bytes = &value.data[4..];
    let mut rows = Vec::new();
    for _ in 0..count {
        let length =
            u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        let name = bytes[4..4 + length].to_vec();
        bytes = &bytes[4 + length..];
        let id = ResourceId {
            epoch: bytes[..16].try_into().unwrap(),
            serial: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        };
        bytes = &bytes[24..];
        rows.push((name, id));
    }
    assert_eq!(u64::from_le_bytes(bytes.try_into().unwrap()), 0);
    rows
}

#[test]
fn listing_is_an_owned_observation_and_does_not_grant_children() {
    let mut runtime = Runtime::new([0xc1; 16], Limits::default()).unwrap();
    let root = runtime.register_namespace(b"/view").unwrap();
    let binding = runtime
        .bind(&[Grant {
            resource: root,
            access: 1,
            create_access: 0,
            retain_loans: false,
        }])
        .unwrap();
    runtime.start().unwrap();
    let caller = binding.caller();
    let opened = acquire(&mut runtime, &caller, root).unwrap().references[0];
    assert_eq!(
        flags(
            &call(&mut runtime, &caller, Operation::Info, opened, &[]).unwrap()
        ),
        3
    );
    assert_eq!(
        call(&mut runtime, &caller, Operation::Entries, opened, &page(4096)),
        Err(Error::DENIED)
    );
    call(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        opened,
        &1_u32.to_le_bytes(),
    )
    .unwrap();
    let empty =
        call(&mut runtime, &caller, Operation::Entries, opened, &page(4096))
            .unwrap();
    let child = provider(&mut runtime, b"/view/child");
    provider(&mut runtime, b"/view/child/not-a-directory");
    provider(&mut runtime, b"/view-other/unrelated");
    let listed =
        call(&mut runtime, &caller, Operation::Entries, opened, &page(4096))
            .unwrap();
    assert_eq!(rows(&listed), vec![(b"child".to_vec(), child)]);
    assert_eq!(rows(&empty), vec![]);
    assert_eq!(
        flags(
            &call(&mut runtime, &caller, Operation::Info, opened, &[]).unwrap()
        ),
        1
    );
    assert_eq!(acquire(&mut runtime, &caller, child), Err(Error::DENIED));
    assert_eq!(
        call(&mut runtime, &caller, Operation::Entries, opened, &page(12)),
        Err(Error::PRESSURE)
    );
    assert_eq!(rows(&listed), vec![(b"child".to_vec(), child)]);
    runtime.close().unwrap();
}

#[test]
fn names_preserve_opted_in_bytes_and_listing_support_is_explicit() {
    let mut runtime = Runtime::new([0xc2; 16], Limits::default()).unwrap();
    runtime.allow_non_ascii_names().unwrap();
    let root = runtime.register_namespace(b"/").unwrap();
    let child = provider(&mut runtime, b"/native-\xff");
    let bindings = [root, child].map(|resource| Grant {
        resource,
        access: 1,
        create_access: 0,
        retain_loans: false,
    });
    let caller = runtime.bind(&bindings).unwrap().caller();
    runtime.start().unwrap();
    let opened = acquire(&mut runtime, &caller, root).unwrap().references[0];
    call(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        opened,
        &1_u32.to_le_bytes(),
    )
    .unwrap();
    let listed =
        call(&mut runtime, &caller, Operation::Entries, opened, &page(4096))
            .unwrap();
    assert_eq!(rows(&listed), vec![(b"native-\xff".to_vec(), child)]);
    let leaf = acquire(&mut runtime, &caller, child).unwrap().references[0];
    assert_eq!(
        flags(
            &call(&mut runtime, &caller, Operation::Info, leaf, &[]).unwrap()
        ),
        0
    );
    assert_eq!(
        call(&mut runtime, &caller, Operation::Entries, leaf, &page(4096)),
        Err(Error::UNSUPPORTED)
    );
    runtime.close().unwrap();
}
