//! A Session ends with its last caller: two unnamed callers in one
//! Session, a named one that keeps its Session while its link is away,
//! and the Module state under `/session/<id>` that goes with it.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use reovim_kernel_interface::{
    Caller, Calls, Driver, DriverId, Error, Flags, Kind, Module, ModuleId, Op,
    Out, Rd, Reply, Rights,
};
use reovim_kernel_runtime::{Kernel, Limits, ROOT};

static CLOCK: AtomicU64 = AtomicU64::new(1);

fn tick() -> u64 {
    CLOCK.fetch_add(1, Ordering::Relaxed)
}

/// A Module that registers where it is told and opens what it
/// registered: the per-session state of 00-core.md, and nothing else.
struct Store;

impl Module for Store {
    fn start(&self, _: &dyn Calls, _: Rd) -> Result<(), Error> {
        Ok(())
    }
}

impl Driver for Store {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn close(&self, _: &dyn Calls, _: u64) -> Reply<()> {
        Reply::ok(())
    }
}

/// A Kernel with the Store Module loaded and one caller of its own
/// that watches whatever a test tells it to.
struct Rig {
    kernel: Kernel,
    module: ModuleId,
    eyes: Caller,
    seq: Cell<u64>,
    cursor: Cell<u64>,
}

impl Rig {
    fn new() -> Self {
        let kernel = Kernel::new(tick, Limits::default());
        let store = Rc::new(Store);
        let driver: Rc<dyn Driver> = store.clone();
        let module = kernel.load("store", store, vec![driver]).unwrap();
        let eyes = kernel.caller_new();
        let (seq, cursor) = (Cell::new(0), Cell::new(0));
        Self { kernel, module, eyes, seq, cursor }
    }

    fn ok(&self, op: Op) -> Out {
        self.seq.set(self.seq.get() + 1);
        match self.kernel.submit(self.eyes, self.seq.get(), op) {
            Reply::Ready(Ok(out)) => out,
            other => panic!("expected Ready(Ok), got {other:?}"),
        }
    }

    fn err(&self, op: Op) -> Error {
        self.seq.set(self.seq.get() + 1);
        match self.kernel.submit(self.eyes, self.seq.get(), op) {
            Reply::Ready(Err(error)) => error,
            other => panic!("expected Ready(Err), got {other:?}"),
        }
    }

    fn lookup(&self, rd: Rd, name: &str) -> Rd {
        match self.ok(Op::Lookup { rd, name: name.to_string() }) {
            Out::Rd(rd) => rd,
            other => panic!("{other:?}"),
        }
    }

    fn children(&self, rd: Rd) -> Vec<Rd> {
        match self.ok(Op::Children { rd }) {
            Out::Rds(rds) => rds,
            other => panic!("{other:?}"),
        }
    }

    /// One more rd whose events this rig sees; the handle stays open.
    fn watch(&self, rd: Rd) {
        let Out::Handle(h) = self.ok(Op::Open { rd, rights: Rights::READ })
        else {
            panic!("open rd:{}", rd.0)
        };
        self.ok(Op::Watch { h, on: true });
    }

    /// What the watched rds have seen since the last ask.
    fn kinds(&self) -> Vec<(Rd, Kind, Option<Rd>)> {
        let Out::Events { events, cursor } =
            self.ok(Op::Since { cursor: self.cursor.get() })
        else {
            panic!("events")
        };
        self.cursor.set(cursor);
        events.iter().map(|e| (e.rd, e.kind, e.child)).collect()
    }
}

#[test]
fn a_session_ends_with_its_last_unnamed_caller() {
    let rig = Rig::new();
    let sessions = rig.lookup(ROOT, "session");
    rig.watch(sessions);
    let session = rig.kernel.session_new();
    let (first, one) = rig.kernel.connect(session, b"").unwrap();
    let (second, two) = rig.kernel.connect(session, b"").unwrap();
    let clients = rig.lookup(session, "client");
    assert_eq!(rig.children(clients), vec![one, two]);
    assert_eq!(rig.kinds(), vec![(sessions, Kind::Create, Some(session))]);

    // One link of the two ends: its client rd goes with it, since an
    // empty credential can never come back for it. The Session stays.
    rig.kernel.caller_end(first);
    assert_eq!(rig.children(clients), vec![two]);
    rig.ok(Op::Info { rd: session });
    assert_eq!(rig.kinds(), vec![], "/session saw nothing");

    // The last caller ends, and no caller of it is named: the Session
    // is gone and `/session` says so.
    rig.kernel.caller_end(second);
    assert_eq!(rig.err(Op::Info { rd: session }), Error::Invalid);
    assert_eq!(rig.children(sessions), Vec::new());
    assert_eq!(rig.kinds(), vec![(sessions, Kind::Delete, Some(session))]);
}

#[test]
fn a_named_caller_keeps_its_session_while_it_is_away() {
    let rig = Rig::new();
    let sessions = rig.lookup(ROOT, "session");
    rig.watch(sessions);
    let session = rig.kernel.session_new();
    let (tui, seat) = rig.kernel.connect(session, b"tui:7").unwrap();
    let (cli, _) = rig.kernel.connect(session, b"").unwrap();
    let clients = rig.lookup(session, "client");

    // Both links end, the named one first: the Session may not go
    // while a caller of it can still be adopted.
    rig.kernel.caller_end(tui);
    rig.kernel.caller_end(cli);
    assert_eq!(rig.children(clients), vec![seat], "the named client rd");
    assert_eq!(rig.kinds(), vec![(sessions, Kind::Create, Some(session))]);

    // The next Connect under that name is the same caller and the
    // same client rd, and ending it again changes nothing.
    assert_eq!(rig.kernel.connect(session, b"tui:7"), Ok((tui, seat)));
    rig.kernel.caller_end(tui);
    rig.ok(Op::Info { rd: session });
    assert_eq!(rig.kinds(), vec![], "nothing was deleted");
}

#[test]
fn a_session_takes_the_module_state_under_it() {
    let rig = Rig::new();
    let sessions = rig.lookup(ROOT, "session");
    rig.watch(sessions);
    let session = rig.kernel.session_new();
    let (caller, client) = rig.kernel.connect(session, b"").unwrap();
    let calls = rig.kernel.calls(rig.module);
    let state = calls.register(DriverId(0), session, Some("text")).unwrap();
    calls.report(state, Flags::READ | Flags::CONTAINER, &[]).unwrap();
    let buffer = calls.register(DriverId(0), state, Some("buffer")).unwrap();
    calls.report(buffer, Flags::READ, &[]).unwrap();
    let clients = rig.lookup(session, "client");
    let capability = rig.lookup(client, "capability");
    for rd in [session, clients, client, capability, state, buffer] {
        rig.watch(rd);
    }
    rig.kinds();

    rig.kernel.caller_end(caller);
    let gone = |rd: Rd| (rd, Kind::Gone, None);
    assert_eq!(
        rig.kinds(),
        vec![
            // The client rd of the ended caller, then the Session:
            // deepest first, the Delete on `/session` last.
            gone(capability),
            gone(client),
            (clients, Kind::Delete, Some(client)),
            gone(buffer),
            gone(state),
            gone(clients),
            gone(session),
            (sessions, Kind::Delete, Some(session)),
        ]
    );
    assert_eq!(rig.err(Op::Info { rd: buffer }), Error::Invalid);
    assert_eq!(rig.err(Op::Children { rd: session }), Error::Invalid);
}
