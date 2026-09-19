//! What a run under `/bin` holds open when an op of its own fails.
//!
//! `dmesg` and `watch` both open something before they have a cell to
//! hand it to — a handle on the log, a watch on the target — and both
//! then ask one more thing of the Kernel. The tree here answers every
//! op a run makes and refuses that last one, which a Kernel of its own
//! never does; what the run leaves open is read off the closes.

use std::cell::{Cell, RefCell};

use reovim_kernel_interface::{
    Calls, Decl, DriverId, Error, Flags, Handle, Info, Kind, Op, Out, Outcome,
    Rd, Reply, Rights, Token,
};
use reovim_module_sysutils::Sysutils;
use reovim_module_sysutils::bins::dmesg::dmesg::Api as Dmesg;
use reovim_module_sysutils::bins::watch::watch::Api as Watch;
use reovim_module_sysutils::bins::{dmesg, watch};
use reovim_sdk::walk::ROOT;

/// A tree that answers the ops a run makes of it: every `Lookup` is
/// the rd after the one asked, an `Open` is a handle counted here, and
/// the two reads a run follows its target with answer `Denied`.
#[derive(Default)]
struct Tree {
    open: Cell<u64>,
    closed: RefCell<Vec<Handle>>,
}

impl Tree {
    /// The handles the run handed back.
    fn closed(&self) -> Vec<Handle> {
        self.closed.borrow().clone()
    }
}

impl Calls for Tree {
    fn register(
        &self,
        _: DriverId,
        parent: Rd,
        _: Option<&str>,
    ) -> Result<Rd, Error> {
        Ok(Rd(parent.0 + 1))
    }

    fn name(&self, _: Rd, _: Option<&str>) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    fn report(&self, _: Rd, _: Flags, _: &[Decl]) -> Result<(), Error> {
        Ok(())
    }

    fn unregister(&self, _: Rd) -> Result<(), Error> {
        Ok(())
    }

    fn changed(&self, _: Rd, _: Kind) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    fn done(&self, _: Token, _: Outcome) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    fn open(&self, _: Rd, _: Rights) -> Result<Handle, Error> {
        self.open.set(self.open.get() + 1);
        Ok(Handle(self.open.get()))
    }

    fn submit(&self, op: Op) -> Reply<Out> {
        match op {
            Op::Lookup { rd, .. } => Reply::ok(Out::Rd(Rd(rd.0 + 1))),
            Op::Info { rd } => Reply::ok(Out::Info(named(rd))),
            Op::Watch { .. } => Reply::ok(Out::Unit),
            Op::Close { h } => {
                self.closed.borrow_mut().push(h);
                Reply::ok(Out::Unit)
            }
            Op::Read { .. } | Op::Since { .. } => Reply::err(Error::Denied),
            _ => Reply::err(Error::Unsupported),
        }
    }
}

/// What this tree caches about `rd`, which is a name and nothing more.
fn named(rd: Rd) -> Info {
    Info {
        name: Some(format!("rd{}", rd.0)),
        flags: Flags::READ,
        ctime: 0,
        mtime: 0,
        atime: 0,
        apis: Vec::new(),
    }
}

/// A `dmesg` run with nothing asked for but the tail, which is the run
/// that keeps its handle on the log.
const fn tailing() -> dmesg::Args {
    dmesg::Args {
        t: false,
        T: false,
        H: false,
        d: false,
        r: false,
        x: false,
        k: false,
        w: true,
        W: false,
        f: String::new(),
        l: String::new(),
        color: false,
    }
}

#[test]
fn a_dmesg_that_cannot_read_the_log_hands_its_handle_back() {
    let tree = Tree::default();
    let run = Dmesg::run(&Sysutils::default(), &tree, ROOT, tailing());

    assert_eq!(run, Err(Error::Denied));
    assert_eq!(tree.closed(), [Handle(1)], "the handle on the log");
}

#[test]
fn a_watch_that_cannot_read_the_ring_hands_its_watch_back() {
    let tree = Tree::default();
    let args =
        watch::Args { cwd: ROOT, color: false, target: String::from("note") };
    let run = Watch::run(&Sysutils::default(), &tree, ROOT, args);

    assert_eq!(run, Err(Error::Denied));
    assert_eq!(tree.closed(), [Handle(1)], "the handle that watched");
}
