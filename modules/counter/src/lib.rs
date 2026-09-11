#![no_std]
#![forbid(unsafe_code)]
//! A loadable counter service using only Kernel Interface contracts.

extern crate alloc;

use alloc::{boxed::Box, rc::Rc, vec};
use core::{
    cell::Cell,
    future::{Ready, ready},
};
use reovim_kernel_interface::{
    Contract, Error, Message, ResourceId,
    buffer::integer,
    driver::{DriverOps, Lifecycle, Progress},
    module::{Factory, Module},
    provider::{
        Contribution, Permissions, ResourceOps, ResourceProvider, Surface,
    },
};

/// Declares an inert counter; Runtime drives construction and publication.
/// ```
/// let factory = reovim_module_counter::factory();
/// assert!(factory.dependencies.is_empty());
/// ```
#[must_use]
pub fn factory() -> Factory {
    Factory::new((), |(), _| Ok(construct()), Rc::new(()))
}

fn construct() -> Module {
    Module::new(
        Lifecycle::Constructed,
        DriverOps {
            status: |state| *state,
            advance: |state, _| {
                if *state == Lifecycle::Closed {
                    return Ok(Progress::Stopped);
                }
                *state = Lifecycle::Ready;
                Ok(Progress::Idle)
            },
            close: |state| {
                *state = Lifecycle::Closed;
                Ok(())
            },
        },
        vec![Contribution::Resource {
            path: "modules/counter/value".into(),
            provider: provider(Rc::new(Cell::new(0))),
        }],
        Rc::new(()),
    )
}

/// Adds little-endian u64 values through `external.counter` v1.
/// ```
/// use std::{cell::Cell, rc::Rc};
/// let value = Rc::new(Cell::new(0));
/// let provider = reovim_module_counter::provider(value.clone());
/// // Assembly registers provider; reads observe value, writes add to it.
/// assert_eq!(value.get(), 0);
/// ```
#[must_use]
pub fn provider(value: Rc<Cell<u64>>) -> ResourceProvider {
    ResourceProvider::new(
        value,
        ResourceOps {
            unload: None,
            read: |value, subject, input| {
                Box::pin(read_value(value, subject, input))
            },
            write: |value, subject, input| {
                Box::pin(add_value(value, subject, input))
            },
        },
        Box::new([Surface {
            contract: Contract::new("external.counter", 1),
            methods: Permissions::ALL,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
}

fn read_value(
    value: &Cell<u64>,
    _: ResourceId,
    input: Message,
) -> Ready<Result<Message, Error>> {
    ready(if input.data.is_empty() {
        Ok(Message::new(input.contract, &value.get().to_le_bytes()))
    } else {
        Err(Error::INVALID)
    })
}
fn add_value(
    value: &Cell<u64>,
    _: ResourceId,
    input: Message,
) -> Ready<Result<Message, Error>> {
    ready(integer(&input.data).and_then(|amount| {
        let next = value.get().checked_add(amount).ok_or(Error::PRESSURE)?;
        value.set(next);
        Ok(Message::new(input.contract, &next.to_le_bytes()))
    }))
}
fn native_state(_: reovim_module::Connection) -> Cell<u64> {
    Cell::new(0)
}
const COUNTER: reovim_module::Contract =
    reovim_module::Contract::new("external.counter", 1);
reovim_module::rv_module! {
    pub MODULE {
        contract: ("counter", 1),
        state: native_state,
        resources: {
            "value" => {
                read COUNTER => read_value,
                write COUNTER => add_value,
            },
        },
    }
}
