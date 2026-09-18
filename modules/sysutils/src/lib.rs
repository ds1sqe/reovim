#![no_std]
#![forbid(unsafe_code)]
//! sysutils: the Module that provides the Drivers under `/bin` that
//! face the Kernel itself.
//!
//! coreutils has the tree and its bytes; these three have what only the
//! Kernel holds: `log` and `dmesg` read `/sys/log`, and `call` runs a
//! method of whatever api an rd offers. The shape is coreutils' own:
//! one Module and one Driver serving three declared apis, a name under
//! the root's `bin` per api, and a run answering with the rd of one
//! output cell that its reader drains and closes.
//!
//! Each api is a file of `bins`. Beside them the two things more than
//! one of them reads: `entries`, the log from a sequence, and `names`,
//! what the numbers of the Kernel's own lines stand for.

extern crate alloc;

use alloc::rc::Rc;
use alloc::vec::Vec;

use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Hash, Module, Rd,
    Reply, Rights,
};
use reovim_sdk::cell;

pub mod bins;
mod entries;
mod names;

use bins::{call, dmesg, log};

/// The one Driver, which is `Booted::drivers[0]`: every name this
/// Module registers is served by it, and so is every output cell.
const DRIVER: DriverId = DriverId(0);

/// The rd each name is served at: a Container, so a run's output cell
/// goes under it, and READ, so a caller may hold the right `run` asks
/// for.
const NODE: Flags = Flags::READ.union(Flags::CONTAINER);

/// What is registered under the root's `bin`, in the order of `decls`
/// and `hashes`.
const NAMES: [&str; 3] = ["log", "dmesg", "call"];

/// The Module and its Driver in one; pass it twice at `load`.
pub struct Sysutils {
    /// One per name of [`NAMES`], reported at that name's rd.
    decls: [Decl; 3],
    hashes: [Hash; 3],
    cells: cell::Cells,
}

/// The Module built and ready for `load`, as a Module's `boot` is.
#[must_use]
pub fn boot(_root: &str) -> Booted {
    let sysutils = Rc::new(Sysutils::new());
    Booted { module: sysutils.clone(), drivers: alloc::vec![sysutils] }
}

impl Default for Sysutils {
    fn default() -> Self {
        Self::new()
    }
}

impl Sysutils {
    /// The Module, before it is loaded.
    #[must_use]
    pub fn new() -> Self {
        let decls = [log::decl(), dmesg::decl(), call::decl()];
        let hashes = decls.each_ref().map(Decl::hash);
        Self { decls, hashes, cells: cell::Cells::default() }
    }

    /// The output cell a run answers with: `bytes` under `rd`, the rd
    /// the run was called on, which is the name's own.
    fn cell(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        bytes: Vec<u8>,
    ) -> Result<Rd, Error> {
        self.cells.open(ctx, DRIVER, rd, bytes)
    }
}

impl Module for Sysutils {
    fn start(&self, ctx: &dyn Calls, _home: Rd) -> Result<(), Error> {
        let bin = cell::bin(ctx)?;
        for (name, decl) in NAMES.iter().zip(&self.decls) {
            let rd = ctx.register(DRIVER, bin, Some(name))?;
            ctx.report(rd, NODE, core::slice::from_ref(decl))?;
        }
        Ok(())
    }
}

impl Driver for Sysutils {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        _: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        self.cells.read(Rd(h), at, len).into()
    }

    /// The api the Kernel checked the rd offers says which run this is;
    /// the rd called on is the one the output cell goes under.
    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        api: Hash,
        number: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        let [log, dmesg, call] = self.hashes;
        let rd = Rd(h);
        let ran = match api {
            a if a == log => log::dispatch(self, ctx, rd, number, input),
            a if a == dmesg => dmesg::dispatch(self, ctx, rd, number, input),
            a if a == call => call::dispatch(self, ctx, rd, number, input),
            _ => Err(Error::Unsupported),
        };
        ran.into()
    }

    fn close(&self, ctx: &dyn Calls, h: u64) -> Reply<()> {
        self.cells.close(ctx, Rd(h)).into()
    }
}
