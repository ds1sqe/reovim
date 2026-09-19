//! A slot that answers `Pending` on the ops that name no handle, and
//! on the `open` whose handle does not exist yet.

mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use common::Rig;
use reovim_kernel_interface::{
    Caller, Calls, Driver, DriverId, Error, Flags, Handle, Module, Op, Out,
    Outcome, Rd, Reply, Rights, Token,
};
use reovim_kernel_runtime::{Limits, SYS};

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

    const fn dir(&self) -> Rd {
        self.dir.get().unwrap()
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

impl Rig {
    /// The Driver's first own token finished; every case makes one.
    fn done(&self, outcome: Outcome) {
        self.calls().done(Token(1), outcome).unwrap();
    }

    /// One handle on `/sys/core`.
    fn core_handle(&self) -> Handle {
        self.open(self.lookup(SYS, "core"), Rights::READ)
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

/// The Slow Module on a Kernel that stands still, and the Module.
fn limited(limits: Limits) -> (Rig, Rc<Slow>) {
    let slow = Rc::new(Slow::default());
    let driver: Rc<dyn Driver> = slow.clone();
    (Rig::over(|| 1, limits, "s", slow.clone(), vec![driver]), slow)
}

fn fresh() -> (Rig, Rc<Slow>) {
    limited(Limits::default())
}

/// An `open` of the Container that is waiting: the handle it made,
/// which the caller does not hold yet, and the token it holds instead.
fn opening(rig: &Rig, dir: Rd) -> (Handle, Token) {
    let h = rig.next_handle();
    (h, rig.waits(Op::Open { rd: dir, rights: Rights::READ }))
}

/// `op` run by a caller that is not the rig's, at `seq` of its own:
/// what a second Client on the one Kernel sends.
fn by(rig: &Rig, caller: Caller, seq: u64, op: Op) -> Result<Out, Error> {
    match rig.kernel.submit(caller, seq, op) {
        Reply::Ready(result) => result,
        other @ Reply::Pending(_) => panic!("expected Ready, got {other:?}"),
    }
}

/// The child the Driver registers while the caller waits, as its
/// `lookup` would have registered it at once.
fn child(rig: &Rig, dir: Rd, name: &str) -> Rd {
    let rd = rig.calls().register(DriverId(0), dir, Some(name)).unwrap();
    rig.calls().report(rd, Flags::READ, &[]).unwrap();
    rd
}

#[test]
fn a_lookup_that_waits_is_asked_out_as_the_child() {
    let (rig, slow) = fresh();
    let token = rig.waits(Op::Lookup { rd: slow.dir(), name: "a".into() });
    assert_eq!(rig.err(Op::Ask { token }), Error::Busy);
    let a = child(&rig, slow.dir(), "a");
    rig.done(Ok(Out::Rd(a)));
    assert_eq!(rig.ok(Op::Ask { token }), Out::Rd(a));
    assert_eq!(rig.err(Op::Ask { token }), Error::Invalid, "asked once");
}

#[test]
fn a_listing_that_waits_is_asked_out_as_the_children() {
    let (rig, slow) = fresh();
    let token = rig.waits(Op::Children { rd: slow.dir() });
    assert_eq!(rig.err(Op::Ask { token }), Error::Busy);
    let (a, b) = (child(&rig, slow.dir(), "a"), child(&rig, slow.dir(), "b"));
    // The listing is the Driver's own answer and not the table's: the
    // table was not full yet when the op returned.
    rig.done(Ok(Out::Rds(vec![a, b])));
    assert_eq!(rig.ok(Op::Ask { token }), Out::Rds(vec![a, b]));
    assert_eq!(rig.err(Op::Ask { token }), Error::Invalid, "asked once");
}

#[test]
fn a_cancel_before_the_done_answers_cancelled() {
    let (rig, slow) = fresh();
    let token = rig.waits(Op::Lookup { rd: slow.dir(), name: "a".into() });
    assert_eq!(rig.ok(Op::Cancel { token }), Out::Unit);
    let a = child(&rig, slow.dir(), "a");
    rig.done(Ok(Out::Rd(a)));
    assert_eq!(rig.err(Op::Ask { token }), Error::Cancelled);
    assert_eq!(rig.err(Op::Ask { token }), Error::Invalid);
}

#[test]
fn a_core_names_the_caller_of_a_wait_that_holds_no_handle() {
    let (rig, slow) = fresh();
    let lookup = rig.waits(Op::Lookup { rd: slow.dir(), name: "a".into() });
    let children = rig.waits(Op::Children { rd: slow.dir() });
    let core = rig.core();
    // The wait records the caller it is owed to whether or not it
    // names a handle, so only the handle is a dash.
    for token in [lookup, children] {
        let line = format!(
            "pending token={} caller={} handle=- state=waiting",
            token.0, rig.caller.0
        );
        assert!(core.lines().any(|l| l == line), "{line}: {core}");
    }
}

#[test]
fn a_listing_that_never_lands_leaves_the_rd_unlisted() {
    let (rig, slow) = fresh();
    let token = rig.waits(Op::Children { rd: slow.dir() });
    assert_eq!(rig.ok(Op::Cancel { token }), Out::Unit);

    // The Driver listed nothing, so the next `Children` asks it again
    // rather than answering for ever from a cache nobody filled.
    let again = rig.waits(Op::Children { rd: slow.dir() });
    let a = child(&rig, slow.dir(), "a");
    rig.calls().done(Token(2), Ok(Out::Rds(vec![a]))).unwrap();
    assert_eq!(rig.ok(Op::Ask { token: again }), Out::Rds(vec![a]));
    // Landed now: the cache is what answers and the Driver is done
    // with it.
    assert_eq!(rig.ok(Op::Children { rd: slow.dir() }), Out::Rds(vec![a]));
}

#[test]
fn a_listing_the_driver_refuses_leaves_the_rd_unlisted() {
    let (rig, slow) = fresh();
    let token = rig.waits(Op::Children { rd: slow.dir() });
    rig.done(Err(Error::Denied));
    assert_eq!(rig.err(Op::Ask { token }), Error::Denied);
    rig.waits(Op::Children { rd: slow.dir() });
}

#[test]
fn an_open_that_waits_is_the_handle_its_done_fills_in() {
    let (rig, slow) = fresh();
    let (h, token) = opening(&rig, slow.dir());
    // The caller holds the token, not the handle: the number exists
    // and nothing may be done through it.
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 4 }), Error::Busy);
    assert!(slow.reads.borrow().is_empty(), "nothing reached the Driver");

    rig.done(Ok(Out::Handle(Handle(7))));
    assert_eq!(rig.ok(Op::Ask { token }), Out::Handle(h), "the Kernel's own");
    rig.waits(Op::Read { h, at: 0, len: 4 });
    assert_eq!(*slow.reads.borrow(), vec![7], "the Driver's cookie");
}

#[test]
fn an_open_refused_at_the_done_leaves_no_handle() {
    let (rig, slow) = fresh();
    let (h, token) = opening(&rig, slow.dir());
    rig.done(Err(Error::Denied));
    assert_eq!(rig.err(Op::Ask { token }), Error::Denied);
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 4 }), Error::Invalid);
    assert!(slow.closed.borrow().is_empty(), "no handle to close");
}

#[test]
fn an_open_closed_before_its_done_is_gone() {
    let (rig, slow) = fresh();
    let (h, token) = opening(&rig, slow.dir());
    assert_eq!(rig.ok(Op::Close { h }), Out::Unit);
    assert_eq!(*slow.closed.borrow(), vec![0], "no cookie to close on");
    rig.done(Ok(Out::Handle(Handle(7))));
    assert_eq!(rig.err(Op::Ask { token }), Error::Gone);
}

#[test]
fn an_open_cancelled_before_its_done_leaves_no_handle() {
    let (rig, slow) = fresh();
    let (h, token) = opening(&rig, slow.dir());
    assert_eq!(rig.ok(Op::Cancel { token }), Out::Unit);
    rig.done(Ok(Out::Handle(Handle(7))));
    assert_eq!(rig.err(Op::Ask { token }), Error::Cancelled);
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 4 }), Error::Invalid);
}

#[test]
fn another_callers_ask_is_denied_and_takes_nothing() {
    let (rig, slow) = fresh();
    let token = rig.waits(Op::Lookup { rd: slow.dir(), name: "a".into() });
    let b = rig.kernel.caller_new();
    assert_eq!(by(&rig, b, 1, Op::Ask { token }), Err(Error::Denied));
    let a = child(&rig, slow.dir(), "a");
    rig.done(Ok(Out::Rd(a)));
    // The answer is the token's own caller's: B's second try does not
    // collect it and the first try left it where it was.
    assert_eq!(by(&rig, b, 2, Op::Ask { token }), Err(Error::Denied));
    assert_eq!(rig.ok(Op::Ask { token }), Out::Rd(a));
}

#[test]
fn another_callers_cancel_is_denied_and_the_handle_stands() {
    let (rig, slow) = fresh();
    let (h, token) = opening(&rig, slow.dir());
    let b = rig.kernel.caller_new();
    assert_eq!(by(&rig, b, 1, Op::Cancel { token }), Err(Error::Denied));
    // The slot the waiting `open` made is still there for its cookie.
    rig.done(Ok(Out::Handle(Handle(7))));
    assert_eq!(rig.ok(Op::Ask { token }), Out::Handle(h));
    rig.waits(Op::Read { h, at: 0, len: 4 });
    assert_eq!(*slow.reads.borrow(), vec![7], "the Driver's cookie");
}

#[test]
fn a_caller_that_ends_takes_its_waits_with_it() {
    let (rig, slow) = fresh();
    let b = rig.kernel.caller_new();
    let lookup = Op::Lookup { rd: slow.dir(), name: "a".into() };
    let Reply::Pending(token) = rig.kernel.submit(b, 1, lookup) else {
        panic!("expected Pending")
    };
    let waiting = format!("pending token={} ", token.0);
    let core = rig.core();
    assert!(core.lines().any(|l| l.starts_with(&waiting)), "{core}");
    rig.kernel.caller_end(b);
    // Nobody is owed the answer any more, so the row is not kept for
    // an `Ask` that can never come: the token is unknown, not waiting.
    let core = rig.core();
    assert!(!core.lines().any(|l| l.starts_with(&waiting)), "{core}");
    let c = rig.kernel.caller_new();
    assert_eq!(by(&rig, c, 1, Op::Ask { token }), Err(Error::Invalid));
    let a = child(&rig, slow.dir(), "a");
    let late = rig.calls().done(Token(1), Ok(Out::Rd(a)));
    assert_eq!(late, Err(Error::Invalid), "the Driver's own token too");
}

#[test]
fn the_cap_counts_the_open_and_then_the_ops_on_its_handle() {
    let (rig, slow) =
        limited(Limits { pending_per_handle: 1, ..Limits::default() });
    let token = rig.waits(Op::Open { rd: slow.dir(), rights: Rights::READ });
    rig.done(Ok(Out::Handle(Handle(7))));
    let Out::Handle(h) = rig.ok(Op::Ask { token }) else { panic!("ask") };
    // The `open`'s own wait was counted on the slot and given back at
    // its `done`, so the handle arrives with all of its room.
    rig.waits(Op::Read { h, at: 0, len: 4 });
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 4 }), Error::Busy);
    assert_eq!(slow.reads.borrow().len(), 1, "the second never ran");
}
