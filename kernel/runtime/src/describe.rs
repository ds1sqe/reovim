use alloc::boxed::Box;
use reovim_kernel_interface::{Contract, Error, Message, ResourceId};

pub fn contract(value: &Contract) -> bool {
    value == &Contract::new("kernel.buffer.describe", 1)
}

pub fn read(buffer: &crate::Editing, input: Message) -> Result<Message, Error> {
    if !input.data.is_empty() || !input.resources.is_empty() {
        return Err(Error::INVALID);
    }
    let mut reply =
        Message::new(input.contract, &buffer.state.revision().to_le_bytes());
    reply.resources = match &buffer.backing {
        Some(backing) => Box::new([
            ResourceId(buffer.resource),
            ResourceId(buffer.history),
            ResourceId(backing.source),
        ]),
        None => {
            Box::new([ResourceId(buffer.resource), ResourceId(buffer.history)])
        }
    };
    Ok(reply)
}
