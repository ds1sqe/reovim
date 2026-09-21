//! What a caller asks the Kernel and what it gets back. One enum each,
//! so the runtime has one entry point and the wire maps frames one to
//! one.

use alloc::string::String;
use alloc::vec::Vec;

use crate::types::{
    Event, Handle, Hash, Info, Kind, Outcome, Rd, Rights, Token,
};

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
    /// Child rds; needs no handle. Asks the Driver to list once, and
    /// answers from the Kernel's own table unless that listing went
    /// `Pending`, in which case the Driver's `done` is the listing.
    Children { rd: Rd },
    /// An op this caller was asked to serve is finished: `token` is the
    /// one the `serve` list named and `outcome` is in the caller's own
    /// rd and handle space. It is `Calls::done` as a Client makes it,
    /// and it never reaches the Kernel's dispatch.
    Done { token: Token, outcome: Outcome },
    /// An rd this caller serves changed, for a watch it was asked to
    /// keep: `Modify` or `Attrib`, which is what `Calls::changed` takes
    /// at either end. Or it went: `Gone` for an rd the caller
    /// unregistered, watched or not, which is `Calls::unregister` on
    /// the mirror's row for it, so what is under it goes with it and
    /// its parent lists what the caller lists. A name that came is not
    /// announced; the mirror learns it when somebody walks again. Like
    /// `Done` it never reaches the Kernel's dispatch.
    Changed { rd: Rd, kind: Kind },
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
    /// `Since`: events in order, the cursor for the next `Since`, and
    /// the ops the Kernel asks this caller to serve, each with the
    /// token its `Op::Done` names again. The list is empty for a
    /// caller nothing is asked of.
    Events { events: Vec<Event>, cursor: u64, serve: Vec<(Token, Op)> },
}
