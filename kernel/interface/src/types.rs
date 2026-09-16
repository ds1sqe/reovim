//! Ids the Kernel owns, flags and rights, the closed error set and the
//! reply shape every operation answers with.

use alloc::string::String;
use alloc::vec::Vec;

use reovim_bitflags::rv_bitflags;

/// Resource descriptor: the Kernel-owned id of one Resource.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Rd(pub u64);

/// A caller's open handle on one Resource, with the rights it asked for.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Handle(pub u64);

/// Identifies one Pending reply until `done` and `ask` retire it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Token(pub u64);

/// One connection or one Module acting as a caller; owns handles, a
/// reply window and a watch set.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Caller(pub u64);

/// One loaded Module.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModuleId(pub u32);

/// One Driver inside a Module, numbered in the order given at load.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DriverId(pub u32);

/// Identity of a declaration: the hash of its canonical bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Hash(pub u64);

rv_bitflags! {
    /// Shape of a Resource as its Driver reports it.
    pub struct Flags: u32 {
        /// `read`, `watch` and `lookup` may be granted.
        const READ = 1;
        /// `write` may be granted.
        const WRITE = 2;
        /// May have children.
        const CONTAINER = 4;
    }
}

rv_bitflags! {
    /// What a handle may do; capped by the Resource's flags.
    pub struct Rights: u32 {
        /// `read`, `watch`, `lookup`, and methods declared READ.
        const READ = 1;
        /// `write` and methods declared WRITE.
        const WRITE = 2;
    }
}

/// The nine causes an operation may fail with. Closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The handle lacks the right, or the caller may not touch that rd.
    Denied,
    /// The Driver leaves the slot empty or the rd offers no such method.
    Unsupported,
    /// Malformed input, unknown id, or a bound exceeded.
    Invalid,
    /// A precondition the caller stated no longer holds.
    Stale,
    /// Try again later; a cap was reached.
    Busy,
    /// A Driver limit on storage was reached.
    NoSpace,
    /// The handle was closed.
    Closed,
    /// The Resource was unregistered.
    Gone,
    /// The Pending reply was cancelled.
    Cancelled,
}

/// How every operation answers, in the same call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reply<T> {
    /// Finished now.
    Ready(Result<T, Error>),
    /// Finishes later; the Driver calls `done(token, outcome)`.
    Pending(Token),
}

impl<T> Reply<T> {
    /// `Ready(Ok(value))`.
    #[must_use]
    pub const fn ok(value: T) -> Self {
        Self::Ready(Ok(value))
    }

    /// `Ready(Err(error))`.
    #[must_use]
    pub const fn err(error: Error) -> Self {
        Self::Ready(Err(error))
    }
}

impl<T> From<Result<T, Error>> for Reply<T> {
    fn from(result: Result<T, Error>) -> Self {
        Self::Ready(result)
    }
}

/// What `done` delivers for a Pending reply.
pub type Outcome = Result<super::Out, Error>;

/// The seven event kinds. Closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// Content changed; the Driver reported it.
    Modify,
    /// Flags, times or apis changed; the Driver reported it.
    Attrib,
    /// A child was registered under the rd.
    Create,
    /// A child was removed under the rd.
    Delete,
    /// A child's name changed under the rd.
    Move,
    /// The rd was removed; its handles are dead.
    Gone,
    /// The log was trimmed past the watcher's cursor.
    Overflow,
}

/// One watch event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Event {
    /// Position in the Kernel's event log.
    pub seq: u64,
    /// The watched rd.
    pub rd: Rd,
    /// What happened.
    pub kind: Kind,
    /// The child, for `Create`, `Delete` and `Move`.
    pub child: Option<Rd>,
}

/// The Kernel's cached view of one Resource, as `info(rd)` returns it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Info {
    /// The name under its parent, if any.
    pub name: Option<String>,
    /// Reported shape.
    pub flags: Flags,
    /// Kernel clock at the last `Attrib` or `report`.
    pub ctime: u64,
    /// Kernel clock at the last `Modify`.
    pub mtime: u64,
    /// Kernel clock at the last `read`.
    pub atime: u64,
    /// Declarations the rd offers.
    pub apis: Vec<Hash>,
}
