#![no_std]
#![forbid(unsafe_code)]
//! coreutils: the Module that provides the Drivers under `/bin` the
//! server links.
//!
//! One Module and one Driver serving four declared apis: `stat`, `ls`,
//! `tree` and `cat`. `start` registers a name under the root's `bin`
//! per api, and a call names the api it belongs to, so the rd called on
//! and the hash together say what to run. Method 0 of each answers with
//! the rd of one output cell; whoever opened that cell reads it at an
//! offset to the empty read that ends it and closes it, which takes it
//! out of the tree. A run that fails registers nothing.
//!
//! Each api is a file of `bins`, holding its declaration and its `run`.
//! The text more than one of them prints is `format`; what they ask the
//! Kernel is the sdk's `ops`.

extern crate alloc;

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;

use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Hash, Module, Rd,
    Reply, Rights,
};
use reovim_sdk::cell::{self, Follow};

pub mod bins;
mod format;

use bins::{cat, ls, stat, tree};

/// The one Driver, which is `Booted::drivers[0]`: every name this
/// Module registers is served by it, and so is every output cell.
const DRIVER: DriverId = DriverId(0);

/// The rd each name is served at: a Container, so a run's output cell
/// goes under it, and READ, so a caller may hold the right `run` asks
/// for.
const NODE: Flags = Flags::READ.union(Flags::CONTAINER);

/// What is registered under the root's `bin`, in the order of `decls`
/// and `hashes`.
const NAMES: [&str; 4] = ["stat", "ls", "tree", "cat"];

/// The Module and its Driver in one; pass it twice at `load`.
pub struct Coreutils {
    /// One per name of [`NAMES`], reported at that name's rd.
    decls: [Decl; 4],
    hashes: [Hash; 4],
    cells: cell::Cells,
}

/// The Module built and ready for `load`, as a Module's `boot` is.
#[must_use]
pub fn boot(_root: &str) -> Booted {
    let coreutils = Rc::new(Coreutils::new());
    Booted { module: coreutils.clone(), drivers: alloc::vec![coreutils] }
}

impl Default for Coreutils {
    fn default() -> Self {
        Self::new()
    }
}

impl Coreutils {
    /// The Module, before it is loaded.
    #[must_use]
    pub fn new() -> Self {
        let decls = [stat::decl(), ls::decl(), tree::decl(), cat::decl()];
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

    /// The output cell of a run that goes on past what it found now:
    /// `follow` is what a read at the cell's end asks.
    fn following(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        bytes: Vec<u8>,
        follow: Box<dyn Follow>,
    ) -> Result<Rd, Error> {
        self.cells.follow(ctx, DRIVER, rd, bytes, follow)
    }
}

impl Module for Coreutils {
    fn start(&self, ctx: &dyn Calls, _home: Rd) -> Result<(), Error> {
        let bin = cell::bin(ctx)?;
        for (name, decl) in NAMES.iter().zip(&self.decls) {
            let rd = ctx.register(DRIVER, bin, Some(name))?;
            ctx.report(rd, NODE, core::slice::from_ref(decl))?;
        }
        Ok(())
    }
}

impl Driver for Coreutils {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        self.cells.read(ctx, Rd(h), at, len).into()
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
        let [stat, ls, tree, cat] = self.hashes;
        let rd = Rd(h);
        let ran = match api {
            a if a == stat => stat::dispatch(self, ctx, rd, number, input),
            a if a == ls => ls::dispatch(self, ctx, rd, number, input),
            a if a == tree => tree::dispatch(self, ctx, rd, number, input),
            a if a == cat => cat::dispatch(self, ctx, rd, number, input),
            _ => Err(Error::Unsupported),
        };
        ran.into()
    }

    fn close(&self, ctx: &dyn Calls, h: u64) -> Reply<()> {
        self.cells.close(ctx, Rd(h)).into()
    }
}
