//! What a caller asks the Kernel and what it gets back. One enum each,
//! so the runtime has one entry point and the wire maps frames one to
//! one.

use alloc::string::String;
use alloc::vec::Vec;

use crate::types::{Event, Handle, Hash, Info, Rd, Rights, Token};

/// One request, carried with the caller's sequence number.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Op {
    /// Resolve a child by name; the Container's Driver registers it.
    Lookup { rd: Rd, name: String },
    /// Open a handle; `rights` is capped by the rd's flags.
    Open { rd: Rd, rights: Rights },
    /// Up to `len` bytes at `at`; a stream Driver ignores `at`.
    Read { h: Handle, at: u64, len: u32 },
    /// Bytes at `at`; a stream Driver ignores `at`.
    Write { h: Handle, at: u64, bytes: Vec<u8> },
    /// Run method `method` of declaration `api`, which the rd must offer,
    /// with `input` in the declared encoding.
    Call { h: Handle, api: Hash, method: u16, input: Vec<u8> },
    /// Start or stop receiving events for the handle's rd.
    Watch { h: Handle, on: bool },
    /// Close the handle; implies unwatch.
    Close { h: Handle },
    /// Fetch and retire a Pending outcome.
    Ask { token: Token },
    /// Cancel a Pending reply that has not finished.
    Cancel { token: Token },
    /// Events after `cursor` on the caller's watched rds.
    Since { cursor: u64 },
    /// Cached metadata; needs no handle.
    Info { rd: Rd },
    /// Parent rd; needs no handle.
    Parent { rd: Rd },
    /// Child rds; needs no handle. Asks the Driver to list once.
    Children { rd: Rd },
}

/// One successful answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Out {
    /// `Watch`, `Close`, `Cancel`.
    Unit,
    /// `Lookup`, `Parent`.
    Rd(Rd),
    /// `Open`.
    Handle(Handle),
    /// `Write`: bytes accepted.
    Len(u32),
    /// `Read`, `Call`. Zero bytes from `Read` is end of content.
    Bytes(Vec<u8>),
    /// `Info`.
    Info(Info),
    /// `Children`.
    Rds(Vec<Rd>),
    /// `Since`: events in order and the cursor for the next `Since`.
    Events { events: Vec<Event>, cursor: u64 },
}
