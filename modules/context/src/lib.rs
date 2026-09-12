#![no_std]
#![forbid(unsafe_code)]
//! Retained editing focus over model-independent Buffer references.
extern crate alloc;
pub mod api;

use alloc::{boxed::Box, vec::Vec};
use reovim_kernel_interface::{
    Connection, Error, Message,
    buffer::{self, State, StateReply},
    provider::Permissions,
};

reovim_module::rv_module! {
    pub CONTEXT {
        contract: ("editing.context.model", 1),
        model: {
            references: Permissions::READ,
            initialize ("editing.context.initial", 1) => unavailable,
            prepare ("editing.context.update", 1) => unavailable_edit,
            observe ("editing.context", 1) => observe,
            initialize_async => initialize,
            prepare_async => prepare,
        },
    }
}

struct Context {
    kernel: Connection,
    value: api::EditingSessionState,
    _captures: Vec<Connection>,
}
fn unavailable(_: Message) -> Result<State, Error> {
    Err(Error::UNSUPPORTED)
}
fn unavailable_edit(_: &State, _: Message) -> Result<State, Error> {
    Err(Error::UNSUPPORTED)
}
fn initialize(kernel: Option<Connection>, input: Message) -> StateReply {
    Box::pin(
        async move { candidate(kernel.ok_or(Error::DENIED)?, input).await },
    )
}
fn prepare(state: State, input: Message) -> StateReply {
    Box::pin(async move {
        let context = state.get::<Context>().ok_or(Error::PROVIDER)?;
        candidate(context.kernel.clone(), input).await
    })
}
#[allow(clippy::future_not_send)] // Model state belongs to one executor.
async fn candidate(kernel: Connection, input: Message) -> Result<State, Error> {
    let value = api::decode_value(&input)?;
    let mut captures = Vec::new();
    captures.try_reserve(value.known.len()).map_err(|_| Error::PRESSURE)?;
    for id in &value.known {
        let access = kernel.capture(*id, Permissions::READ)?;
        let reply =
            buffer::describe(&access, *id).await.map_err(|e| failure(&e))?;
        if buffer::descriptor(&reply)?.state.buffer != *id {
            return Err(Error::PROVIDER);
        }
        captures.push(access);
    }
    let cost = core::mem::size_of::<Context>()
        + captures.capacity() * core::mem::size_of::<Connection>()
        + value.known.len() * core::mem::size_of::<reovim_module::ResourceId>();
    Ok(State::new(Context { kernel, value, _captures: captures }, cost as u64))
}
const fn failure(error: &reovim_kernel_interface::RequestError) -> Error {
    use reovim_kernel_interface::RequestError;
    match error {
        RequestError::Rejected(error) | RequestError::Failed(error) => {
            Error::new(error.code.0)
        }
        RequestError::Unknown { .. } => Error::PROVIDER,
    }
}
fn observe(state: &State, _: Message) -> Result<Message, Error> {
    let context = state.get::<Context>().ok_or(Error::PROVIDER)?;
    api::message("editing.context", &context.value)
}
