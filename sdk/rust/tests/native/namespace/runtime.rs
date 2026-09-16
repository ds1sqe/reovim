use super::*;
use reovim_kernel_interface::{
    caller::{Call, Caller, Operation, Status},
    resource::AccessId,
};
use reovim_kernel_runtime::{Limits, Runtime};

fn mounted(
    runtime: &mut Runtime,
    library: &Rc<Library>,
    path: &[u8],
) -> ResourceId {
    let raw = directory(library);
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    runtime.register(path, provider, None).map_err(|(e, _)| e).unwrap()
}
fn query(
    runtime: &mut Runtime,
    caller: &Caller,
    op: Operation,
    subject: AccessId,
    data: &[u8],
) -> Result<Value, Error> {
    let request = caller.reserve()?;
    if let Err(error) = caller.submit(Call {
        request,
        operation: op,
        subject,
        command: 0,
        expected: None,
        input: Value::copy(data, &[])?,
    }) {
        caller.release(request)?;
        return Err(error);
    }
    completed(runtime, caller, request)
}
fn completed(
    runtime: &mut Runtime,
    caller: &Caller,
    request: u64,
) -> Result<Value, Error> {
    let cx = &mut Context::from_waker(Waker::noop());
    for _ in 0..16 {
        runtime.advance(cx)?;
        let Status::Outcome(outcome) = caller.snapshot(request)? else {
            continue;
        };
        let result = success(outcome.as_ref());
        if result.is_ok() {
            caller.adopt(request)?;
        }
        caller.release(request)?;
        runtime.advance(cx)?;
        return result;
    }
    panic!("immediate fixture did not settle within its drive bound")
}
fn success(outcome: &Outcome) -> Result<Value, Error> {
    match outcome {
        Outcome::Ok(value) => Ok(value.clone()),
        Outcome::Failed(error) => Err(*error),
        other @ Outcome::Unknown { .. } => {
            panic!("unexpected uncertain listing: {other:?}")
        }
    }
}
fn open(runtime: &mut Runtime, caller: &Caller, id: ResourceId) -> AccessId {
    let mut input = 1_u32.to_le_bytes().to_vec();
    input.extend_from_slice(&id.epoch);
    input.extend_from_slice(&id.serial.to_le_bytes());
    query(runtime, caller, Operation::Acquire, AccessId(0), &input)
        .unwrap()
        .references[0]
}
fn entries(
    runtime: &mut Runtime,
    caller: &Caller,
    access: AccessId,
    maximum: u64,
) -> Result<Value, Error> {
    let mut input = 0_u64.to_le_bytes().to_vec();
    input.extend_from_slice(&maximum.to_le_bytes());
    query(runtime, caller, Operation::Entries, access, &input)
}
fn ids(value: &Value) -> Vec<(Vec<u8>, ResourceId)> {
    let mut input = &value.data[..];
    let count = u32::from_le_bytes(input[..4].try_into().unwrap());
    input = &input[4..];
    let mut rows = Vec::new();
    for _ in 0..count {
        let name = bytes(&mut input);
        let id = ResourceId {
            epoch: input[..16].try_into().unwrap(),
            serial: u64::from_le_bytes(input[16..24].try_into().unwrap()),
        };
        input = &input[24..];
        rows.push((name, id));
    }
    assert_eq!(u64::from_le_bytes(input.try_into().unwrap()), 0);
    rows
}
#[test]
fn caller_listing_interns_aliases_and_preserves_replacement_identity() {
    let library = Library::compile_source("namespace.c");
    let mut runtime =
        Runtime::new([0x81; 16], Limits { resources: 3, ..Limits::default() })
            .unwrap();
    let directory = mounted(&mut runtime, &library, b"/device");
    let caller = runtime.bind(&[grant(directory, 1)]).unwrap().caller();
    runtime.start().unwrap();
    let access = open(&mut runtime, &caller, directory);
    assert_eq!(entries(&mut runtime, &caller, access, 96), Err(Error::DENIED));
    query(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        access,
        &1_u32.to_le_bytes(),
    )
    .unwrap();
    // Native keys fit, but translating them into identities exceeds this page.
    assert_eq!(
        entries(&mut runtime, &caller, access, 50),
        Err(Error::PRESSURE)
    );
    let first = entries(&mut runtime, &caller, access, 96).unwrap();
    let original = ids(&first);
    assert_eq!(original[0].0, b"disk");
    assert_eq!(original[1], (b"alias".to_vec(), original[0].1));
    assert_eq!(
        ids(&entries(&mut runtime, &caller, access, 96).unwrap()),
        original
    );
    let replace: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_replace") };
    unsafe { replace() };
    let current = ids(&entries(&mut runtime, &caller, access, 96).unwrap());
    assert_ne!(current[0].1, original[0].1);
    assert_eq!(ids(&first), original);
    unsafe { replace() };
    assert_eq!(
        entries(&mut runtime, &caller, access, 96),
        Err(Error::PRESSURE)
    );
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_directories_destroyed"), 1);
}
#[test]
fn identical_keys_in_different_provider_instances_are_distinct() {
    let library = Library::compile_source("namespace.c");
    let mut runtime = Runtime::new([0x82; 16], Limits::default()).unwrap();
    let a = mounted(&mut runtime, &library, b"/a");
    let b = mounted(&mut runtime, &library, b"/b");
    let caller = runtime.bind(&[grant(a, 1), grant(b, 1)]).unwrap().caller();
    runtime.start().unwrap();
    let a = open(&mut runtime, &caller, a);
    let b = open(&mut runtime, &caller, b);
    for access in [a, b] {
        query(
            &mut runtime,
            &caller,
            Operation::SetAccess,
            access,
            &1_u32.to_le_bytes(),
        )
        .unwrap();
    }
    let a = ids(&entries(&mut runtime, &caller, a, 96).unwrap());
    let b = ids(&entries(&mut runtime, &caller, b, 96).unwrap());
    assert_ne!(a[0].1, b[0].1);
    runtime.close().unwrap();
}

#[test]
fn admitted_listing_waits_for_driving_and_lost_reply_joins_original() {
    let library = Library::compile_source("namespace.c");
    let mut runtime = Runtime::new([0x83; 16], Limits::default()).unwrap();
    let directory = mounted(&mut runtime, &library, b"/device");
    let caller = runtime.bind(&[grant(directory, 1)]).unwrap().caller();
    runtime.start().unwrap();
    let access = open(&mut runtime, &caller, directory);
    query(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        access,
        &1_u32.to_le_bytes(),
    )
    .unwrap();
    let mut data = 0_u64.to_le_bytes().to_vec();
    data.extend_from_slice(&96_u64.to_le_bytes());
    let request = Call {
        request: caller.reserve().unwrap(),
        operation: Operation::Entries,
        subject: access,
        command: 0,
        expected: None,
        input: Value::copy(&data, &[]).unwrap(),
    };
    caller.submit(request.clone()).unwrap();
    assert_eq!(library.count(c"fixture_listings"), 0);
    assert_eq!(caller.snapshot(request.request).unwrap(), Status::Pending);
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    assert_eq!(library.count(c"fixture_listings"), 1);
    caller.submit(request.clone()).unwrap();
    let observed = completed(&mut runtime, &caller, request.request).unwrap();
    assert_eq!(ids(&observed)[0].1, ids(&observed)[1].1);
    assert_eq!(library.count(c"fixture_listings"), 1);
    runtime.close().unwrap();
}

#[path = "materialization.rs"]
mod materialization;

#[path = "metadata.rs"]
mod metadata;
