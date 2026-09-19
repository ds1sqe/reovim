//! `unload`: refused while a handle is open, then the Module's rds are
//! gone, its Drivers dropped and the name free again.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::Rig;
use reovim_kernel_interface::{
    Calls, Decl, Driver, DriverId, Error, Flags, Handle, Kind, Method, Module,
    Op, Out, Rd, Reply, Rights, Token, Ty,
};
use reovim_kernel_runtime::{Limits, ROOT};

/// One rd under home and one under `/var`; `start` keeps a handle on
/// the first until method `release` closes it.
#[derive(Default)]
struct Test {
    held: Cell<Option<Handle>>,
    stopped: Cell<bool>,
    /// `read` answers `Pending` and never finishes.
    later: Cell<bool>,
}

fn decl() -> Decl {
    let release = Method {
        number: 0,
        name: "release".to_string(),
        input: Ty::Unit,
        output: Ty::Unit,
        right: Rights::WRITE,
    };
    Decl { api: "t".to_string(), ver: 1, methods: vec![release] }
}

impl Module for Test {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let cell = ctx.register(DriverId(0), home, Some("cell"))?;
        ctx.report(cell, Flags::READ | Flags::WRITE, &[decl()])?;
        let var = ctx.register(DriverId(0), Rd(6), Some("t"))?;
        ctx.report(var, Flags::READ, &[])?;
        self.held.set(Some(ctx.open(cell, Rights::READ)?));
        Ok(())
    }

    fn stop(&self) {
        self.stopped.set(true);
    }
}

impl Driver for Test {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(&self, _: &dyn Calls, _: u64, _: u64, _: usize) -> Reply<Vec<u8>> {
        if self.later.get() {
            return Reply::Pending(Token(1));
        }
        Reply::ok(Vec::new())
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        _: u64,
        _: reovim_kernel_interface::Hash,
        _: u16,
        _: &[u8],
    ) -> Reply<Vec<u8>> {
        if let Some(h) = self.held.take() {
            ctx.submit(Op::Close { h });
        }
        Reply::ok(Vec::new())
    }
}

/// The Rig's own ops, refusal and all: an rd whose Module has gone
/// answers `Invalid` where the cases above refuse to be panicked on.
impl Rig {
    fn step(&self, rd: Rd, name: &str) -> Result<Rd, Error> {
        match self.submit(Op::Lookup { rd, name: name.to_string() }) {
            Reply::Ready(Ok(Out::Rd(rd))) => Ok(rd),
            Reply::Ready(Err(e)) => Err(e),
            other => panic!("{other:?}"),
        }
    }

    fn resolve(&self, path: &str) -> Result<Rd, Error> {
        path.split('/')
            .filter(|n| !n.is_empty())
            .try_fold(ROOT, |rd, n| self.step(rd, n))
    }

    fn opened(&self, rd: Rd, rights: Rights) -> Result<Handle, Error> {
        match self.submit(Op::Open { rd, rights }) {
            Reply::Ready(Ok(Out::Handle(h))) => Ok(h),
            Reply::Ready(Err(e)) => Err(e),
            other => panic!("{other:?}"),
        }
    }
}

/// The Test Module on a Kernel that stands still, and the Module.
fn fresh() -> (Rig, Rc<Test>) {
    let test = Rc::new(Test::default());
    let driver: Rc<dyn Driver> = test.clone();
    let limits = Limits::default();
    (Rig::over(|| 1, limits, "t", test.clone(), vec![driver]), test)
}

#[test]
fn unload_waits_for_handles_then_removes_everything() {
    let (rig, test) = fresh();
    let home = rig.resolve("/module/t").unwrap();
    let cell = rig.resolve("/module/t/cell").unwrap();
    let var = rig.resolve("/var/t").unwrap();
    let modules = rig.resolve("/module").unwrap();
    let watch = rig.opened(modules, Rights::READ).unwrap();
    rig.submit(Op::Watch { h: watch, on: true });
    let cursor = match rig.submit(Op::Since { cursor: 0 }) {
        Reply::Ready(Ok(Out::Events { cursor, .. })) => cursor,
        other => panic!("{other:?}"),
    };

    assert_eq!(
        rig.kernel.unload("t"),
        Err(Error::Busy),
        "the Module holds one"
    );
    let h = rig.opened(cell, Rights::WRITE).unwrap();
    let api = decl().hash();
    rig.submit(Op::Call { h, api, method: 0, input: Vec::new() });
    assert_eq!(rig.kernel.unload("t"), Err(Error::Busy), "a caller holds one");
    rig.submit(Op::Close { h });
    assert!(!test.stopped.get());

    assert_eq!(rig.kernel.unload("t"), Ok(()));
    assert!(test.stopped.get());
    assert_eq!(rig.kernel.unload("t"), Err(Error::Invalid));
    assert_eq!(rig.resolve("/module/t"), Err(Error::Invalid));
    assert_eq!(rig.resolve("/var/t"), Err(Error::Invalid));
    assert_eq!(rig.opened(cell, Rights::READ), Err(Error::Invalid));
    let events = rig.kinds(cursor);
    assert!(
        events.contains(&(modules, Kind::Delete, Some(home))),
        "{events:?}"
    );
    assert!(!events.iter().any(|e| e.0 == cell || e.0 == var), "not watched");
    let log = rig.resolve("/sys/log").unwrap();
    let h = rig.opened(log, Rights::READ).unwrap();
    let Reply::Ready(Ok(Out::Bytes(bytes))) =
        rig.submit(Op::Read { h, at: 1, len: 4096 })
    else {
        panic!("log")
    };
    let said = String::from_utf8_lossy(&bytes);
    assert!(said.contains("unload module=t"), "{said}");

    let again = Rc::new(Test::default());
    let driver: Rc<dyn Driver> = again.clone();
    assert!(rig.kernel.load("t", again, vec![driver]).is_ok());
    assert!(rig.resolve("/module/t/cell").is_ok());
}

#[test]
fn a_module_that_leaves_takes_its_waits_with_it() {
    let (rig, test) = fresh();
    let cell = rig.resolve("/module/t/cell").unwrap();
    let h = rig.opened(cell, Rights::WRITE).unwrap();
    let api = decl().hash();
    rig.submit(Op::Call { h, api, method: 0, input: Vec::new() });
    rig.submit(Op::Close { h });

    test.later.set(true);
    let h = rig.opened(cell, Rights::READ).unwrap();
    let Reply::Pending(token) = rig.submit(Op::Read { h, at: 0, len: 4 })
    else {
        panic!("expected Pending")
    };
    // The handle goes, so nothing the Module serves is open; the wait
    // it left behind is the Module's, and the Driver that would finish
    // it is about to be dropped.
    rig.submit(Op::Close { h });
    assert_eq!(rig.kernel.unload("t"), Ok(()));

    assert_eq!(
        rig.submit(Op::Ask { token }),
        Reply::err(Error::Cancelled),
        "not Busy for ever"
    );
    let core = rig.resolve("/sys/core").unwrap();
    let h = rig.opened(core, Rights::READ).unwrap();
    let Reply::Ready(Ok(Out::Bytes(bytes))) =
        rig.submit(Op::Read { h, at: 0, len: 65535 })
    else {
        panic!("core")
    };
    let said = String::from_utf8(bytes).unwrap();
    assert!(!said.contains("state=waiting"), "{said}");
}
