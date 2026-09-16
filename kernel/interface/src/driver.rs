//! The two sides of the contract: what a Driver answers and what a
//! Module may ask the Kernel. Seven slots, seven calls, never more.

use alloc::vec::Vec;

use crate::decl::Decl;
use crate::op::{Op, Out};
use crate::types::{
    DriverId, Error, Flags, Handle, Hash, Kind, Outcome, Rd, Reply, Rights,
    Token,
};

/// The fixed ops one Driver serves. Every slot defaults to `Unsupported`,
/// so a Driver leaves a slot empty by not writing it.
///
/// `ctx` is the Kernel as seen by the Driver's Module. The `u64` handle
/// in `open` and after is the Driver's own value; the Kernel maps its
/// `Handle` to it. A slot may block. Only `read`, `write` and `call` may
/// answer `Pending`; the token is the Driver's own value, unique among
/// its outstanding ones, and `done` names it again.
#[allow(unused_variables)]
pub trait Driver {
    /// Register the child `name` under `rd` and return it. An empty
    /// name means "register all children now".
    fn lookup(&self, ctx: &dyn Calls, rd: Rd, name: &str) -> Reply<Rd> {
        Reply::err(Error::Unsupported)
    }

    /// Open `rd` with `rights`, already capped by the Kernel.
    fn open(&self, ctx: &dyn Calls, rd: Rd, rights: Rights) -> Reply<u64> {
        Reply::err(Error::Unsupported)
    }

    /// Up to `len` bytes at `at`; an empty answer is end of content.
    fn read(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        Reply::err(Error::Unsupported)
    }

    /// Accept bytes at `at`; answer how many.
    fn write(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        buf: &[u8],
    ) -> Reply<usize> {
        Reply::err(Error::Unsupported)
    }

    /// Run a declared method; the Kernel has checked hash, method, right
    /// and input fit.
    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        api: Hash,
        method: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        Reply::err(Error::Unsupported)
    }

    /// Hint: someone started or stopped watching `rd`.
    fn watch(&self, ctx: &dyn Calls, rd: Rd, on: bool) -> Reply<()> {
        Reply::err(Error::Unsupported)
    }

    /// The handle is done.
    fn close(&self, ctx: &dyn Calls, h: u64) -> Reply<()> {
        Reply::err(Error::Unsupported)
    }
}

/// What a Module may ask the Kernel. The Kernel builds one per Module,
/// so a Driver cannot speak for another Module.
pub trait Calls {
    /// Create a Resource served by `driver` under `parent`.
    ///
    /// # Errors
    /// `Denied` unless the parent is served by this Module or is a Kernel
    /// Container flagged WRITE for it; `Invalid` for a bad or taken name.
    fn register(
        &self,
        driver: DriverId,
        parent: Rd,
        name: Option<&str>,
    ) -> Result<Rd, Error>;

    /// Set or clear the name; the parent's watchers get `Move`.
    ///
    /// # Errors
    /// `Denied` unless this Module serves `rd`; `Invalid` for a bad name.
    fn name(&self, rd: Rd, name: Option<&str>) -> Result<(), Error>;

    /// Change flags and offered apis; watchers get `Attrib`.
    ///
    /// # Errors
    /// `Denied` unless this Module serves `rd`; `Invalid` for a
    /// declaration out of bounds or a hash collision.
    fn report(&self, rd: Rd, flags: Flags, apis: &[Decl]) -> Result<(), Error>;

    /// Remove `rd` and its subtree; each gets `Gone`, the parent
    /// `Delete`.
    ///
    /// # Errors
    /// `Denied` unless this Module serves `rd`.
    fn unregister(&self, rd: Rd) -> Result<(), Error>;

    /// After an accepted change: `Modify` or `Attrib`.
    ///
    /// # Errors
    /// `Denied` unless this Module serves `rd`; `Invalid` for any other
    /// kind.
    fn changed(&self, rd: Rd, kind: Kind) -> Result<(), Error>;

    /// Finish a Pending reply this Module's Driver gave.
    ///
    /// # Errors
    /// `Invalid` when no such token is outstanding.
    fn done(&self, token: Token, outcome: Outcome) -> Result<(), Error>;

    /// Open any Resource, own or not, as this Module.
    ///
    /// # Errors
    /// As `Op::Open`.
    fn open(&self, rd: Rd, rights: Rights) -> Result<Handle, Error>;

    /// Any caller op on handles from `open`, as this Module.
    fn submit(&self, op: Op) -> Reply<Out>;
}

/// A unit of loading: one `start`, one `stop`, zero or more Drivers.
pub trait Module {
    /// Called once after load. `home` is `/module/<name>`.
    ///
    /// # Errors
    /// Any; the Kernel reports it from `load`.
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error>;

    /// Called before unload. Nothing is served after it.
    fn stop(&self) {}
}
