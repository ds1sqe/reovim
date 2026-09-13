//! Explicit service-side Session checkpoints, not a crash journal.

use crate::{remote::Remote, session::Session};
use reovim_client_tui::operation::{Change, Operation};
use reovim_client_tui::view::View;
use reovim_kernel_interface::{Contract, Message, ResourceId, buffer};
use reovim_module_text::api;
use reovim_service::request::{Checkpoint, Request};
use std::io::Write;
use std::io::{self, Cursor, Read};

const LIMIT: usize = 3 * reovim_service::MAX_LINE;
const FILE_LIMIT: usize = 35 * reovim_service::MAX_LINE;
mod input;
mod submitted;
mod validate;

pub fn argument(args: &mut Vec<String>) -> io::Result<Option<String>> {
    if args.len() == 6 && args[4] == "--checkpoint" {
        let path = args.pop().ok_or_else(invalid)?;
        args.pop();
        return Ok(Some(path));
    }
    Ok(None)
}

pub fn save(
    path: &str,
    session: &Session,
    factory: ResourceId,
    view: &View,
) -> io::Result<()> {
    let bytes = encode_complete(session, factory, view)?;
    let mut file = private_file(path)?;
    file.write_all(&bytes)?;
    file.sync_all()
}

#[cfg(unix)]
fn private_file(path: &str) -> io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true).mode(0o600).open(path)
}

#[cfg(not(unix))]
fn private_file(_: &str) -> io::Result<std::fs::File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "private checkpoint permissions unavailable",
    ))
}

pub fn inspect(path: &str) -> io::Result<()> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((FILE_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)?;
    let (session, _, input) = decode_complete(&bytes)?;
    println!(
        "Buffer @{}; operation {:?}; receipt {:?}",
        session.buffer.0,
        session.operation,
        session.remote.pending.as_ref().map(Request::id)
    );
    if let Some(input) = input {
        println!(
            "Input basis {:?}; cursor {}; planned {:?}; pending {}; \
            halted {}; queued {}; submitted {}",
            input.basis,
            input.cursor,
            input.planned,
            input.pending,
            input.halted,
            input.queued.len(),
            input.submitted.is_some()
        );
    }
    Ok(())
}

pub fn encode_complete(
    session: &Session,
    factory: ResourceId,
    view: &View,
) -> io::Result<Vec<u8>> {
    let input = view.retained_input().map_err(io::Error::other)?;
    let service = encode(session, factory)?;
    compose(&service, &input)
}

pub fn compose(
    service: &[u8],
    input: &reovim_client_tui::custody::RetainedInput,
) -> io::Result<Vec<u8>> {
    let (session, _) = decode(service)?;
    submitted::validate(&session, input.submitted.as_ref(), input.planned)?;
    if input.basis.is_some_and(|basis| basis.buffer != session.buffer) {
        return Err(invalid());
    }
    let mut bytes = Vec::from(*b"RVTUI003");
    number(&mut bytes, service.len() as u64);
    bytes.extend(service);
    input::encode(&mut bytes, input)?;
    if bytes.len() > FILE_LIMIT {
        return Err(invalid());
    }
    Ok(bytes)
}

pub fn decode_complete(
    bytes: &[u8],
) -> io::Result<(Session, ResourceId, Option<input::RetainedInput>)> {
    if bytes.starts_with(b"RVTUI001") {
        let (session, factory) = decode(bytes)?;
        return Ok((session, factory, None));
    }
    let with_submitted = bytes.starts_with(b"RVTUI003");
    if bytes.len() > FILE_LIMIT
        || !(with_submitted || bytes.starts_with(b"RVTUI002"))
    {
        return Err(invalid());
    }
    let mut reader = Decoder(Cursor::new(&bytes[8..]));
    let size = reader.number()?;
    if size > LIMIT as u64 {
        return Err(invalid());
    }
    let mut service = vec![0; usize::try_from(size).map_err(|_| invalid())?];
    reader.0.read_exact(&mut service)?;
    let (session, factory) = decode(&service)?;
    let input = input::decode(&mut reader, with_submitted)?;
    if with_submitted {
        submitted::validate(&session, input.submitted.as_ref(), input.planned)?;
    }
    if reader.0.position() != (bytes.len() - 8) as u64
        || input.basis.is_some_and(|basis| basis.buffer != session.buffer)
        || input
            .submitted
            .as_ref()
            .is_some_and(|value| value.basis.buffer != session.buffer)
    {
        return Err(invalid());
    }
    Ok((session, factory, Some(input)))
}

pub fn encode(session: &Session, factory: ResourceId) -> io::Result<Vec<u8>> {
    validate::owner(session, factory)?;
    let mut out = Vec::from(*b"RVTUI001");
    string(&mut out, &session.remote.address)?;
    string(&mut out, &session.remote.client)?;
    number(&mut out, factory.0);
    number(&mut out, session.buffer.0);
    optional(&mut out, session.access.map(|id| id.0));
    operation(&mut out, session.operation);
    out.push(u8::from(session.observed.is_some()));
    if let Some(value) = session.observed {
        for n in [
            value.observed().buffer.revision,
            value.byte_len(),
            value.observed().representation.0,
            value.observed().space.0,
            value.history().0,
            value.snapshot().0,
            value.watch().0,
        ] {
            number(&mut out, n);
        }
    }
    number(&mut out, session.retiring.len() as u64);
    for id in &session.retiring {
        number(&mut out, id.0);
    }
    optional(&mut out, session.accepted.map(|state| state.revision));
    optional(&mut out, session.remote.reserved);
    out.push(u8::from(session.remote.pending.is_some()));
    if let Some(request) = &session.remote.pending {
        let saved = request.checkpoint();
        string(&mut out, &saved.frame)?;
        out.extend([
            u8::from(saved.sent),
            u8::from(saved.adopted),
            u8::from(saved.released),
            u8::from(saved.response.is_some()),
        ]);
        if let Some(reply) = saved.response {
            string(&mut out, &reply)?;
        }
    }
    if out.len() > LIMIT {
        return Err(invalid());
    }
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> io::Result<(Session, ResourceId)> {
    if bytes.len() > LIMIT || !bytes.starts_with(b"RVTUI001") {
        return Err(invalid());
    }
    let mut input = Decoder(Cursor::new(&bytes[8..]));
    let address = input.string()?;
    let client = input.string()?;
    if client.len() != 32 || !client.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    let factory = ResourceId(input.number()?);
    let buffer = ResourceId(input.number()?);
    let access = input.optional()?.map(ResourceId);
    let operation = input.operation()?;
    let observed = input.observation(buffer, access)?;
    let count = input.number()?;
    if count > 3 {
        return Err(invalid());
    }
    let mut retiring = Vec::new();
    for _ in 0..count {
        retiring.push(ResourceId(input.number()?));
    }
    let accepted = input
        .optional()?
        .map(|revision| buffer::BufferState { buffer, revision });
    let reserved = input.optional()?;
    let pending = input.request(&client)?;
    if input.0.position() != (bytes.len() - 8) as u64
        || (reserved.is_some() && pending.is_some())
        || (matches!(
            operation,
            Operation::Context { .. } | Operation::Change(_)
        ) && observed.is_none())
    {
        return Err(invalid());
    }
    let session = Session {
        remote: Remote { address, client, pending, reserved },
        buffer,
        access,
        operation,
        observed,
        retiring,
        accepted,
        cancel: std::sync::Arc::default(),
        journal: None,
    };
    validate::owner(&session, factory)?;
    Ok((session, factory))
}

fn number(out: &mut Vec<u8>, value: u64) {
    out.extend(value.to_le_bytes());
}

fn optional(out: &mut Vec<u8>, value: Option<u64>) {
    out.push(u8::from(value.is_some()));
    if let Some(value) = value {
        number(out, value);
    }
}

fn string(out: &mut Vec<u8>, value: &str) -> io::Result<()> {
    if value.len() >= reovim_service::MAX_LINE {
        return Err(invalid());
    }
    number(out, value.len() as u64);
    out.extend(value.as_bytes());
    Ok(())
}

fn operation(out: &mut Vec<u8>, value: Operation) {
    let (tag, first, second) = match value {
        Operation::Idle => (0, 0, None),
        Operation::Attach(id) => (1, id.0, None),
        Operation::Retire { origin } => (2, origin, None),
        Operation::Observe { origin } => (3, origin, None),
        Operation::Context { origin, reading } => (4, origin, reading),
        Operation::Change(Change::Replace { origin }) => (5, origin, None),
        Operation::Change(Change::History { redo }) => {
            (6, u64::from(redo), None)
        }
        Operation::Closing => (7, 0, None),
    };
    out.push(tag);
    number(out, first);
    out.push(u8::from(second.is_some()));
    if let Some((start, count)) = second {
        number(out, start);
        number(out, count);
    }
}

struct Decoder<'a>(Cursor<&'a [u8]>);

impl Decoder<'_> {
    fn byte(&mut self) -> io::Result<u8> {
        let mut value = [0];
        self.0.read_exact(&mut value)?;
        Ok(value[0])
    }
    fn flag(&mut self) -> io::Result<bool> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid()),
        }
    }
    fn number(&mut self) -> io::Result<u64> {
        let mut value = [0; 8];
        self.0.read_exact(&mut value)?;
        Ok(u64::from_le_bytes(value))
    }
    fn optional(&mut self) -> io::Result<Option<u64>> {
        if self.flag()? { Ok(Some(self.number()?)) } else { Ok(None) }
    }
    fn string(&mut self) -> io::Result<String> {
        let size = self.number()?;
        if size >= reovim_service::MAX_LINE as u64 {
            return Err(invalid());
        }
        let mut bytes = vec![0; usize::try_from(size).map_err(|_| invalid())?];
        self.0.read_exact(&mut bytes)?;
        String::from_utf8(bytes).map_err(|_| invalid())
    }
    fn operation(&mut self) -> io::Result<Operation> {
        let tag = self.byte()?;
        let first = self.number()?;
        let reading = if self.flag()? {
            Some((self.number()?, self.number()?))
        } else {
            None
        };
        if tag != 4 && reading.is_some() {
            return Err(invalid());
        }
        match tag {
            0 if first == 0 => Ok(Operation::Idle),
            1 => Ok(Operation::Attach(ResourceId(first))),
            2 => Ok(Operation::Retire { origin: first }),
            3 => Ok(Operation::Observe { origin: first }),
            4 => Ok(Operation::Context { origin: first, reading }),
            5 => Ok(Operation::Change(Change::Replace { origin: first })),
            6 if first <= 1 => {
                Ok(Operation::Change(Change::History { redo: first == 1 }))
            }
            7 if first == 0 => Ok(Operation::Closing),
            _ => Err(invalid()),
        }
    }
    fn observation(
        &mut self,
        buffer: ResourceId,
        access: Option<ResourceId>,
    ) -> io::Result<Option<api::Observation>> {
        if !self.flag()? {
            return Ok(None);
        }
        let revision = self.number()?;
        let length = self.number()?;
        let mut ids = vec![
            ResourceId(self.number()?),
            ResourceId(self.number()?),
            buffer,
        ];
        for _ in 0..3 {
            ids.push(ResourceId(self.number()?));
        }
        let value = Message {
            contract: Contract::new("text.observe", 1),
            data: [
                revision.to_le_bytes(),
                0_u64.to_le_bytes(),
                length.to_le_bytes(),
            ]
            .concat()
            .into(),
            resources: ids.into(),
        };
        api::observation(&value, access.ok_or_else(invalid)?)
            .map(Some)
            .map_err(io::Error::other)
    }
    fn request(&mut self, client: &str) -> io::Result<Option<Request>> {
        if !self.flag()? {
            return Ok(None);
        }
        let frame = self.string()?;
        let sent = self.flag()?;
        let adopted = self.flag()?;
        let released = self.flag()?;
        let response = if self.flag()? { Some(self.string()?) } else { None };
        if released {
            return Err(invalid());
        }
        Request::restore(
            client,
            Checkpoint { frame, sent, adopted, released, response },
        )
        .map(Some)
        .map_err(io::Error::other)
    }
}

fn invalid() -> io::Error {
    io::Error::other("invalid Session checkpoint")
}
