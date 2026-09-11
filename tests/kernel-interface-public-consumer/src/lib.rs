#![no_std]
//! A separately compiled provider requiring only the public Interface.

extern crate alloc;

use alloc::{boxed::Box, rc::Rc};
use core::cell::Cell;
use reovim_kernel_interface::provider::{
    Permissions, ProviderReply, ResourceOps, ResourceProvider, Surface,
};
use reovim_kernel_interface::{Contract, Error, Message, ResourceId};

/// New surface outside Kernel: registration requires no Kernel specialization.
#[must_use]
pub fn counter(value: Rc<Cell<u64>>) -> ResourceProvider {
    ResourceProvider::new(
        value,
        ResourceOps { unload: None, read, write },
        Box::new([Surface {
            contract: Contract::new("external.counter", 1),
            methods: Permissions::ALL,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
}

fn read(
    value: &mut Rc<Cell<u64>>,
    _: ResourceId,
    input: Message,
) -> ProviderReply {
    let reply = Message::new(input.contract, &value.get().to_le_bytes());
    Box::pin(async move { Ok(reply) })
}

fn write(
    value: &mut Rc<Cell<u64>>,
    _: ResourceId,
    input: Message,
) -> ProviderReply {
    let result = add(value, input);
    Box::pin(async move { result })
}

fn add(value: &Cell<u64>, input: Message) -> Result<Message, Error> {
    let delta = reovim_kernel_interface::buffer::integer(&input.data)?;
    let next = value.get().checked_add(delta).ok_or(Error::PRESSURE)?;
    value.set(next);
    Ok(Message::new(input.contract, &next.to_le_bytes()))
}
