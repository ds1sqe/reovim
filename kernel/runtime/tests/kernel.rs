//! The Kernel against one test Module: a byte cell with an api, and a
//! Container whose children appear on `lookup`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use reovim_kernel_interface::{
    Caller, Calls, Decl, Driver, DriverId, Error, Event, Flags, Handle, Hash,
    Kind, Method, Module, ModuleId, Op, Out, Rd, Reply, Rights, Token, Ty,
};
use reovim_kernel_runtime::{Kernel, Limits, ROOT, SYS};

static CLOCK: AtomicU64 = AtomicU64::new(1);

fn tick() -> u64 {
    CLOCK.fetch_add(1, Ordering::Relaxed)
}

#[derive(Default)]
struct Inner {
    cell: Option<Rd>,
    dir: Option<Rd>,
    bytes: Vec<u8>,
    /// Slot runs that reached the Driver.
    runs: usize,
    /// Answer `Pending` from `read` and method 2; the own token used.
    later: bool,
    next_token: u64,
    hints: Vec<(Rd, bool)>,
    closed: Vec<u64>,
}

#[derive(Default)]
struct Test {
    inner: Mutex<Inner>,
}

fn decl() -> Decl {
    let m = |number, name: &str, input, output, right| Method {
        number,
        name: name.to_string(),
        input,
        output,
        right,
    };
    Decl {
        api: "cell".to_string(),
        ver: 1,
        methods: vec![
            m(0, "get", Ty::Unit, Ty::Bytes(64), Rights::READ),
            m(1, "set", Ty::Bytes(16), Ty::Unit, Rights::WRITE),
            m(2, "later", Ty::Unit, Ty::Bytes(64), Rights::READ),
        ],
    }
}

fn api() -> Hash {
    decl().hash()
}

impl Module for Test {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let cell = ctx.register(DriverId(0), home, Some("cell"))?;
        ctx.report(cell, Flags::READ | Flags::WRITE, &[decl()])?;
        let dir = ctx.register(DriverId(0), home, Some("dir"))?;
        ctx.report(dir, Flags::READ | Flags::CONTAINER, &[])?;
        let mut inner = self.inner.lock().unwrap();
        inner.cell = Some(cell);
        inner.dir = Some(dir);
        drop(inner);
        Ok(())
    }
}

impl Driver for Test {
    fn lookup(&self, ctx: &dyn Calls, rd: Rd, name: &str) -> Reply<Rd> {
        self.inner.lock().unwrap().runs += 1;
        if name.is_empty() {
            let x = child(ctx, rd, "x");
            let _ = child(ctx, rd, "y");
            return Reply::Ready(x);
        }
        Reply::Ready(child(ctx, rd, name))
    }

    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0 + 100)
    }

    fn read(
        &self,
        _: &dyn Calls,
        _: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        let mut inner = self.inner.lock().unwrap();
        inner.runs += 1;
        if inner.later {
            return pending(&mut inner);
        }
        let start = usize::try_from(at).unwrap().min(inner.bytes.len());
        let end = (start + len).min(inner.bytes.len());
        Reply::ok(inner.bytes[start..end].to_vec())
    }

    fn write(
        &self,
        _: &dyn Calls,
        _: u64,
        at: u64,
        buf: &[u8],
    ) -> Reply<usize> {
        write(&mut self.inner.lock().unwrap(), at, buf)
    }

    fn call(
        &self,
        _: &dyn Calls,
        _: u64,
        _: Hash,
        method: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        call(&mut self.inner.lock().unwrap(), method, input)
    }

    fn watch(&self, _: &dyn Calls, rd: Rd, on: bool) -> Reply<()> {
        self.inner.lock().unwrap().hints.push((rd, on));
        Reply::ok(())
    }

    fn close(&self, _: &dyn Calls, h: u64) -> Reply<()> {
        self.inner.lock().unwrap().closed.push(h);
        Reply::ok(())
    }
}

/// Register and report one Read child.
fn child(ctx: &dyn Calls, parent: Rd, name: &str) -> Result<Rd, Error> {
    let rd = ctx.register(DriverId(0), parent, Some(name))?;
    ctx.report(rd, Flags::READ, &[])?;
    Ok(rd)
}

fn write(inner: &mut Inner, at: u64, buf: &[u8]) -> Reply<usize> {
    inner.runs += 1;
    let at = usize::try_from(at).unwrap();
    inner.bytes.truncate(at);
    inner.bytes.extend_from_slice(buf);
    Reply::ok(buf.len())
}

fn call(inner: &mut Inner, method: u16, input: &[u8]) -> Reply<Vec<u8>> {
    inner.runs += 1;
    match method {
        0 => Reply::ok(inner.bytes.clone()),
        1 => {
            inner.bytes = input[4..].to_vec();
            Reply::ok(Vec::new())
        }
        _ => pending(inner),
    }
}

const fn pending<T>(inner: &mut Inner) -> Reply<T> {
    inner.next_token += 1;
    Reply::Pending(Token(inner.next_token))
}

struct Rig {
    kernel: Kernel,
    module: ModuleId,
    test: Arc<Test>,
    caller: Caller,
    seq: std::cell::Cell<u64>,
}

impl Rig {
    fn new(limits: Limits) -> Self {
        let kernel = Kernel::new(tick, limits);
        let test = Arc::new(Test::default());
        let driver: Arc<dyn Driver> = test.clone();
        let module = kernel.load("t", test.as_ref(), vec![driver]).unwrap();
        let caller = kernel.caller_new();
        Self { kernel, module, test, caller, seq: std::cell::Cell::new(0) }
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

    fn lookup(&self, rd: Rd, name: &str) -> Rd {
        match self.ok(Op::Lookup { rd, name: name.to_string() }) {
            Out::Rd(rd) => rd,
            other => panic!("{other:?}"),
        }
    }

    fn open(&self, rd: Rd, rights: Rights) -> Handle {
        match self.ok(Op::Open { rd, rights }) {
            Out::Handle(h) => h,
            other => panic!("{other:?}"),
        }
    }

    fn cell(&self) -> Rd {
        self.test.inner.lock().unwrap().cell.unwrap()
    }

    fn dir(&self) -> Rd {
        self.test.inner.lock().unwrap().dir.unwrap()
    }

    fn runs(&self) -> usize {
        self.test.inner.lock().unwrap().runs
    }

    fn calls(&self) -> impl Calls + '_ {
        self.kernel.calls(self.module)
    }

    fn since(&self, cursor: u64) -> (Vec<Event>, u64) {
        match self.ok(Op::Since { cursor }) {
            Out::Events { events, cursor } => (events, cursor),
            other => panic!("{other:?}"),
        }
    }

    fn kinds(&self, cursor: u64) -> Vec<(Rd, Kind, Option<Rd>)> {
        self.since(cursor).0.iter().map(|e| (e.rd, e.kind, e.child)).collect()
    }

    fn info(&self, rd: Rd) -> reovim_kernel_interface::Info {
        match self.ok(Op::Info { rd }) {
            Out::Info(info) => info,
            other => panic!("{other:?}"),
        }
    }
}

fn rw() -> Rights {
    Rights::READ | Rights::WRITE
}

fn blob(bytes: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(bytes.len()).unwrap().to_le_bytes().to_vec();
    out.extend_from_slice(bytes);
    out
}

/// Pinned on 2026-09-16; a change here is a wire-visible change.
const GOLDEN: Hash = Hash(0x6fd5_f6d9_6c0e_7f0a);

#[test]
fn declaration_hash_is_stable_and_order_free() {
    let mut swapped = decl();
    swapped.methods.reverse();
    assert_eq!(swapped.hash(), decl().hash());
    assert_eq!(decl().hash(), GOLDEN);
}

#[test]
fn fixed_tree_is_reachable_from_root() {
    let rig = Rig::default();
    assert!(rig.cell().0 >= 100, "registered rds start at 100");
    assert_eq!(rig.lookup(ROOT, "sys"), SYS);
    for name in ["module", "session", "var"] {
        let rd = rig.lookup(ROOT, name);
        assert_eq!(rig.ok(Op::Parent { rd }), Out::Rd(ROOT));
    }
    let log = rig.lookup(SYS, "log");
    let decls = rig.lookup(SYS, "decls");
    assert_eq!(rig.ok(Op::Children { rd: SYS }), Out::Rds(vec![decls, log]));
    assert_eq!(
        rig.err(Op::Lookup { rd: ROOT, name: "none".into() }),
        Error::Invalid
    );
    assert_eq!(rig.err(Op::Parent { rd: ROOT }), Error::Invalid);
}

#[test]
fn load_makes_home_and_start_registers_below_it() {
    let rig = Rig::default();
    let modules = rig.lookup(ROOT, "module");
    let home = rig.lookup(modules, "t");
    assert_eq!(rig.lookup(home, "cell"), rig.cell());
    let info = rig.info(rig.cell());
    assert_eq!(info.name.as_deref(), Some("cell"));
    assert_eq!(info.flags, Flags::READ | Flags::WRITE);
    assert_eq!(info.apis, vec![api()]);
    let again = rig.kernel.load("t", rig.test.as_ref(), vec![]);
    assert_eq!(again.err(), Some(Error::Invalid));
}

#[test]
fn lookup_registers_the_child_inside_the_slot() {
    let rig = Rig::default();
    let dir = rig.dir();
    let a = rig.lookup(dir, "a");
    assert_eq!(rig.ok(Op::Parent { rd: a }), Out::Rd(dir));
    assert_eq!(rig.lookup(dir, "a"), a, "cached; the slot is not asked");
    assert_eq!(rig.runs(), 1);
    let listed = rig.ok(Op::Children { rd: dir });
    let x = rig.lookup(dir, "x");
    let y = rig.lookup(dir, "y");
    assert_eq!(listed, Out::Rds(vec![a, x, y]));
    assert_eq!(rig.runs(), 2, "listed once");
    rig.ok(Op::Children { rd: dir });
    assert_eq!(rig.runs(), 2);
}

#[test]
fn register_follows_the_parent_owner() {
    let rig = Rig::default();
    let other = Arc::new(Test::default());
    let driver: Arc<dyn Driver> = other.clone();
    let u = rig.kernel.load("u", other.as_ref(), vec![driver]).unwrap();
    let calls = rig.kernel.calls(u);
    let home_t = rig.lookup(rig.lookup(ROOT, "module"), "t");
    assert_eq!(calls.register(DriverId(0), home_t, None), Err(Error::Denied));
    assert_eq!(
        calls.register(DriverId(0), rig.dir(), None),
        Err(Error::Denied)
    );
    assert_eq!(calls.register(DriverId(0), ROOT, None), Err(Error::Denied));
    let var = rig.lookup(ROOT, "var");
    let in_var = calls.register(DriverId(0), var, Some("v")).unwrap();
    assert_eq!(rig.ok(Op::Parent { rd: in_var }), Out::Rd(var));
    let session = rig.kernel.session_new();
    assert!(calls.register(DriverId(0), session, Some("s")).is_ok());
    assert_eq!(
        calls.register(DriverId(0), var, Some("v")),
        Err(Error::Invalid)
    );
    assert_eq!(
        calls.register(DriverId(0), var, Some("a/b")),
        Err(Error::Invalid)
    );
    assert_eq!(calls.register(DriverId(1), var, None), Err(Error::Invalid));
    assert_eq!(calls.unregister(rig.cell()), Err(Error::Denied));
    assert_eq!(calls.name(rig.cell(), None), Err(Error::Denied));
}

#[test]
fn declarations_are_stored_once_and_served() {
    let rig = Rig::default();
    let decls = rig.lookup(SYS, "decls");
    let name = format!("{:016x}", api().0);
    let rd = rig.lookup(decls, &name);
    let h = rig.open(rd, Rights::READ);
    let bytes = rig.ok(Op::Read { h, at: 0, len: 1 << 16 });
    assert_eq!(bytes, Out::Bytes(decl().canonical()));
    assert_eq!(
        rig.ok(Op::Read { h, at: 0, len: 2 }),
        Out::Bytes(decl().canonical()[..2].to_vec())
    );
    rig.calls().report(rig.cell(), Flags::READ, &[decl()]).unwrap();
    assert_eq!(rig.ok(Op::Children { rd: decls }), Out::Rds(vec![rd]));
    let mut wide = decl();
    wide.methods.extend((3..300).map(|n| Method {
        number: n,
        name: "m".into(),
        input: Ty::Unit,
        output: Ty::Unit,
        right: Rights::READ,
    }));
    assert_eq!(
        rig.calls().report(rig.cell(), Flags::READ, &[wide]),
        Err(Error::Invalid)
    );
}

#[test]
fn open_is_capped_by_flags() {
    let rig = Rig::default();
    assert_eq!(
        rig.err(Op::Open { rd: rig.dir(), rights: Rights::WRITE }),
        Error::Denied
    );
    assert_eq!(
        rig.err(Op::Open { rd: rig.cell(), rights: Rights::empty() }),
        Error::Denied
    );
    assert_eq!(
        rig.err(Op::Open { rd: Rd(999), rights: Rights::READ }),
        Error::Invalid
    );
    let h = rig.open(rig.cell(), Rights::READ);
    assert_eq!(rig.err(Op::Write { h, at: 0, bytes: vec![1] }), Error::Denied);
    let stranger = rig.kernel.caller_new();
    assert_eq!(
        rig.kernel.submit(stranger, 1, Op::Close { h }),
        Reply::err(Error::Denied)
    );
    assert_eq!(
        rig.kernel.submit(Caller(77), 1, Op::Close { h }),
        Reply::err(Error::Invalid)
    );
}

#[test]
fn read_and_write_go_through_the_handle() {
    let rig = Rig::default();
    let h = rig.open(rig.cell(), rw());
    let before = rig.info(rig.cell()).atime;
    assert_eq!(
        rig.ok(Op::Write { h, at: 0, bytes: b"hello".to_vec() }),
        Out::Len(5)
    );
    assert_eq!(
        rig.ok(Op::Read { h, at: 1, len: 3 }),
        Out::Bytes(b"ell".to_vec())
    );
    assert!(rig.info(rig.cell()).atime > before, "read stamps atime");
    assert_eq!(rig.ok(Op::Read { h, at: 5, len: 3 }), Out::Bytes(vec![]));
    rig.ok(Op::Close { h });
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 1 }), Error::Invalid);
    let cookie = rig.cell().0 + 100;
    assert_eq!(rig.test.inner.lock().unwrap().closed, vec![cookie]);
}

#[test]
fn call_is_checked_before_the_driver_runs() {
    let rig = Rig::default();
    let ro = rig.open(rig.cell(), Rights::READ);
    let call =
        |h, api, method, input: Vec<u8>| Op::Call { h, api, method, input };
    assert_eq!(rig.err(call(ro, Hash(1), 0, vec![])), Error::Unsupported);
    assert_eq!(rig.err(call(ro, api(), 9, vec![])), Error::Unsupported);
    assert_eq!(rig.err(call(ro, api(), 1, blob(b"x"))), Error::Denied);
    let h = rig.open(rig.cell(), rw());
    assert_eq!(rig.err(call(h, api(), 1, blob(&[0; 17]))), Error::Invalid);
    assert_eq!(rig.err(call(h, api(), 1, b"x".to_vec())), Error::Invalid);
    assert_eq!(rig.err(call(h, api(), 0, vec![1])), Error::Invalid);
    assert_eq!(rig.runs(), 0, "nothing reached the Driver");
    assert_eq!(rig.ok(call(h, api(), 1, blob(b"hi"))), Out::Bytes(vec![]));
    assert_eq!(rig.ok(call(ro, api(), 0, vec![])), Out::Bytes(b"hi".to_vec()));
    assert_eq!(rig.runs(), 2);
    let sys = rig.open(SYS, Rights::READ);
    assert_eq!(rig.err(call(sys, api(), 0, vec![])), Error::Unsupported);
}

#[test]
fn pending_is_finished_by_done_and_asked_once() {
    let rig = Rig::default();
    let h = rig.open(rig.cell(), Rights::READ);
    let Reply::Pending(token) =
        rig.submit(Op::Call { h, api: api(), method: 2, input: vec![] })
    else {
        panic!("expected Pending");
    };
    assert_eq!(rig.err(Op::Ask { token }), Error::Busy);
    assert_eq!(rig.calls().done(Token(9), Ok(Out::Unit)), Err(Error::Invalid));
    rig.calls().done(Token(1), Ok(Out::Bytes(b"late".to_vec()))).unwrap();
    assert_eq!(rig.calls().done(Token(1), Ok(Out::Unit)), Err(Error::Invalid));
    assert_eq!(rig.ok(Op::Ask { token }), Out::Bytes(b"late".to_vec()));
    assert_eq!(rig.err(Op::Ask { token }), Error::Invalid);
}

#[test]
fn cancel_drops_the_later_done() {
    let rig = Rig::default();
    rig.test.inner.lock().unwrap().later = true;
    let h = rig.open(rig.cell(), Rights::READ);
    let Reply::Pending(token) = rig.submit(Op::Read { h, at: 0, len: 4 })
    else {
        panic!("expected Pending");
    };
    assert_eq!(rig.ok(Op::Cancel { token }), Out::Unit);
    assert_eq!(rig.err(Op::Cancel { token: Token(42) }), Error::Invalid);
    rig.calls().done(Token(1), Ok(Out::Bytes(vec![1]))).unwrap();
    assert_eq!(rig.err(Op::Ask { token }), Error::Cancelled);
    assert_eq!(rig.err(Op::Ask { token }), Error::Invalid);
}

#[test]
fn pending_cap_answers_busy() {
    let rig = Rig::new(Limits { pending_per_handle: 1, ..Limits::default() });
    let h = rig.open(rig.cell(), Rights::READ);
    let op = || Op::Call { h, api: api(), method: 2, input: vec![] };
    assert!(matches!(rig.submit(op()), Reply::Pending(_)));
    assert_eq!(rig.err(op()), Error::Busy);
    rig.calls().done(Token(1), Ok(Out::Unit)).unwrap();
    assert!(matches!(rig.submit(op()), Reply::Pending(_)));
}

#[test]
fn a_repeated_sequence_is_answered_from_memory() {
    let rig = Rig::new(Limits { reply_window: 2, ..Limits::default() });
    let h = rig.open(rig.cell(), rw());
    let write = Op::Write { h, at: 0, bytes: b"once".to_vec() };
    let first = rig.kernel.submit(rig.caller, 10, write.clone());
    assert_eq!(first, Reply::ok(Out::Len(4)));
    assert_eq!(rig.kernel.submit(rig.caller, 10, write.clone()), first);
    assert_eq!(rig.runs(), 1, "the slot ran once");
    assert_eq!(
        rig.kernel.submit(rig.caller, 5, write.clone()),
        Reply::err(Error::Stale)
    );
    rig.kernel.submit(rig.caller, 11, Op::Info { rd: ROOT });
    rig.kernel.submit(rig.caller, 12, Op::Info { rd: ROOT });
    assert_eq!(
        rig.kernel.submit(rig.caller, 10, write),
        Reply::err(Error::Stale)
    );
    assert_eq!(rig.runs(), 1);
}

#[test]
fn watch_modify_and_attrib_come_from_the_driver() {
    let rig = Rig::default();
    let cell = rig.cell();
    let h = rig.open(cell, Rights::READ);
    let (_, cursor) = rig.since(0);
    rig.ok(Op::Watch { h, on: true });
    assert_eq!(rig.test.inner.lock().unwrap().hints, vec![(cell, true)]);
    assert_eq!(rig.calls().changed(cell, Kind::Create), Err(Error::Invalid));
    let before = rig.info(cell);
    rig.calls().changed(cell, Kind::Modify).unwrap();
    rig.calls().changed(cell, Kind::Attrib).unwrap();
    let after = rig.info(cell);
    assert!(after.mtime > before.mtime && after.ctime > before.ctime);
    let expected = vec![(cell, Kind::Modify, None), (cell, Kind::Attrib, None)];
    assert_eq!(rig.kinds(cursor), expected);
    let (_, cursor) = rig.since(cursor);
    assert_eq!(rig.kinds(cursor), vec![]);
    rig.ok(Op::Watch { h, on: false });
    rig.calls().changed(cell, Kind::Modify).unwrap();
    assert_eq!(rig.kinds(cursor), vec![], "unwatched");
}

#[test]
fn watch_tree_kinds_come_from_the_kernel() {
    let rig = Rig::default();
    let dir = rig.dir();
    let h = rig.open(dir, Rights::READ);
    rig.ok(Op::Watch { h, on: true });
    let (_, cursor) = rig.since(0);
    let b = rig.lookup(dir, "b");
    assert_eq!(rig.kinds(cursor), vec![(dir, Kind::Create, Some(b))]);
    let (_, cursor) = rig.since(cursor);
    rig.calls().name(b, Some("c")).unwrap();
    assert_eq!(rig.info(b).name.as_deref(), Some("c"));
    assert_eq!(rig.lookup(dir, "c"), b);
    assert_eq!(rig.kinds(cursor), vec![(dir, Kind::Move, Some(b))]);
    let (_, cursor) = rig.since(cursor);
    let hb = rig.open(b, Rights::READ);
    rig.ok(Op::Watch { h: hb, on: true });
    rig.calls().unregister(b).unwrap();
    let expected = vec![(b, Kind::Gone, None), (dir, Kind::Delete, Some(b))];
    assert_eq!(rig.kinds(cursor), expected);
    assert_eq!(rig.err(Op::Read { h: hb, at: 0, len: 1 }), Error::Gone);
    assert_eq!(rig.err(Op::Info { rd: b }), Error::Invalid);
    assert_eq!(rig.ok(Op::Close { h: hb }), Out::Unit);
}

#[test]
fn unregister_takes_the_subtree_deepest_first() {
    let rig = Rig::default();
    let dir = rig.dir();
    let a = rig.lookup(dir, "a");
    let h = rig.open(a, Rights::READ);
    rig.ok(Op::Watch { h, on: true });
    let hd = rig.open(dir, Rights::READ);
    rig.ok(Op::Watch { h: hd, on: true });
    let (_, cursor) = rig.since(0);
    rig.calls().unregister(dir).unwrap();
    let expected = vec![(a, Kind::Gone, None), (dir, Kind::Gone, None)];
    assert_eq!(rig.kinds(cursor), expected);
    assert_eq!(rig.err(Op::Children { rd: dir }), Error::Invalid);
}

#[test]
fn a_slow_watcher_gets_overflow() {
    let rig = Rig::new(Limits { watch_ring: 2, ..Limits::default() });
    let cell = rig.cell();
    let h = rig.open(cell, Rights::READ);
    rig.ok(Op::Watch { h, on: true });
    let (_, cursor) = rig.since(0);
    for _ in 0..5 {
        rig.calls().changed(cell, Kind::Modify).unwrap();
    }
    let (events, head) = rig.since(cursor);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, Kind::Overflow);
    assert_eq!(head, events[0].seq);
    assert_eq!(rig.kinds(head), vec![]);
    rig.calls().changed(cell, Kind::Modify).unwrap();
    assert_eq!(rig.kinds(head), vec![(cell, Kind::Modify, None)]);
}

#[test]
fn close_implies_unwatch() {
    let rig = Rig::default();
    let cell = rig.cell();
    let h = rig.open(cell, Rights::READ);
    rig.ok(Op::Watch { h, on: true });
    let (_, cursor) = rig.since(0);
    rig.ok(Op::Close { h });
    rig.calls().changed(cell, Kind::Modify).unwrap();
    assert_eq!(rig.kinds(cursor), vec![]);
}

#[test]
fn the_log_holds_the_call_pair_and_module_lines() {
    let rig = Rig::default();
    let h = rig.open(rig.cell(), Rights::READ);
    rig.ok(Op::Call { h, api: api(), method: 0, input: vec![] });
    let log = rig.lookup(SYS, "log");
    let hl = rig.calls().open(log, rw()).unwrap();
    let wrote = rig.calls().submit(Op::Write {
        h: hl,
        at: 0,
        bytes: b"hello".to_vec(),
    });
    assert_eq!(wrote, Reply::ok(Out::Len(5)));
    let h = rig.open(log, Rights::READ);
    let Out::Bytes(bytes) = rig.ok(Op::Read { h, at: 1, len: 1 << 16 }) else {
        panic!()
    };
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("method=0 dispatch"), "{text}");
    assert!(text.contains("method=0 reply"), "{text}");
    assert!(text.contains("kernel"), "{text}");
    assert!(
        text.contains("hello") && text.contains("\u{1}\0\0\0t"),
        "mark is the Module name"
    );
    assert_eq!(rig.ok(Op::Read { h, at: 0, len: 4 }), Out::Bytes(vec![]));
}
