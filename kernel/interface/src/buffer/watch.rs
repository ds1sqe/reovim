use super::{BufferState, RetainedState, integer};
use crate::{
    Connection, Contract, Error, Message, ResourceId, access::Reply,
    notify::Next,
};

/// Snapshot and subscription created at the same Buffer boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Watching {
    pub snapshot: RetainedState,
    pub history: ResourceId,
    pub watch: ResourceId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transition {
    Edit,
    Undo,
    Redo,
}

/// Accepted edge in the observed Buffer's current finite History profile.
///
/// Revisions and node ordinals belong to the observation's Buffer/History.
/// `request` correlates execution; it is not a Resource access grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Change {
    pub request: u64,
    pub before: u64,
    pub after: u64,
    pub from_node: u64,
    pub to_node: u64,
    pub transition: Transition,
}

/// Acquires an immutable Model snapshot and an independent change watch.
#[must_use = "Retain both owned outputs or recover the original request"]
pub fn observe(connection: &Connection, buffer: ResourceId) -> Reply {
    connection.read(
        buffer,
        &Message::new(Contract::new("kernel.buffer.observe", 1), &[]),
    )
}

/// Reads the next delivery result; `wait = false` permits Pending.
#[must_use = "Await delivery or retain the original request for recovery"]
pub fn next(connection: &Connection, watch: ResourceId, wait: bool) -> Reply {
    connection.read(
        watch,
        &Message::new(
            Contract::new("kernel.buffer.next", 1),
            if wait { &[] } else { &[0] },
        ),
    )
}

/// Decodes the coherent boundary and its separately owned outputs.
/// # Errors
/// Rejects foreign or malformed observation replies.
pub fn watching(reply: &Message) -> Result<Watching, Error> {
    if reply.contract != Contract::new("kernel.buffer.observe", 1)
        || reply.resources.len() != 4
        || reply.data.len() != 16
    {
        return Err(Error::INVALID);
    }
    Ok(Watching {
        snapshot: RetainedState {
            id: reply.resources[2],
            state: BufferState {
                buffer: reply.resources[0],
                revision: integer(&reply.data[..8])?,
            },
            node: integer(&reply.data[8..])?,
        },
        history: reply.resources[1],
        watch: reply.resources[3],
    })
}

/// Decodes delivery without treating gaps or closure as empty content.
/// # Errors
/// Rejects foreign contracts, unknown tags and malformed transition data.
pub fn change(reply: &Message) -> Result<Next<Change>, Error> {
    if reply.contract != Contract::new("kernel.buffer.next", 1)
        || !reply.resources.is_empty()
    {
        return Err(Error::INVALID);
    }
    match reply.data.as_ref() {
        [1] => Ok(Next::Gap),
        [2, lost @ 0..=1] => Ok(Next::End { lost: *lost != 0, error: None }),
        [3] => Ok(Next::Pending),
        [0, kind, data @ ..] if data.len() == 40 => {
            let transition = match kind {
                0 => Transition::Edit,
                1 => Transition::Undo,
                2 => Transition::Redo,
                _ => return Err(Error::INVALID),
            };
            Ok(Next::Item(Change {
                request: integer(&data[..8])?,
                before: integer(&data[8..16])?,
                after: integer(&data[16..24])?,
                from_node: integer(&data[24..32])?,
                to_node: integer(&data[32..])?,
                transition,
            }))
        }
        _ => Err(Error::INVALID),
    }
}
