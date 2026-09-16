#![allow(unsafe_code)]

#[path = "support/memory.rs"]
mod memory;

use reovim_kernel_interface::{
    Connection, Contract, Error, Message, abi,
    buffer::{State, StateReply},
    provider::Permissions,
};
use reovim_kernel_runtime::{Limits, Runtime};
use reovim_sdk::module::{Identity, ModelDefinition};
use std::{
    ffi::c_void,
    ptr,
    task::{Context, Waker},
};

static MODEL: ModelDefinition = ModelDefinition {
    identity: Identity::new("rust.backed", 1),
    dependencies: &[],
    initial: Identity::new("rust.initial", 1),
    initial_input: &[Permissions::READ],
    references: None,
    edit: Identity::new("rust.edit", 1),
    read: Identity::new("rust.value", 1),
    initialize: |_| Err(Error::UNSUPPORTED),
    prepare: |_, _| Err(Error::UNSUPPORTED),
    observe: |_, input| Ok(input),
    inspect: None,
    initialize_async: Some(initialize),
    prepare_async: None,
};

fn initialize(kernel: Option<Connection>, input: Message) -> StateReply {
    Box::pin(async move {
        let kernel = kernel.ok_or(Error::DENIED)?;
        let id = input.resources[0];
        let captured = kernel.capture(id, Permissions::READ)?;
        let result = captured
            .read(id, &Message::new(Contract::new("resource.bytes", 1), &[]))
            .await
            .map_err(|_| Error::UNAVAILABLE)?;
        Ok(State::new((captured, result.data), 0))
    })
}
const unsafe extern "C" fn noop(_: *const c_void) {}
fn wake() -> abi::Wake {
    abi::Wake { context: ptr::null(), retain: noop, release: noop, wake: noop }
}

#[test]
fn safe_initializer_uses_composition_connection_after_constructor_borrow() {
    let mut runtime = Runtime::new(210, Limits::default()).unwrap();
    runtime.start().unwrap();
    let source = runtime
        .register("source", memory::provider(b"raw"))
        .map_err(|(error, _)| error)
        .unwrap();
    let binding = runtime.bind(&[(source, Permissions::READ)]).unwrap();
    let kernel = binding.connection();
    let id = kernel.resolve("source").into_inner().unwrap();
    let raw = reovim_sdk::export(kernel);
    let table = MODEL.table();
    let context =
        unsafe { table.create.unwrap()(table.declaration, &raw const raw) };
    unsafe {
        ((*raw.table).release)(raw.context);
    }
    drop(binding);
    let resources = [id.0];
    let input = abi::Request {
        method: abi::WRITE,
        subject: 0,
        contract: table.create_contract,
        data: abi::Bytes { data: ptr::null(), len: 0 },
        resources: resources.as_ptr(),
        resource_count: 1,
    };
    let pending =
        unsafe { table.initialize_async.unwrap()(context, &raw const input) };
    let mut executor = Context::from_waker(Waker::noop());
    let mut status = unsafe { (pending.poll)(pending.context, wake()) };
    assert_eq!(status.status, 0);
    for _ in 0..8 {
        runtime.advance(&mut executor).unwrap();
        status = unsafe { (pending.poll)(pending.context, wake()) };
        if status.status != 0 {
            break;
        }
    }
    assert_eq!((status.status, status.error), (1, 0));
    let mut reply = unsafe { (pending.take)(pending.context) };
    unsafe {
        (pending.release)(pending.context);
    }
    let state = unsafe { &*reply.state.cast::<State>() };
    let (_, bytes) = state.get::<(Connection, Box<[u8]>)>().unwrap();
    assert_eq!(bytes.as_ref(), b"raw");
    assert_eq!(runtime.unregister(source), Err(Error::BUSY));
    unsafe {
        table.release.unwrap()(context, &raw mut reply);
        table.destroy.unwrap()(context);
    }
    runtime.unregister(source).unwrap();
    runtime.close().unwrap();
}
