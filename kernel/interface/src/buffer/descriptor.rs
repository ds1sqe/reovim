use super::BufferState;
use crate::{Connection, Contract, Error, Message, ResourceId, access::Reply};

/// Coherent Buffer metadata; copied identities own no lifetime lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BufferDescriptor {
    pub state: BufferState,
    pub history: ResourceId,
    pub backing: Option<ResourceId>,
}

/// Reads Buffer identity/currentness without inspecting its concrete Model.
#[must_use = "Observe the metadata reply or recover the request"]
pub fn describe(connection: &Connection, buffer: ResourceId) -> Reply {
    connection.read(
        buffer,
        &Message::new(Contract::new("kernel.buffer.describe", 1), &[]),
    )
}

/// Decodes a descriptor, preserving the distinction between backing and input.
/// # Errors
/// Rejects malformed metadata or another contract's reply.
pub fn descriptor(reply: &Message) -> Result<BufferDescriptor, Error> {
    if reply.contract != Contract::new("kernel.buffer.describe", 1)
        || reply.data.len() != 8
    {
        return Err(Error::INVALID);
    }
    let (buffer, history, backing) = match reply.resources.as_ref() {
        [buffer, history] => (*buffer, *history, None),
        [buffer, history, backing] => (*buffer, *history, Some(*backing)),
        _ => return Err(Error::INVALID),
    };
    let revision = u64::from_le_bytes(
        reply.data.as_ref().try_into().map_err(|_| Error::INVALID)?,
    );
    Ok(BufferDescriptor {
        state: BufferState { buffer, revision },
        history,
        backing,
    })
}
