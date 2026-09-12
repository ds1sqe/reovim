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
            inspect: None,
            unload: None,
            initialize: |(), input| {
                Box::pin(core::future::ready(heading_initial(input)))
            },
            prepare: |(), state, input| {
                Box::pin(core::future::ready(heading_edit(state, input)))
            },
            observe: |(), state, input| heading_read(state, input),
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
            inspect: None,
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

/// Prepares immutable heading state without performing external work.
/// # Errors
/// Rejects a source without a supported heading.
pub fn heading_initial(input: Message) -> Result<State, Error> {
    let bytes = Bytes(input.data.into_vec());
    reovim_domain_basic::replace_heading(&bytes, b"check")?;
    let cost = bytes.0.len() as u64;
    Ok(State::new(bytes, cost))
}

/// Prepares a replacement while preserving unrelated source bytes.
/// # Errors
/// Rejects foreign state or an unsupported heading edit.
pub fn heading_edit(state: &State, input: Message) -> Result<State, Error> {
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
    result
}

/// Observes the preserved source bytes of immutable heading state.
/// # Errors
/// Rejects state belonging to another model.
pub fn heading_read(state: &State, input: Message) -> Result<Message, Error> {
    if !input.data.is_empty() {
        return Err(Error::INVALID);
    }
    drop(input);
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

fn keys_read(
    _: &mut (),
    state: &State,
    input: Message,
) -> Result<Message, Error> {
    if !input.data.is_empty() {
        return Err(Error::INVALID);
    }
    drop(input);
    let values = state.get::<Keys>().ok_or(Error::UNSUPPORTED)?;
    Ok(Message::new(
        Contract::new("keys.values", 1),
        &reovim_domain_basic::encode_keys(values),
    ))
}
