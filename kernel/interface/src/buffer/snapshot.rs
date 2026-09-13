use super::{BufferState, integer};
use crate::{Connection, Contract, Error, Message, ResourceId, access::Reply};

/// An owned read access to exact immutable Model state, separate from Buffer.
///
/// Read `id` using the Model's read contract and payload; close it to release.
/// Copying this metadata does not create another access.
/// `Connection::read_view` acquires an independent data-only reader of this
/// selected state. It survives close of the original owned snapshot access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetainedState {
    pub id: ResourceId,
    pub state: BufferState,
    pub node: u64,
}

/// Retains a read-only state access if the Buffer is still exactly current.
///
/// The access survives edits and shared Buffer close. Its owner releases it
/// with `Connection::close`; request/result cleanup does not release it.
#[must_use = "Retain the returned access or recover its request outcome"]
pub fn retain(connection: &Connection, expected: BufferState) -> Reply {
    connection.read(
        expected.buffer,
        &Message::new(
            Contract::new("kernel.buffer.retain", 1),
            &expected.revision.to_le_bytes(),
        ),
    )
}

/// Decodes a successful retain reply without inventing a state qualifier.
/// # Errors
/// Rejects a foreign contract or malformed state/access metadata.
pub fn retained(reply: &Message) -> Result<RetainedState, Error> {
    if reply.contract != Contract::new("kernel.buffer.retain", 1)
        || reply.data.len() != 16
        || reply.resources.len() != 2
    {
        return Err(Error::INVALID);
    }
    Ok(RetainedState {
        id: reply.resources[1],
        state: BufferState {
            buffer: reply.resources[0],
            revision: integer(&reply.data[..8])?,
        },
        node: integer(&reply.data[8..])?,
    })
}
