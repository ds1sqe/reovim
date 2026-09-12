#![no_std]
#![forbid(unsafe_code)]
//! Exact source bytes behind a public native immutable Model contract.

extern crate alloc;

pub mod api;
mod backing;
#[cfg(test)]
mod backing_tests;
mod storage;
#[cfg(test)]
mod tests;

use reovim_kernel_interface::{Error, Message, buffer::State};
use storage::Source;

reovim_module::rv_module! {
    pub SOURCE {
        contract: ("source.model", 1),
        model: {
            initialize ("source.initial", 1) => initialize,
            prepare ("source.splice", 1) => prepare,
            observe ("source.bytes", 1) => observe,
            inspect => inspect,
        },
    }
}

reovim_module::rv_module! {
    pub OPEN {
        contract: ("source.open.model", 1),
        model: {
            initial_input: [reovim_module::Permissions::READ],
            initialize ("source.open", 1) => unavailable,
            prepare ("source.splice", 1) => prepare,
            observe ("source.bytes", 1) => observe,
            inspect => inspect,
            initialize_async => open,
        },
    }
}

fn unavailable(_: Message) -> Result<State, Error> {
    Err(Error::UNSUPPORTED)
}
fn open(
    kernel: Option<reovim_module::Connection>,
    input: Message,
) -> reovim_kernel_interface::buffer::StateReply {
    alloc::boxed::Box::pin(async move {
        if !input.data.is_empty() {
            return Err(Error::INVALID);
        }
        let source = Source::backed(
            backing::Backing::open(
                kernel.ok_or(Error::DENIED)?,
                input.resources[0],
            )
            .await?,
        );
        let cost = source.cost()?;
        Ok(State::new(source, cost))
    })
}

fn inspect(
    state: State,
    input: Message,
) -> reovim_kernel_interface::provider::ProviderReply {
    alloc::boxed::Box::pin(async move {
        if input.data.is_empty() {
            return observe(&state, input);
        }
        let source = state.get::<Source>().ok_or(Error::PROVIDER)?;
        let range = api::decode_range(&input.data)?;
        let mut data = api::encode_range(&range).to_vec();
        source.read_async(range, &mut data).await?;
        Ok(Message {
            contract: input.contract,
            data: data.into(),
            resources: alloc::boxed::Box::new([]),
        })
    })
}

fn initialize(input: Message) -> Result<State, Error> {
    let source = Source::new(input.data)?;
    let cost = source.cost()?;
    Ok(State::new(source, cost))
}

fn prepare(state: &State, input: Message) -> Result<State, Error> {
    let source = state.get::<Source>().ok_or(Error::PROVIDER)?;
    let range = api::decode_range(input.data.get(..16).ok_or(Error::INVALID)?)?;
    let next = source.splice(range, &input.data[16..])?;
    drop(input);
    let cost = next.cost()?;
    Ok(State::new(next, cost))
}

fn observe(state: &State, input: Message) -> Result<Message, Error> {
    let source = state.get::<Source>().ok_or(Error::PROVIDER)?;
    if input.data.is_empty() {
        return Ok(Message::new(input.contract, &source.len().to_le_bytes()));
    }
    let range = api::decode_range(&input.data)?;
    let mut data = api::encode_range(&range).to_vec();
    source.read(range, &mut data)?;
    Ok(Message {
        contract: input.contract,
        data: data.into(),
        resources: alloc::boxed::Box::new([]),
    })
}
