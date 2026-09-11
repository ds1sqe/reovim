#![no_std]
#![forbid(unsafe_code)]
//! Public model/semantic tables registered without specializing the Kernel.

extern crate alloc;

use alloc::{boxed::Box, rc::Rc};
use reovim_kernel_interface::buffer::{
    ModelOps, ModelProvider, State, StateReply, integer,
};
use reovim_kernel_interface::{Contract, Error, Message};
use reovim_resource_local::{Bytes, Keys};

#[must_use]
pub fn heading() -> ModelProvider {
    ModelProvider::new(
        (),
        ModelOps {
            unload: None,
            initialize: heading_initial,
            prepare: heading_edit,
            observe: heading_read,
        },
        Contract::new("heading.initial", 1),
        Contract::new("heading.source", 1),
        Contract::new("heading.replace", 1),
        Rc::new(()),
    )
}

#[must_use]
pub fn keyed() -> ModelProvider {
    ModelProvider::new(
        (),
        ModelOps {
            unload: None,
            initialize: keys_initial,
            prepare: keys_edit,
            observe: keys_read,
        },
        Contract::new("keys.initial", 1),
        Contract::new("keys.values", 1),
        Contract::new("keys.toggle", 1),
        Rc::new(()),
    )
}

fn heading_initial(_: &mut (), input: Message) -> StateReply {
    let bytes = Bytes(input.data.into_vec());
    Box::pin(async move {
        reovim_domain_basic::replace_heading(&bytes, b"check")?;
        let cost = bytes.0.len() as u64;
        Ok(State::new(bytes, cost))
    })
}

fn heading_edit(_: &mut (), state: &State, input: Message) -> StateReply {
    let result = state
        .get::<Bytes>()
        .ok_or(Error::UNSUPPORTED)
        .and_then(|source| {
            reovim_domain_basic::replace_heading(source, &input.data)
        })
        .map(|value| {
            let cost = value.0.len() as u64;
            State::new(value, cost)
        });
    drop(input);
    Box::pin(core::future::ready(result))
}

fn heading_read(_: &mut (), state: &State) -> Result<Message, Error> {
    let bytes = state.get::<Bytes>().ok_or(Error::UNSUPPORTED)?;
    Ok(Message::new(Contract::new("heading.source", 1), &bytes.0))
}

fn keys_initial(_: &mut (), input: Message) -> StateReply {
    Box::pin(async move {
        let values = reovim_domain_basic::decode_keys(&input.data)?;
        let cost = values.0.len() as u64 * 32;
        Ok(State::new(values, cost))
    })
}

fn keys_edit(_: &mut (), state: &State, input: Message) -> StateReply {
    let result = state
        .get::<Keys>()
        .ok_or(Error::UNSUPPORTED)
        .and_then(|source| {
            reovim_domain_basic::toggle(source, integer(&input.data)?)
        })
        .map(|value| {
            let cost = value.0.len() as u64 * 32;
            State::new(value, cost)
        });
    drop(input);
    Box::pin(core::future::ready(result))
}

fn keys_read(_: &mut (), state: &State) -> Result<Message, Error> {
    let values = state.get::<Keys>().ok_or(Error::UNSUPPORTED)?;
    Ok(Message::new(
        Contract::new("keys.values", 1),
        &reovim_domain_basic::encode_keys(values),
    ))
}
