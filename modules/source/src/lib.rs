#![no_std]
#![forbid(unsafe_code)]
//! Exact source bytes behind a public native immutable Model contract.

extern crate alloc;

pub mod api;
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
        },
    }
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
