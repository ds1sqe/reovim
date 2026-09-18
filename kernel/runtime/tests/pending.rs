//! A slot that answers `Pending` on the ops that name no handle, and
//! on the `open` whose handle does not exist yet.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use reovim_kernel_interface::{
    Caller, Calls, Driver, DriverId, Error, Flags, Handle, Module, ModuleId,
    Op, Out, Outcome, Rd, Reply, Rights, Token,
};
use reovim_kernel_runtime::{Heap, Kernel, Limits, SYS};

/// One Container under home, and every slot of it waiting: the Driver
/// answers `Pending` with a token of its own and the case is what
/// finishes it.
#[derive(Default)]
struct Slow {
    dir: Cell<Option<Rd>>,
    next: Cell<u64>,
    /// The cookie each `read` was asked through.
    reads: RefCell<Vec<u64>>,
    closed: RefCell<Vec<u64>>,
}

impl Slow {
    /// The next own token; the Kernel scopes it by this Module.
    fn later<T>(&self) -> Reply<T> {
        self.next.set(self.next.get() + 1);
        Reply::Pending(Token(self.next.get()))
    }
}

impl Module for Slow {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let dir = ctx.register(DriverId(0), home, Some("dir"))?;
        ctx.report(dir, Flags::READ | Flags::WRITE | Flags::CONTAINER, &[])?;
        self.dir.set(Some(dir));
        Ok(())
    }
}

impl Driver for Slow {
    fn lookup(&self, _: &dyn Calls, _: Rd, _: &str) -> Reply<Rd> {
        self.later()
    }

    fn open(&self, _: &dyn Calls, _: Rd, _: Rights) -> Reply<u64> {
        self.later()
    }

    fn read(&self, _: &dyn Calls, h: u64, _: u64, _: usize) -> Reply<Vec<u8>> {
        self.reads.borrow_mut().push(h);
        self.later()
    }

    fn close(&self, _: &dyn Calls, h: u64) -> Reply<()> {
        self.closed.borrow_mut().push(h);
        Reply::ok(())
    }
}

struct Rig {
    kernel: Kernel,
    slow: Rc<Slow>,
    module: ModuleId,
    caller: Caller,
    seq: Cell<u64>,
}

impl Rig {
    fn new(limits: Limits) -> Self {
        let kernel = Kernel::new(|| 1, Heap::default, limits);
        let slow = Rc::new(Slow::default());
        let driver: Rc<dyn Driver> = slow.clone();
        let module = kernel.load("s", slow.clone(), vec![driver]).unwrap();
        let caller = kernel.caller_new();
        Self { kernel, slow, module, caller, seq: Cell::new(0) }
    }

    fn default() -> Self {
        Self::new(Limits::default())
    }

    fn submit(&self, op: Op) -> Reply<Out> {
        self.seq.set(self.seq.get() + 1);
        self.kernel.submit(self.caller, self.seq.get(), op)
    }

    fn ok(&self, op: Op) -> Out {
        match self.submit(op) {
            Reply::Ready(Ok(out)) => out,
            other => panic!("expected Ready(Ok), got {other:?}"),
        }
    }

    fn err(&self, op: Op) -> Error {
        match self.submit(op) {
            Reply::Ready(Err(error)) => error,
            other => panic!("expected Ready(Err), got {other:?}"),
        }
    }

    fn waits(&self, op: Op) -> Token {
        match self.submit(op) {
            Reply::Pending(token) => token,
            other @ Reply::Ready(_) => {
                panic!("expected Pending, got {other:?}")
            }
        }
    }

    fn calls(&self) -> impl Calls + '_ {
        self.kernel.calls(self.module)
    }

    /// The Driver's first own token finished; every case makes one.
    fn done(&self, outcome: Outcome) {
        self.calls().done(Token(1), outcome).unwrap();
    }

    fn dir(&self) -> Rd {
        self.slow.dir.get().unwrap()
    }

    /// One handle on `/sys/core`.
    fn core_handle(&self) -> Handle {
        let found = self.ok(Op::Lookup { rd: SYS, name: "core".into() });
        let Out::Rd(rd) = found else { panic!("{found:?}") };
        match self.ok(Op::Open { rd, rights: Rights::READ }) {
            Out::Handle(h) => h,
            other => panic!("{other:?}"),
        }
    }

    /// The handle the next `open` will be given, so a case can name a
    /// number the caller does not hold yet.
    fn next_handle(&self) -> Handle {
        let h = self.core_handle();
        self.ok(Op::Close { h });
        Handle(h.0 + 1)
    }

    /// `/sys/core` as one text.
    fn core(&self) -> String {
        let h = self.core_handle();
        let Out::Bytes(bytes) = self.ok(Op::Read { h, at: 0, len: 65535 })
        else {
            panic!("read core")
        };
        self.ok(Op::Close { h });
        String::from_utf8(bytes).unwrap()
    }
}

/// An `open` of the Container that is waiting: the handle it made,
/// which the caller does not hold yet, and the token it holds instead.
fn opening(rig: &Rig) -> (Handle, Token) {
    let h = rig.next_handle();
    (h, rig.waits(Op::Open { rd: rig.dir(), rights: Rights::READ }))
}

/// The child the Driver registers while the caller waits, as its
/// `lookup` would have registered it at once.
fn child(rig: &Rig, name: &str) -> Rd {
    let rd = rig.calls().register(DriverId(0), rig.dir(), Some(name)).unwrap();
    rig.calls().report(rd, Flags::READ, &[]).unwrap();
    rd
}

#[test]
fn a_lookup_that_waits_is_asked_out_as_the_child() {
    let rig = Rig::default();
    let token = rig.waits(Op::Lookup { rd: rig.dir(), name: "a".into() });
    assert_eq!(rig.err(Op::Ask { token }), Error::Busy);
    let a = child(&rig, "a");
    rig.done(Ok(Out::Rd(a)));
    assert_eq!(rig.ok(Op::Ask { token }), Out::Rd(a));
    assert_eq!(rig.err(Op::Ask { token }), Error::Invalid, "asked once");
}

#[test]
fn a_listing_that_waits_is_asked_out_as_the_children() {
    let rig = Rig::default();
    let token = rig.waits(Op::Children { rd: rig.dir() });
    assert_eq!(rig.err(Op::Ask { token }), Error::Busy);
    let (a, b) = (child(&rig, "a"), child(&rig, "b"));
    // The listing is the Driver's own answer and not the table's: the
    // table was not full yet when the op returned.
    rig.done(Ok(Out::Rds(vec![a, b])));
    assert_eq!(rig.ok(Op::Ask { token }), Out::Rds(vec![a, b]));
    assert_eq!(rig.err(Op::Ask { token }), Error::Invalid, "asked once");
}

#[test]
fn a_cancel_before_the_done_answers_cancelled() {
    let rig = Rig::default();
    let token = rig.waits(Op::Lookup { rd: rig.dir(), name: "a".into() });
    assert_eq!(rig.ok(Op::Cancel { token }), Out::Unit);
    let a = child(&rig, "a");
    rig.done(Ok(Out::Rd(a)));
    assert_eq!(rig.err(Op::Ask { token }), Error::Cancelled);
    assert_eq!(rig.err(Op::Ask { token }), Error::Invalid);
}

#[test]
fn a_core_says_dash_for_a_wait_that_names_no_handle() {
    let rig = Rig::default();
    let lookup = rig.waits(Op::Lookup { rd: rig.dir(), name: "a".into() });
    let children = rig.waits(Op::Children { rd: rig.dir() });
    let core = rig.core();
    for token in [lookup, children] {
        let line = format!(
            "pending token={} caller=- handle=- state=waiting",
            token.0
        );
        assert!(core.lines().any(|l| l == line), "{line}: {core}");
    }
}

#[test]
fn an_open_that_waits_is_the_handle_its_done_fills_in() {
    let rig = Rig::default();
    let (h, token) = opening(&rig);
    // The caller holds the token, not the handle: the number exists
    // and nothing may be done through it.
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 4 }), Error::Busy);
    assert!(rig.slow.reads.borrow().is_empty(), "nothing reached the Driver");

    rig.done(Ok(Out::Handle(Handle(7))));
    assert_eq!(rig.ok(Op::Ask { token }), Out::Handle(h), "the Kernel's own");
    rig.waits(Op::Read { h, at: 0, len: 4 });
    assert_eq!(*rig.slow.reads.borrow(), vec![7], "the Driver's cookie");
}

#[test]
fn an_open_refused_at_the_done_leaves_no_handle() {
    let rig = Rig::default();
    let (h, token) = opening(&rig);
    rig.done(Err(Error::Denied));
    assert_eq!(rig.err(Op::Ask { token }), Error::Denied);
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 4 }), Error::Invalid);
    assert!(rig.slow.closed.borrow().is_empty(), "no handle to close");
}

#[test]
fn an_open_closed_before_its_done_is_gone() {
    let rig = Rig::default();
    let (h, token) = opening(&rig);
    assert_eq!(rig.ok(Op::Close { h }), Out::Unit);
    assert_eq!(*rig.slow.closed.borrow(), vec![0], "no cookie to close on");
    rig.done(Ok(Out::Handle(Handle(7))));
    assert_eq!(rig.err(Op::Ask { token }), Error::Gone);
}

#[test]
fn an_open_cancelled_before_its_done_leaves_no_handle() {
    let rig = Rig::default();
    let (h, token) = opening(&rig);
    assert_eq!(rig.ok(Op::Cancel { token }), Out::Unit);
    rig.done(Ok(Out::Handle(Handle(7))));
    assert_eq!(rig.err(Op::Ask { token }), Error::Cancelled);
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 4 }), Error::Invalid);
}

#[test]
fn the_cap_counts_the_open_and_then_the_ops_on_its_handle() {
    let rig = Rig::new(Limits { pending_per_handle: 1, ..Limits::default() });
    let token = rig.waits(Op::Open { rd: rig.dir(), rights: Rights::READ });
    rig.done(Ok(Out::Handle(Handle(7))));
    let Out::Handle(h) = rig.ok(Op::Ask { token }) else { panic!("ask") };
    // The `open`'s own wait was counted on the slot and given back at
    // its `done`, so the handle arrives with all of its room.
    rig.waits(Op::Read { h, at: 0, len: 4 });
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 4 }), Error::Busy);
    assert_eq!(rig.slow.reads.borrow().len(), 1, "the second never ran");
}
