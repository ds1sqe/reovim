//! `/self/bin`'s two names, which is how this Client loads.
//!
//! `load` and `unload` are messages of the link and not ops, so no
//! `Calls` can spell them. The Client may speak to the link and to its
//! own Host, and this is the one Driver that is the Client's own: it
//! stands under no Module, `ModuleId(0)` serving it, and no `unload`
//! reaches it.
//!
//! Each name declares one `run`, with `local` the flag that says which
//! side is meant: `load PATH` is the server's userland and `load -l
//! PATH` this Client's. A run answers one output cell holding nothing,
//! as every Driver under a `bin` does, so a redirect and the reads
//! that end a run are the same code they are for `ls`.

use alloc::boxed::Box;
use alloc::rc::{Rc, Weak};
use alloc::string::String;
use alloc::vec::Vec;

use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Hash, Rd, Reply,
    Rights,
};
use reovim_sdk::{cell, ops};

use crate::host::{Host, SELF, refused};

reovim_sdk::declare! {
    api load ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { local: bool, path: utf8(512) }
}

reovim_sdk::declare! {
    api unload ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { local: bool, name: utf8(64) }
}

/// The one Driver, which is the Client's own: both names are served by
/// it, and so is every output cell.
const DRIVER: DriverId = DriverId(0);

/// The rd each name is served at, as coreutils' are: a Container, so a
/// run's output cell goes under it, and READ, so a caller may hold the
/// right `run` asks for.
const NODE: Flags = Flags::READ.union(Flags::CONTAINER);

/// What is registered under `/self/bin`, in the order of [`decls`].
const NAMES: [&str; 2] = ["load", "unload"];

/// What reads the Module in a `.rvm` file: the name it is loaded under
/// and what was built.
pub type Read = Box<dyn Fn(&str) -> Result<(String, Booted), Error>>;

/// The `.rvm` files of the machine the Client runs on, as the closure
/// the app injects.
///
/// The Client links no host crate, so a file is reached this way, as
/// the clipboard's machine is.
pub struct Rvm(Read);

impl Rvm {
    /// One over `read`.
    #[must_use]
    pub const fn new(read: Read) -> Self {
        Self(read)
    }

    /// A Client that was given no files: `Unsupported`, which is what
    /// `-l` then answers.
    #[must_use]
    pub fn none() -> Self {
        Self::new(Box::new(|_| Err(Error::Unsupported)))
    }
}

/// The Client's own Driver: the Host it speaks for and the files
/// behind `load -l`.
pub struct Bin {
    /// The Host holds this Driver, so this holds the Host weakly and
    /// the pair frees with the Client.
    host: Weak<Host>,
    rvm: Rvm,
    hashes: [Hash; 2],
    cells: cell::Cells,
}

/// `load` and `unload` under `/self/bin`, served by `host`.
///
/// `rvm` is how a `.rvm` file is read, [`Rvm::none`] for a Client that
/// was given none.
///
/// # Errors
/// `Invalid` when `/self/bin` is not there; otherwise as
/// `Calls::register` and `Calls::report`.
pub fn boot(host: &Rc<Host>, rvm: Rvm) -> Result<(), Error> {
    let bin = Rc::new(Bin::new(host, rvm));
    host.serves(bin);
    let ctx = host.own();
    let at = ops::lookup(&ctx, SELF, "bin")?;
    for (name, decl) in NAMES.iter().zip(&decls()) {
        let rd = ctx.register(DRIVER, at, Some(name))?;
        ctx.report(rd, NODE, core::slice::from_ref(decl))?;
    }
    Ok(())
}

/// One declaration per name of [`NAMES`], in its order.
fn decls() -> [Decl; 2] {
    [load::decl(), unload::decl()]
}

impl Bin {
    fn new(host: &Rc<Host>, rvm: Rvm) -> Self {
        Self {
            host: Rc::downgrade(host),
            rvm,
            hashes: decls().each_ref().map(Decl::hash),
            cells: cell::Cells::default(),
        }
    }

    /// The Host, while the Client it belongs to is still there.
    fn host(&self) -> Result<Rc<Host>, Error> {
        self.host.upgrade().ok_or(Error::Gone)
    }

    fn script(&self, ctx: &dyn Calls, rd: Rd, path: &str) -> Result<Rd, Error> {
        use reovim_client_api::exec;
        let cli = ops::lookup(ctx, SELF, "cli")
            .and_then(|cli| ops::lookup(ctx, cli, "exec"))
            .map_err(|_| Error::Unsupported)?;
        if !ops::info(ctx, cli)?.apis.contains(&exec::hash()) {
            return Err(Error::Unsupported);
        }
        ops::call(
            ctx,
            cli,
            exec::hash(),
            &exec::decl().methods[0],
            reovim_sdk::encode(&String::from(path)),
        )?;
        self.done(ctx, rd)
    }

    /// One run's output: a cell holding nothing, neither name printing
    /// anything.
    fn done(&self, ctx: &dyn Calls, rd: Rd) -> Result<Rd, Error> {
        self.cells.open(ctx, DRIVER, rd, Vec::new())
    }
}

impl load::Api for Bin {
    /// `load -l PATH` into this Client, `load PATH` into the server.
    ///
    /// A path nobody typed is `Invalid` before either side is asked: a
    /// declaration says which fields a run has and not which it needs.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: load::Args,
    ) -> Result<Rd, Error> {
        if args.path.is_empty() {
            return Err(Error::Invalid);
        }
        if args.local
            && args
                .path
                .rsplit_once('.')
                .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("fipe"))
        {
            return self.script(ctx, rd, &args.path);
        }
        let host = self.host()?;
        if args.local {
            let (name, booted) = (self.rvm.0)(&args.path)?;
            host.load(&name, booted.module, booted.drivers)?;
        } else {
            // The borrow of the wire lasts exactly this statement.
            host.wire().load(&args.path).map_err(|f| refused(&f))?;
        }
        self.done(ctx, rd)
    }
}

impl unload::Api for Bin {
    /// `unload -l NAME` out of this Client, `unload NAME` out of the
    /// server. An empty name is `Invalid`, as an empty path is, and a
    /// name the Client is nothing without is `Denied`: the Host keeps
    /// its own devices, its own row and the Plugin holding the region
    /// from every door, this one and the swap alike.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: unload::Args,
    ) -> Result<Rd, Error> {
        if args.name.is_empty() {
            return Err(Error::Invalid);
        }
        let host = self.host()?;
        if args.local {
            host.unload(&args.name)?;
        } else {
            host.wire().unload(&args.name).map_err(|f| refused(&f))?;
        }
        self.done(ctx, rd)
    }
}

impl Driver for Bin {
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

    /// The api the Host checked the rd offers says which run this is;
    /// the rd called on is the one the output cell goes under.
    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        api: Hash,
        number: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        let [load, unload] = self.hashes;
        let rd = Rd(h);
        let ran = match api {
            a if a == load => load::dispatch(self, ctx, rd, number, input),
            a if a == unload => unload::dispatch(self, ctx, rd, number, input),
            _ => Err(Error::Unsupported),
        };
        ran.into()
    }

    fn close(&self, ctx: &dyn Calls, h: u64) -> Reply<()> {
        self.cells.close(ctx, Rd(h)).into()
    }
}
