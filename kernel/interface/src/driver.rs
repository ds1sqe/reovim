//! The two sides of the contract: what a Driver answers and what a
//! Module may ask the Kernel. Seven slots, seven calls, never more.
//!
//! With them the checks a tree makes before a slot and the two cuts a
//! cell's bytes take, which the Kernel and a Client's Host spell one
//! way and not two.

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::decl::Decl;
use crate::op::{Op, Out};
use crate::types::{
    DriverId, Error, Flags, Handle, Hash, Kind, Outcome, Rd, Reply, Rights,
    Token,
};
use crate::value;

/// The fixed ops one Driver serves. Every slot defaults to `Unsupported`,
/// so a Driver leaves a slot empty by not writing it.
///
/// `ctx` is the Kernel as seen by the Driver's Module. The `u64` handle
/// in `open` and after is the Driver's own value; the Kernel maps its
/// `Handle` to it. A slot may block.
///
/// Any slot may answer `Pending`: the token is the Driver's own value,
/// unique among its outstanding ones, and `done` names it again with
/// the `Out` its op answers — `Rd` for `lookup` a name, `Rds` for
/// `lookup` the whole listing, `Handle` for `open`, `Bytes` for `read`
/// and `call`, `Len` for `write`. `watch` and `close` need no token:
/// the Kernel's answer to either does not depend on the slot's, so a
/// `Pending` from them is discarded with the rest of the reply.
#[allow(unused_variables)]
pub trait Driver {
    /// Register the child `name` under `rd` and return it. An empty
    /// name means "register all children now".
    fn lookup(&self, ctx: &dyn Calls, rd: Rd, name: &str) -> Reply<Rd> {
        Reply::err(Error::Unsupported)
    }

    /// Open `rd` with `rights`, already capped by the Kernel.
    ///
    /// A `Pending` here is finished by a `done` carrying
    /// `Ok(Out::Handle(Handle(cookie)))`: this Driver's own value in
    /// the one `Out` that holds one, since the Kernel's handle does
    /// not exist for the Driver to name. The Kernel writes the cookie
    /// into the slot the wait made and answers the caller with that
    /// handle instead.
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
    ///
    /// Every watch that ends is told here, a close included: closing a
    /// watching handle implies unwatch, as [`Op::Close`]'s own doc
    /// says, and the Driver is told of that one as it is told of every
    /// other. So a Driver that counts what it is told reaches zero
    /// whichever way the last watcher goes. The count is of watches on
    /// `rd` and not of handles: two on one rd are two hints.
    fn watch(&self, ctx: &dyn Calls, rd: Rd, on: bool) -> Reply<()> {
        Reply::err(Error::Unsupported)
    }

    /// The handle is done.
    fn close(&self, ctx: &dyn Calls, h: u64) -> Reply<()> {
        Reply::err(Error::Unsupported)
    }
}

/// The rd's flags cap the asked rights; asking nothing is `Denied` too.
#[must_use]
pub fn allows(flags: Flags, rights: Rights) -> bool {
    !rights.is_empty()
        && Flags::from_bits(rights.bits())
            .is_some_and(|asked| flags.contains(asked))
}

/// What a tree checks before [`Driver::call`], which is written
/// trusting it: the rd offers the api, the declaration has the method,
/// the handle holds its right, the input fits.
///
/// # Errors
/// `Unsupported` for an api the rd does not offer or a method the
/// declaration has not, `Denied` for a right the handle lacks,
/// `Invalid` for an input that is not one value of the method's type.
pub fn check_call(
    rights: Rights,
    offered: &[Hash],
    decls: &BTreeMap<Hash, Decl>,
    api: Hash,
    method: u16,
    input: &[u8],
) -> Result<(), Error> {
    if !offered.contains(&api) {
        return Err(Error::Unsupported);
    }
    let decl = decls.get(&api).ok_or(Error::Unsupported)?;
    let method = decl.method(method).ok_or(Error::Unsupported)?;
    if !rights.contains(method.right) {
        return Err(Error::Denied);
    }
    if !value::fits(input, &method.input) {
        return Err(Error::Invalid);
    }
    Ok(())
}

/// Up to `len` bytes of `bytes` from `at`, or as many as there are:
/// what a cell answers a read at an offset with.
#[must_use]
pub fn part(bytes: &[u8], at: u64, len: usize) -> Vec<u8> {
    let start = usize::try_from(at).unwrap_or(usize::MAX).min(bytes.len());
    let end = start.saturating_add(len).min(bytes.len());
    bytes[start..end].to_vec()
}

/// `draft` with its `len` bytes from `at` replaced by `text`: what a
/// cell takes a write at an offset as.
///
/// # Errors
/// `Invalid` when that range is not inside `draft`.
pub fn spliced(
    draft: &[u8],
    at: u64,
    len: u64,
    text: &[u8],
) -> Result<Vec<u8>, Error> {
    let at = usize::try_from(at).map_err(|_| Error::Invalid)?;
    let len = usize::try_from(len).map_err(|_| Error::Invalid)?;
    let end = at.checked_add(len).ok_or(Error::Invalid)?;
    if end > draft.len() {
        return Err(Error::Invalid);
    }
    let mut out = draft[..at].to_vec();
    out.extend_from_slice(text);
    out.extend_from_slice(&draft[end..]);
    Ok(out)
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

/// A Module built and ready for `load`: the Module and its Drivers in
/// `DriverId` order. What a module crate's `boot` returns, so the binary
/// that links it names nothing else.
pub struct Booted {
    /// The Module, started by `load`.
    pub module: Rc<dyn Module>,
    /// Its Drivers; `drivers[i]` is `DriverId(i)`.
    pub drivers: Vec<Rc<dyn Driver>>,
}
