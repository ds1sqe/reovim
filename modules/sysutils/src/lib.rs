#![no_std]
#![forbid(unsafe_code)]
//! sysutils: the Module that provides the Drivers under `/bin` that
//! face the Kernel itself.
//!
//! coreutils has the tree and its bytes; these six have what only the
//! Kernel holds: `log` and `dmesg` read `/sys/log`, `call` runs a
//! method of whatever api an rd offers, `watch` says what happens to an
//! rd, and `date` and `uptime` spell two of the facts under
//! `/sys/info`. The shape is coreutils' own: one Module and one Driver
//! serving six declared apis, a name under the root's `bin` per api,
//! and a run answering with the rd of one output cell that its reader
//! drains and closes.
//!
//! Each api is a file of `bins`. Beside them the two things more than
//! one of them reads: `entries`, the log from a sequence, and `names`,
//! what the numbers of the Kernel's own lines stand for; and here the
//! escape `dmesg` and `watch` both write and the fact the two readers
//! both read.

extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Hash, Module, Rd,
    Reply, Rights,
};
use reovim_sdk::cell::{self, Follow};
use reovim_sdk::ops;
use reovim_sdk::walk::ROOT;

pub mod bins;
mod entries;
mod names;

use bins::{call, date, dmesg, log, uptime, watch};

/// The one Driver, which is `Booted::drivers[0]`: every name this
/// Module registers is served by it, and so is every output cell.
const DRIVER: DriverId = DriverId(0);

/// The rd each name is served at: a Container, so a run's output cell
/// goes under it, and READ, so a caller may hold the right `run` asks
/// for.
const NODE: Flags = Flags::READ.union(Flags::CONTAINER);

/// What is registered under the root's `bin`, in the order of `decls`
/// and `hashes`.
const NAMES: [&str; 6] = ["log", "dmesg", "call", "watch", "date", "uptime"];

/// The Module and its Driver in one; pass it twice at `load`.
pub struct Sysutils {
    /// One per name of [`NAMES`], reported at that name's rd.
    decls: [Decl; 6],
    hashes: [Hash; 6],
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
        let decls = [
            log::decl(),
            dmesg::decl(),
            call::decl(),
            watch::decl(),
            date::decl(),
            uptime::decl(),
        ];
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
        let [log, dmesg, call, watch, date, uptime] = self.hashes;
        let rd = Rd(h);
        let ran = match api {
            a if a == log => log::dispatch(self, ctx, rd, number, input),
            a if a == dmesg => dmesg::dispatch(self, ctx, rd, number, input),
            a if a == call => call::dispatch(self, ctx, rd, number, input),
            a if a == watch => watch::dispatch(self, ctx, rd, number, input),
            a if a == date => date::dispatch(self, ctx, rd, number, input),
            a if a == uptime => uptime::dispatch(self, ctx, rd, number, input),
            _ => Err(Error::Unsupported),
        };
        ran.into()
    }

    fn close(&self, ctx: &dyn Calls, h: u64) -> Reply<()> {
        self.cells.close(ctx, Rd(h)).into()
    }
}

/// The number the fact at `path` holds, which is the whole of what a
/// reader under `/bin` is a wording of. The path is absolute and the
/// walk starts at the root, so the same reader loaded at a Client reads
/// the server's own; a cell that is no number is `Invalid`, which no
/// reader guesses a value for.
fn fact(ctx: &dyn Calls, path: &str) -> Result<u64, Error> {
    let rd = cell::resolve(ctx, ROOT, path)?;
    let bytes = ops::bytes(ctx, rd)?;
    let text = core::str::from_utf8(&bytes).map_err(|_| Error::Invalid)?;
    text.parse().map_err(|_| Error::Invalid)
}

/// `text` in one SGR sequence when the run asked for color, reset after
/// it, so a line never leaks a color into the next.
fn sgr(color: bool, code: &str, text: &str) -> String {
    if color { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
}
