//! The wire: meaning only.
//!
//! A sequence and one `Message`, as one value in the encoding of
//! 00-core.md. A transport moves whole messages and may be unordered or
//! lossy; framing, sync and checksums are its own. Each end owns its
//! reliability: the Kernel's reply window on the server, an `Endpoint`
//! on the client that knows what went unanswered.

use alloc::string::String;
use alloc::vec::Vec;

use crate::codec::{
    Reader, put_bytes, put_str, put_u8, put_u16, put_u32, put_u64,
};
use crate::op::{Op, Out};
use crate::types::{
    Error, Event, Handle, Hash, Info, Kind, Rd, Reply, Rights, Token,
};

/// One message either way.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    /// First on a link: who the client is and which Session it wants;
    /// `None` asks for a new one. Answered with `Out::Rds([session,
    /// client])`.
    Connect { credential: Vec<u8>, session: Option<Rd> },
    /// One operation for the Kernel.
    Request(Op),
    /// The answer to the request with the same sequence.
    Reply(Reply<Out>),
}

/// `seq` then the message.
#[must_use]
pub fn encode(seq: u64, message: &Message) -> Vec<u8> {
    let mut out = Vec::new();
    put_u64(&mut out, seq);
    match message {
        Message::Connect { credential, session } => {
            put_u8(&mut out, 0);
            put_bytes(&mut out, credential);
            put_option(&mut out, session.map(|rd| rd.0));
        }
        Message::Request(op) => {
            put_u8(&mut out, 1);
            put_op(&mut out, op);
        }
        Message::Reply(reply) => {
            put_u8(&mut out, 2);
            put_reply(&mut out, reply);
        }
    }
    out
}

/// The inverse of `encode`; `None` unless `bytes` is exactly one message.
#[must_use]
pub fn decode(bytes: &[u8]) -> Option<(u64, Message)> {
    let mut r = Reader::new(bytes);
    let seq = r.u64()?;
    let message = match r.u8()? {
        0 => {
            let credential = r.bytes()?;
            let session = get_rd(&mut r).ok()?;
            Message::Connect { credential, session }
        }
        1 => Message::Request(get_op(&mut r)?),
        2 => Message::Reply(get_reply(&mut r)?),
        _ => return None,
    };
    r.done().then_some((seq, message))
}

/// The client end: numbers what it sends and remembers it until the
/// reply with that sequence arrives.
#[derive(Debug, Default)]
pub struct Endpoint {
    next: u64,
    outstanding: Vec<(u64, Vec<u8>)>,
}

impl Endpoint {
    /// Encode with the next sequence and remember the bytes.
    pub fn send(&mut self, message: &Message) -> (u64, Vec<u8>) {
        let seq = self.next;
        self.next += 1;
        let bytes = encode(seq, message);
        self.outstanding.push((seq, bytes.clone()));
        (seq, bytes)
    }

    /// Decode a reply and retire its sequence. Undecodable bytes and a
    /// reply to nothing outstanding are dropped.
    pub fn receive(&mut self, bytes: &[u8]) -> Option<(u64, Message)> {
        let (seq, message) = decode(bytes)?;
        let at = self.outstanding.iter().position(|(s, _)| *s == seq)?;
        self.outstanding.remove(at);
        Some((seq, message))
    }

    /// What to send again, oldest first.
    #[must_use]
    pub fn unanswered(&self) -> &[(u64, Vec<u8>)] {
        &self.outstanding
    }
}

fn put_option(out: &mut Vec<u8>, value: Option<u64>) {
    match value {
        None => put_u8(out, 0),
        Some(v) => {
            put_u8(out, 1);
            put_u64(out, v);
        }
    }
}

/// An optional rd: tag 0, or tag 1 then the id. `Err` is a malformed
/// value.
fn get_rd(r: &mut Reader) -> Result<Option<Rd>, ()> {
    match r.u8().ok_or(())? {
        0 => Ok(None),
        1 => r.u64().map(|id| Some(Rd(id))).ok_or(()),
        _ => Err(()),
    }
}

fn put_op(out: &mut Vec<u8>, op: &Op) {
    match op {
        Op::Lookup { rd, name } => {
            put_u8(out, 0);
            put_u64(out, rd.0);
            put_str(out, name);
        }
        Op::Open { rd, rights } => {
            put_u8(out, 1);
            put_u64(out, rd.0);
            put_u32(out, rights.bits());
        }
        Op::Read { h, at, len } => {
            put_u8(out, 2);
            put_u64(out, h.0);
            put_u64(out, *at);
            put_u32(out, *len);
        }
        Op::Write { h, at, bytes } => {
            put_u8(out, 3);
            put_u64(out, h.0);
            put_u64(out, *at);
            put_bytes(out, bytes);
        }
        Op::Call { h, api, method, input } => {
            put_u8(out, 4);
            put_u64(out, h.0);
            put_u64(out, api.0);
            put_u16(out, *method);
            put_bytes(out, input);
        }
        Op::Watch { h, on } => {
            put_u8(out, 5);
            put_u64(out, h.0);
            put_u8(out, u8::from(*on));
        }
        Op::Close { h } => {
            put_u8(out, 6);
            put_u64(out, h.0);
        }
        Op::Ask { token } => {
            put_u8(out, 7);
            put_u64(out, token.0);
        }
        Op::Cancel { token } => {
            put_u8(out, 8);
            put_u64(out, token.0);
        }
        Op::Since { cursor } => {
            put_u8(out, 9);
            put_u64(out, *cursor);
        }
        Op::Info { rd } => {
            put_u8(out, 10);
            put_u64(out, rd.0);
        }
        Op::Parent { rd } => {
            put_u8(out, 11);
            put_u64(out, rd.0);
        }
        Op::Children { rd } => {
            put_u8(out, 12);
            put_u64(out, rd.0);
        }
    }
}

fn get_op(r: &mut Reader) -> Option<Op> {
    let op = match r.u8()? {
        0 => Op::Lookup { rd: Rd(r.u64()?), name: r.str()? },
        1 => {
            Op::Open { rd: Rd(r.u64()?), rights: Rights::from_bits(r.u32()?)? }
        }
        2 => Op::Read { h: Handle(r.u64()?), at: r.u64()?, len: r.u32()? },
        3 => Op::Write { h: Handle(r.u64()?), at: r.u64()?, bytes: r.bytes()? },
        4 => Op::Call {
            h: Handle(r.u64()?),
            api: Hash(r.u64()?),
            method: r.u16()?,
            input: r.bytes()?,
        },
        5 => Op::Watch { h: Handle(r.u64()?), on: r.bool()? },
        6 => Op::Close { h: Handle(r.u64()?) },
        7 => Op::Ask { token: Token(r.u64()?) },
        8 => Op::Cancel { token: Token(r.u64()?) },
        9 => Op::Since { cursor: r.u64()? },
        10 => Op::Info { rd: Rd(r.u64()?) },
        11 => Op::Parent { rd: Rd(r.u64()?) },
        12 => Op::Children { rd: Rd(r.u64()?) },
        _ => return None,
    };
    Some(op)
}

fn put_reply(out: &mut Vec<u8>, reply: &Reply<Out>) {
    match reply {
        Reply::Ready(Ok(value)) => {
            put_u8(out, 0);
            put_out(out, value);
        }
        Reply::Ready(Err(error)) => {
            put_u8(out, 1);
            put_u8(out, error_tag(*error));
        }
        Reply::Pending(token) => {
            put_u8(out, 2);
            put_u64(out, token.0);
        }
    }
}

fn get_reply(r: &mut Reader) -> Option<Reply<Out>> {
    let reply = match r.u8()? {
        0 => Reply::Ready(Ok(get_out(r)?)),
        1 => Reply::Ready(Err(error_from(r.u8()?)?)),
        2 => Reply::Pending(Token(r.u64()?)),
        _ => return None,
    };
    Some(reply)
}

fn put_out(out: &mut Vec<u8>, value: &Out) {
    match value {
        Out::Unit => put_u8(out, 0),
        Out::Rd(rd) => {
            put_u8(out, 1);
            put_u64(out, rd.0);
        }
        Out::Handle(h) => {
            put_u8(out, 2);
            put_u64(out, h.0);
        }
        Out::Len(len) => {
            put_u8(out, 3);
            put_u32(out, *len);
        }
        Out::Bytes(bytes) => {
            put_u8(out, 4);
            put_bytes(out, bytes);
        }
        Out::Info(info) => {
            put_u8(out, 5);
            put_info(out, info);
        }
        Out::Rds(rds) => {
            put_u8(out, 6);
            put_u32(out, count(rds.len()));
            for rd in rds {
                put_u64(out, rd.0);
            }
        }
        Out::Events { events, cursor } => {
            put_u8(out, 7);
            put_u32(out, count(events.len()));
            for event in events {
                put_event(out, event);
            }
            put_u64(out, *cursor);
        }
    }
}

fn get_out(r: &mut Reader) -> Option<Out> {
    let value = match r.u8()? {
        0 => Out::Unit,
        1 => Out::Rd(Rd(r.u64()?)),
        2 => Out::Handle(Handle(r.u64()?)),
        3 => Out::Len(r.u32()?),
        4 => Out::Bytes(r.bytes()?),
        5 => Out::Info(get_info(r)?),
        6 => Out::Rds(get_items(r, |r| r.u64().map(Rd))?),
        7 => Out::Events { events: get_items(r, get_event)?, cursor: r.u64()? },
        _ => return None,
    };
    Some(value)
}

fn count(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

fn get_items<T>(
    r: &mut Reader,
    item: fn(&mut Reader) -> Option<T>,
) -> Option<Vec<T>> {
    let n = r.u32()?;
    (0..n).map(|_| item(r)).collect()
}

fn put_info(out: &mut Vec<u8>, info: &Info) {
    match &info.name {
        None => put_u8(out, 0),
        Some(name) => {
            put_u8(out, 1);
            put_str(out, name);
        }
    }
    put_u32(out, info.flags.bits());
    put_u64(out, info.ctime);
    put_u64(out, info.mtime);
    put_u64(out, info.atime);
    put_u32(out, count(info.apis.len()));
    for api in &info.apis {
        put_u64(out, api.0);
    }
}

fn get_info(r: &mut Reader) -> Option<Info> {
    let name: Option<String> = match r.u8()? {
        0 => None,
        1 => Some(r.str()?),
        _ => return None,
    };
    Some(Info {
        name,
        flags: crate::types::Flags::from_bits(r.u32()?)?,
        ctime: r.u64()?,
        mtime: r.u64()?,
        atime: r.u64()?,
        apis: get_items(r, |r| r.u64().map(Hash))?,
    })
}

fn put_event(out: &mut Vec<u8>, event: &Event) {
    put_u64(out, event.seq);
    put_u64(out, event.rd.0);
    put_u8(out, kind_tag(event.kind));
    put_option(out, event.child.map(|rd| rd.0));
}

fn get_event(r: &mut Reader) -> Option<Event> {
    Some(Event {
        seq: r.u64()?,
        rd: Rd(r.u64()?),
        kind: kind_from(r.u8()?)?,
        child: get_rd(r).ok()?,
    })
}

const ERRORS: [Error; 9] = [
    Error::Denied,
    Error::Unsupported,
    Error::Invalid,
    Error::Stale,
    Error::Busy,
    Error::NoSpace,
    Error::Closed,
    Error::Gone,
    Error::Cancelled,
];

const KINDS: [Kind; 7] = [
    Kind::Modify,
    Kind::Attrib,
    Kind::Create,
    Kind::Delete,
    Kind::Move,
    Kind::Gone,
    Kind::Overflow,
];

fn error_tag(error: Error) -> u8 {
    tag_of(&ERRORS, &error)
}

fn error_from(tag: u8) -> Option<Error> {
    ERRORS.get(usize::from(tag)).copied()
}

fn kind_tag(kind: Kind) -> u8 {
    tag_of(&KINDS, &kind)
}

fn kind_from(tag: u8) -> Option<Kind> {
    KINDS.get(usize::from(tag)).copied()
}

fn tag_of<T: PartialEq>(table: &[T], value: &T) -> u8 {
    let at = table.iter().position(|t| t == value).unwrap_or(0);
    u8::try_from(at).unwrap_or(0)
}
