use super::Observation;
use crate::{
    Connection, Contract, Error, Message, ResourceId, access::Reply,
    notify::Next,
};

/// Snapshot and independent change reader acquired at one Session boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Watching {
    pub observation: Observation,
    pub revision: u64,
    pub watch: ResourceId,
}
/// An accepted context transition; request is correlation, not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Change {
    pub request: u64,
    pub before: u64,
    pub after: u64,
}

#[must_use = "Adopt or recover both returned owners"]
pub fn observe(connection: &Connection, session: ResourceId) -> Reply {
    connection.read(
        session,
        &Message::new(Contract::new("kernel.session.observe", 1), &[]),
    )
}
#[must_use = "Observe or recover the delivered event"]
pub fn next(connection: &Connection, watch: ResourceId, wait: bool) -> Reply {
    connection.read(
        watch,
        &Message::new(
            Contract::new("kernel.session.next", 1),
            if wait { &[] } else { &[0] },
        ),
    )
}

/// Decodes currentness, owned handles and the Model's semantic value.
/// # Errors
/// Rejects malformed contracts, incomplete metadata or invalid UTF-8 names.
pub fn watching(reply: &Message) -> Result<Watching, Error> {
    if reply.contract != Contract::new("kernel.session.observe", 1) {
        return Err(Error::INVALID);
    }
    decode(reply)
}

/// Decodes the coherent live view acquired through owned participation.
/// # Errors
/// Requires the follow contract and the same exact observation envelope.
pub fn followed(reply: &Message) -> Result<Watching, Error> {
    if reply.contract != Contract::new("kernel.session.follow", 1) {
        return Err(Error::INVALID);
    }
    decode(reply)
}

fn decode(reply: &Message) -> Result<Watching, Error> {
    let [session, snapshot, watch, values @ ..] = reply.resources.as_ref()
    else {
        return Err(Error::INVALID);
    };
    let prefix = reply.data.get(..16).ok_or(Error::INVALID)?;
    let revision = integer(&prefix[..8])?;
    let schema = u32::from_le_bytes(
        prefix[8..12].try_into().map_err(|_| Error::INVALID)?,
    );
    let length = u32::from_le_bytes(
        prefix[12..].try_into().map_err(|_| Error::INVALID)?,
    );
    let end = 16_usize
        .checked_add(usize::try_from(length).map_err(|_| Error::INVALID)?)
        .ok_or(Error::INVALID)?;
    let name =
        core::str::from_utf8(reply.data.get(16..end).ok_or(Error::INVALID)?)
            .map_err(|_| Error::INVALID)?;
    let value = Message {
        contract: Contract::new(name, schema),
        data: reply.data[end..].into(),
        resources: values.into(),
    };
    Ok(Watching {
        revision,
        watch: *watch,
        observation: Observation {
            session: *session,
            snapshot: *snapshot,
            value,
        },
    })
}

/// Decodes change, gap, end or passive pending without inventing context data.
/// # Errors
/// Rejects another surface, reference-bearing events or malformed payloads.
pub fn change(reply: &Message) -> Result<Next<Change>, Error> {
    if reply.contract != Contract::new("kernel.session.next", 1)
        || !reply.resources.is_empty()
    {
        return Err(Error::INVALID);
    }
    match reply.data.as_ref() {
        [1] => Ok(Next::Gap),
        [2, lost @ 0..=1] => Ok(Next::End { lost: *lost != 0, error: None }),
        [3] => Ok(Next::Pending),
        [0, data @ ..] if data.len() == 24 => Ok(Next::Item(Change {
            request: integer(&data[..8])?,
            before: integer(&data[8..16])?,
            after: integer(&data[16..])?,
        })),
        _ => Err(Error::INVALID),
    }
}
fn integer(bytes: &[u8]) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(bytes.try_into().map_err(|_| Error::INVALID)?))
}
