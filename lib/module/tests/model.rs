#![allow(unsafe_code)]

use reovim_kernel_interface::{
    Error, Message, abi,
    buffer::{State, StateReply},
};
use std::{ffi::c_void, future::poll_fn, ptr, task::Poll};

reovim_module::rv_module! {
    MODEL {
        contract: ("test.model", 1),
        model: {
            initial_input: [reovim_module::Permissions::READ],
            initialize ("test.initial", 1) => initialize,
            prepare ("test.edit", 1) => prepare,
            observe ("test.read", 1) => observe,
            initialize_async => initialize_async,
            prepare_async => prepare_async,
        },
    }
}

fn initialize(_: Message) -> Result<State, Error> {
    Err(Error::UNSUPPORTED)
}
fn prepare(_: &State, _: Message) -> Result<State, Error> {
    Err(Error::UNSUPPORTED)
}
#[allow(clippy::unnecessary_wraps)] // Model callback signature.
const fn observe(_: &State, input: Message) -> Result<Message, Error> {
    Ok(input)
}
fn initialize_async(
    _: Option<reovim_module::Connection>,
    input: Message,
) -> StateReply {
    assert_eq!(input.resources.as_ref(), &[reovim_module::ResourceId(7)]);
    let mut yielded = false;
    Box::pin(poll_fn(move |context| {
        if !yielded {
            yielded = true;
            context.waker().wake_by_ref();
            return Poll::Pending;
        }
        Poll::Ready(Ok(State::new(input.data.to_vec(), 0)))
    }))
}
fn prepare_async(state: State, input: Message) -> StateReply {
    Box::pin(async move {
        let mut bytes = state.get::<Vec<u8>>().ok_or(Error::PROVIDER)?.clone();
        bytes.extend_from_slice(&input.data);
        Ok(State::new(bytes, 0))
    })
}
const unsafe extern "C" fn noop(_: *const c_void) {}
fn wake() -> abi::Wake {
    abi::Wake { context: ptr::null(), retain: noop, release: noop, wake: noop }
}
const fn request(contract: abi::Contract, data: &[u8]) -> abi::Request {
    abi::Request {
        method: abi::WRITE,
        subject: 0,
        contract,
        data: abi::Bytes { data: data.as_ptr(), len: data.len() },
        resources: ptr::null(),
        resource_count: 0,
    }
}

#[test]
fn declaration_copies_inputs_and_owns_state_across_deferred_callbacks() {
    let table = MODEL.table();
    unsafe {
        let context = table.create.unwrap()(table.declaration, ptr::null());
        let bytes = vec![17];
        let mut input = request(table.create_contract, &bytes);
        let resources = vec![7_u64];
        input.resources = resources.as_ptr();
        input.resource_count = resources.len();
        let pending =
            table.initialize_async.unwrap()(context, &raw const input);
        drop(bytes);
        drop(resources);
        assert_eq!((pending.poll)(pending.context, wake()).status, 0);
        assert_eq!((pending.poll)(pending.context, wake()).status, 1);
        let mut old = (pending.take)(pending.context);
        (pending.release)(pending.context);
        let input = request(table.edit_contract, &[29]);
        let pending =
            table.prepare_async.unwrap()(context, old.state, &raw const input);
        table.release.unwrap()(context, &raw mut old);
        assert_eq!((pending.poll)(pending.context, wake()).status, 1);
        let mut next = (pending.take)(pending.context);
        (pending.release)(pending.context);
        let state = &*next.state.cast::<State>();
        assert_eq!(state.get::<Vec<u8>>().unwrap(), &[17, 29]);
        table.release.unwrap()(context, &raw mut next);
        table.destroy.unwrap()(context);
    }
}
