//! Scalar positions belong to an interpreted snapshot, never the Kernel.
#![allow(clippy::future_not_send)]

use alloc::{boxed::Box, vec::Vec};
use core::ops::Range;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    buffer::{self, BufferState},
    notify::Next,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unit {
    Scalar(char),
    Opaque(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observed {
    pub representation: ResourceId,
    pub space: ResourceId,
    pub buffer: BufferState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observation {
    observed: Observed,
    snapshot: ResourceId,
    watch: ResourceId,
    history: ResourceId,
    byte_len: u64,
}

impl Observation {
    #[must_use]
    pub const fn observed(&self) -> Observed {
        self.observed
    }
    #[must_use]
    pub const fn snapshot(&self) -> ResourceId {
        self.snapshot
    }
    #[must_use]
    pub const fn watch(&self) -> ResourceId {
        self.watch
    }
    #[must_use]
    pub const fn history(&self) -> ResourceId {
        self.history
    }
    #[must_use]
    /// Source length: UTF-8 width per scalar and one byte per opaque unit.
    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Change {
    pub before: Observed,
    pub after: Observed,
    pub history: ResourceId,
    pub source: buffer::Change,
}

pub struct Window {
    pub observed: Observed,
    pub snapshot: ResourceId,
    pub range: Range<u64>,
    pub units: Vec<Unit>,
    pub eof: bool,
}

/// Attaches in this client's Kernel module scope; no new Buffer is created.
/// # Errors
/// Rejects unsupported source access or insufficient caller rights.
pub async fn attach(
    kernel: &Connection,
    factory: ResourceId,
    buffer: ResourceId,
    writable: bool,
) -> Result<ResourceId, RequestError> {
    let name = if writable { "text.attach" } else { "text.attach.read" };
    let reply = kernel
        .write(factory, &Message::reference(Contract::new(name, 1), buffer))
        .await?;
    attachment(&reply, writable).map_err(Into::into)
}

/// Validates an attachment reply for native or remote consumers.
/// # Errors
/// Rejects a foreign contract, payload or reference layout.
pub fn attachment(
    reply: &Message,
    writable: bool,
) -> Result<ResourceId, Error> {
    let name = if writable { "text.attach" } else { "text.attach.read" };
    if reply.contract != Contract::new(name, 1)
        || !reply.data.is_empty()
        || reply.resources.len() != 1
    {
        return Err(Error::PROVIDER);
    }
    Ok(reply.resources[0])
}

/// Acquires independently owned semantic snapshot and watch at one source cut.
/// # Errors
/// Rejects closed interpretation, pressure or failed source observation.
pub async fn observe(
    kernel: &Connection,
    view: ResourceId,
) -> Result<Observation, RequestError> {
    let reply = kernel
        .read(view, &Message::new(Contract::new("text.observe", 1), &[]))
        .await?;
    observation(&reply, view).map_err(Into::into)
}

/// Validates a coherent semantic observation from the selected interpretation.
/// # Errors
/// Rejects foreign identity or malformed semantic metadata.
pub fn observation(
    reply: &Message,
    view: ResourceId,
) -> Result<Observation, Error> {
    if reply.contract != Contract::new("text.observe", 1)
        || reply.resources.len() != 6
        || reply.data.len() != 24
        || reply.resources[0] != view
        || reply.resources[1] != view
    {
        return Err(Error::PROVIDER);
    }
    Ok(Observation {
        observed: Observed {
            representation: reply.resources[0],
            space: reply.resources[1],
            buffer: BufferState {
                buffer: reply.resources[2],
                revision: buffer::integer(&reply.data[..8])?,
            },
        },
        history: reply.resources[3],
        snapshot: reply.resources[4],
        watch: reply.resources[5],
        byte_len: buffer::integer(&reply.data[16..])?,
    })
}

/// Reads up to 1024 scalar/opaque units, returning the actual range and EOF.
/// # Errors
/// Rejects expired snapshots, invalid positions or malformed data.
pub async fn read(
    kernel: &Connection,
    observation: &Observation,
    start: u64,
    count: u64,
) -> Result<Window, RequestError> {
    let reply = kernel
        .read(
            observation.snapshot,
            &Message::new(
                Contract::new("text.window", 1),
                &[start.to_le_bytes(), count.to_le_bytes()].concat(),
            ),
        )
        .await?;
    window(&reply, observation, start, count).map_err(Into::into)
}

/// Decodes exact scalar/opaque units for a retained observation and range.
/// # Errors
/// Rejects invalid units, foreign ranges and malformed wire values.
pub fn window(
    reply: &Message,
    observation: &Observation,
    start: u64,
    count: u64,
) -> Result<Window, Error> {
    if reply.contract != Contract::new("text.window", 1)
        || !reply.resources.is_empty()
        || reply.data.len() < 17
        || reply.data[0] > 1
    {
        return Err(Error::PROVIDER);
    }
    let first = buffer::integer(&reply.data[1..9])?;
    let end = buffer::integer(&reply.data[9..17])?;
    let units = decode(&reply.data[17..])?;
    if first != start
        || end.checked_sub(start) != Some(units.len() as u64)
        || units.len() as u64 > count
    {
        return Err(Error::PROVIDER);
    }
    Ok(Window {
        observed: observation.observed,
        snapshot: observation.snapshot,
        range: start..end,
        units,
        eof: reply.data[0] != 0,
    })
}

/// Replaces a range qualified by this interpreted snapshot with valid UTF-8.
/// # Errors
/// Reports denied edits, stale/foreign snapshots or failed source lowering.
pub async fn replace(
    kernel: &Connection,
    observation: &Observation,
    range: Range<u64>,
    text: &str,
) -> Result<buffer::Observation, RequestError> {
    let mut data =
        [range.start.to_le_bytes(), range.end.to_le_bytes()].concat();
    data.extend_from_slice(text.as_bytes());
    let input = Message {
        contract: Contract::new("text.replace", 1),
        data: data.into(),
        resources: Box::new([observation.snapshot]),
    };
    let reply =
        kernel.write(observation.observed.representation, &input).await?;
    buffer::observation(reply).map_err(Into::into)
}

/// Reads this interpretation's accepted source-change feed.
/// # Errors
/// Reports failed delivery; gaps/end remain explicit stream values.
pub async fn next(
    kernel: &Connection,
    observation: &Observation,
    wait: bool,
) -> Result<Next<Change>, RequestError> {
    let mut reply = kernel
        .read(
            observation.watch,
            &Message::new(
                Contract::new("text.next", 1),
                if wait { &[] } else { &[0] },
            ),
        )
        .await?;
    if reply.contract != Contract::new("text.next", 1) {
        return Err(Error::PROVIDER.into());
    }
    reply.contract = Contract::new("kernel.buffer.next", 1);
    Ok(match buffer::change(&reply)? {
        Next::Item(source) => Next::Item(Change {
            before: qualify(observation, source.before),
            after: qualify(observation, source.after),
            history: observation.history,
            source,
        }),
        Next::Pending => Next::Pending,
        Next::Gap => Next::Gap,
        Next::End { lost, error } => Next::End { lost, error },
    })
}

const fn qualify(observation: &Observation, revision: u64) -> Observed {
    Observed {
        buffer: BufferState { revision, ..observation.observed.buffer },
        ..observation.observed
    }
}

pub(crate) fn encode(unit: Unit, data: &mut Vec<u8>) {
    let (tag, value) = match unit {
        Unit::Scalar(c) => (0, u32::from(c)),
        Unit::Opaque(byte) => (1, u32::from(byte)),
    };
    data.push(tag);
    data.extend_from_slice(&value.to_le_bytes());
}

fn decode(data: &[u8]) -> Result<Vec<Unit>, Error> {
    if !data.len().is_multiple_of(5) {
        return Err(Error::PROVIDER);
    }
    data.as_chunks::<5>()
        .0
        .iter()
        .map(|item| {
            let value =
                u32::from_le_bytes([item[1], item[2], item[3], item[4]]);
            match item[0] {
                0 => char::from_u32(value)
                    .map(Unit::Scalar)
                    .ok_or(Error::PROVIDER),
                1 => u8::try_from(value)
                    .map(Unit::Opaque)
                    .map_err(|_| Error::PROVIDER),
                _ => Err(Error::PROVIDER),
            }
        })
        .collect()
}
