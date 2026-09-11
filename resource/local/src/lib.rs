#![no_std]
#![forbid(unsafe_code)]
//! Immutable sources and public native values for the reference models.

extern crate alloc;

use alloc::{boxed::Box, collections::BTreeMap, rc::Rc, vec::Vec};
use reovim_kernel_interface::provider::{
    Permissions, ProviderReply, ResourceOps, ResourceProvider, Surface,
};
use reovim_kernel_interface::{Contract, Error, Message, ResourceId};

pub struct Bytes(pub Vec<u8>);
pub struct Keys(pub BTreeMap<u64, bool>);

#[must_use]
pub fn memory(data: &[u8]) -> ResourceProvider {
    ResourceProvider::new(
        Bytes(data.to_vec()),
        ResourceOps { unload: None, read: read_memory, write: unsupported },
        Box::new([Surface {
            contract: Contract::new("resource.bytes", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
}

fn read_memory(
    data: &mut Bytes,
    _: ResourceId,
    input: Message,
) -> ProviderReply {
    let result = if input.data.is_empty() {
        Ok(Message::new(input.contract, &data.0))
    } else {
        Err(Error::INVALID)
    };
    Box::pin(async move { result })
}

fn unsupported(_: &mut Bytes, _: ResourceId, _: Message) -> ProviderReply {
    Box::pin(async { Err(Error::UNSUPPORTED) })
}
