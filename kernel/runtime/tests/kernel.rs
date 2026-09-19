//! The Kernel against two test Modules: one with a byte cell with an
//! api and a Container whose children appear on `lookup`, and one
//! whose Driver goes back through the Kernel for its own work.

mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Mutex;

use common::{Rig, entries, tick};
use reovim_kernel_interface::{
    Caller, Calls, Decl, Driver, DriverId, Error, Flags, Hash, Kind, Method,
    Module, Op, Out, Rd, Reply, Rights, Token, Ty,
};
use reovim_kernel_runtime::{Limits, ROOT, SYS};

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

impl Test {
    fn cell(&self) -> Rd {
        self.inner.lock().unwrap().cell.unwrap()
    }

    fn dir(&self) -> Rd {
        self.inner.lock().unwrap().dir.unwrap()
    }

    fn runs(&self) -> usize {
        self.inner.lock().unwrap().runs
    }
}

impl Rig {
    fn info(&self, rd: Rd) -> reovim_kernel_interface::Info {
        match self.ok(Op::Info { rd }) {
            Out::Info(info) => info,
            other => panic!("{other:?}"),
        }
    }
}

/// The Test Module on a Kernel bounded by `limits`, and the Module.
fn limited(limits: Limits) -> (Rig, Rc<Test>) {
    let test = Rc::new(Test::default());
    let driver: Rc<dyn Driver> = test.clone();
    (Rig::over(tick, limits, "t", test.clone(), vec![driver]), test)
}

fn fresh() -> (Rig, Rc<Test>) {
    limited(Limits::default())
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
    let (rig, test) = fresh();
    assert!(test.cell().0 >= 100, "registered rds start at 100");
    assert_eq!(rig.lookup(ROOT, "sys"), SYS);
    for name in ["bin", "module", "session", "var"] {
        let rd = rig.lookup(ROOT, name);
        assert_eq!(rig.ok(Op::Parent { rd }), Out::Rd(ROOT));
    }
    let log = rig.lookup(SYS, "log");
    let decls = rig.lookup(SYS, "decls");
    let level = rig.lookup(SYS, "loglevel");
    let limit = rig.lookup(SYS, "limit");
    let core = rig.lookup(SYS, "core");
    let info = rig.lookup(SYS, "info");
    // The fixed rows are made afresh at every boot, so they stand in
    // the order the table lists them and not in the order they grew.
    assert_eq!(
        rig.ok(Op::Children { rd: SYS }),
        Out::Rds(vec![decls, log, level, limit, core, info])
    );
    let limits = [
        "watch_ring",
        "pending_per_handle",
        "reply_window",
        "log_ring",
        "reentry",
    ];
    let cells: Vec<Rd> = limits.iter().map(|n| rig.lookup(limit, n)).collect();
    assert_eq!(rig.ok(Op::Children { rd: limit }), Out::Rds(cells));
    // The parameters are one order and the facts another: a fact is
    // named in `Fact::ALL` and `/sys/info` holds them as it lists them.
    let facts = ["mem", "time", "uptime", "version", "watchers"];
    let cells: Vec<Rd> = facts.iter().map(|n| rig.lookup(info, n)).collect();
    assert_eq!(rig.ok(Op::Children { rd: info }), Out::Rds(cells));
    assert_eq!(
        rig.err(Op::Lookup { rd: ROOT, name: "none".into() }),
        Error::Invalid
    );
    assert_eq!(rig.err(Op::Parent { rd: ROOT }), Error::Invalid);
}

#[test]
fn load_makes_home_and_start_registers_below_it() {
    let (rig, test) = fresh();
    let modules = rig.lookup(ROOT, "module");
    let home = rig.lookup(modules, "t");
    assert_eq!(rig.lookup(home, "cell"), test.cell());
    let info = rig.info(test.cell());
    assert_eq!(info.name.as_deref(), Some("cell"));
    assert_eq!(info.flags, Flags::READ | Flags::WRITE);
    assert_eq!(info.apis, vec![api()]);
    let again = rig.kernel.load("t", test, vec![]);
    assert_eq!(again.err(), Some(Error::Invalid));
}

#[test]
fn lookup_registers_the_child_inside_the_slot() {
    let (rig, test) = fresh();
    let dir = test.dir();
    let a = rig.lookup(dir, "a");
    assert_eq!(rig.ok(Op::Parent { rd: a }), Out::Rd(dir));
    assert_eq!(rig.lookup(dir, "a"), a, "cached; the slot is not asked");
    assert_eq!(test.runs(), 1);
    let listed = rig.ok(Op::Children { rd: dir });
    let x = rig.lookup(dir, "x");
    let y = rig.lookup(dir, "y");
    assert_eq!(listed, Out::Rds(vec![a, x, y]));
    assert_eq!(test.runs(), 2, "listed once");
    rig.ok(Op::Children { rd: dir });
    assert_eq!(test.runs(), 2);
}

#[test]
fn register_follows_the_parent_owner() {
    let (rig, test) = fresh();
    let other = Rc::new(Test::default());
    let driver: Rc<dyn Driver> = other.clone();
    let u = rig.kernel.load("u", other, vec![driver]).unwrap();
    let calls = rig.kernel.calls(u);
    let home_t = rig.lookup(rig.lookup(ROOT, "module"), "t");
    assert_eq!(calls.register(DriverId(0), home_t, None), Err(Error::Denied));
    assert_eq!(
        calls.register(DriverId(0), test.dir(), None),
        Err(Error::Denied)
    );
    assert_eq!(calls.register(DriverId(0), ROOT, None), Err(Error::Denied));
    let var = rig.lookup(ROOT, "var");
    let in_var = calls.register(DriverId(0), var, Some("v")).unwrap();
    assert_eq!(rig.ok(Op::Parent { rd: in_var }), Out::Rd(var));
    let session = rig.kernel.session_new();
    let in_session = calls.register(DriverId(0), session, Some("s")).unwrap();
    assert_eq!(rig.ok(Op::Parent { rd: in_session }), Out::Rd(session));
    assert_eq!(
        calls.register(DriverId(0), var, Some("v")),
        Err(Error::Invalid)
    );
    assert_eq!(
        calls.register(DriverId(0), var, Some("a/b")),
        Err(Error::Invalid)
    );
    assert_eq!(calls.register(DriverId(1), var, None), Err(Error::Invalid));
    assert_eq!(calls.unregister(test.cell()), Err(Error::Denied));
    assert_eq!(calls.name(test.cell(), None), Err(Error::Denied));
}

#[test]
fn declarations_are_stored_once_and_served() {
    let (rig, test) = fresh();
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
    rig.calls().report(test.cell(), Flags::READ, &[decl()]).unwrap();
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
        rig.calls().report(test.cell(), Flags::READ, &[wide]),
        Err(Error::Invalid)
    );
}

#[test]
fn open_is_capped_by_flags() {
    let (rig, test) = fresh();
    assert_eq!(
        rig.err(Op::Open { rd: test.dir(), rights: Rights::WRITE }),
        Error::Denied
    );
    assert_eq!(
        rig.err(Op::Open { rd: test.cell(), rights: Rights::empty() }),
        Error::Denied
    );
    assert_eq!(
        rig.err(Op::Open { rd: Rd(999), rights: Rights::READ }),
        Error::Invalid
    );
    let h = rig.open(test.cell(), Rights::READ);
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
    let (rig, test) = fresh();
    let h = rig.open(test.cell(), rw());
    let before = rig.info(test.cell()).atime;
    assert_eq!(
        rig.ok(Op::Write { h, at: 0, bytes: b"hello".to_vec() }),
        Out::Len(5)
    );
    assert_eq!(
        rig.ok(Op::Read { h, at: 1, len: 3 }),
        Out::Bytes(b"ell".to_vec())
    );
    assert!(rig.info(test.cell()).atime > before, "read stamps atime");
    assert_eq!(rig.ok(Op::Read { h, at: 5, len: 3 }), Out::Bytes(vec![]));
    rig.ok(Op::Close { h });
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 1 }), Error::Invalid);
    let cookie = test.cell().0 + 100;
    assert_eq!(test.inner.lock().unwrap().closed, vec![cookie]);
}

#[test]
fn call_is_checked_before_the_driver_runs() {
    let (rig, test) = fresh();
    let ro = rig.open(test.cell(), Rights::READ);
    let call =
        |h, api, method, input: Vec<u8>| Op::Call { h, api, method, input };
    assert_eq!(rig.err(call(ro, Hash(1), 0, vec![])), Error::Unsupported);
    assert_eq!(rig.err(call(ro, api(), 9, vec![])), Error::Unsupported);
    assert_eq!(rig.err(call(ro, api(), 1, blob(b"x"))), Error::Denied);
    let h = rig.open(test.cell(), rw());
    assert_eq!(rig.err(call(h, api(), 1, blob(&[0; 17]))), Error::Invalid);
    assert_eq!(rig.err(call(h, api(), 1, b"x".to_vec())), Error::Invalid);
    assert_eq!(rig.err(call(h, api(), 0, vec![1])), Error::Invalid);
    assert_eq!(test.runs(), 0, "nothing reached the Driver");
    assert_eq!(rig.ok(call(h, api(), 1, blob(b"hi"))), Out::Bytes(vec![]));
    assert_eq!(rig.ok(call(ro, api(), 0, vec![])), Out::Bytes(b"hi".to_vec()));
    assert_eq!(test.runs(), 2);
    let sys = rig.open(SYS, Rights::READ);
    assert_eq!(rig.err(call(sys, api(), 0, vec![])), Error::Unsupported);
}

#[test]
fn pending_is_finished_by_done_and_asked_once() {
    let (rig, test) = fresh();
    let h = rig.open(test.cell(), Rights::READ);
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
    let (rig, test) = fresh();
    test.inner.lock().unwrap().later = true;
    let h = rig.open(test.cell(), Rights::READ);
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
    let (rig, test) =
        limited(Limits { pending_per_handle: 1, ..Limits::default() });
    let h = rig.open(test.cell(), Rights::READ);
    let op = || Op::Call { h, api: api(), method: 2, input: vec![] };
    assert!(matches!(rig.submit(op()), Reply::Pending(_)));
    assert_eq!(rig.err(op()), Error::Busy);
    rig.calls().done(Token(1), Ok(Out::Unit)).unwrap();
    assert!(matches!(rig.submit(op()), Reply::Pending(_)));
}

#[test]
fn a_repeated_sequence_is_answered_from_memory() {
    let (rig, test) = limited(Limits { reply_window: 2, ..Limits::default() });
    let h = rig.open(test.cell(), rw());
    let write = Op::Write { h, at: 0, bytes: b"once".to_vec() };
    let first = rig.kernel.submit(rig.caller, 10, write.clone());
    assert_eq!(first, Reply::ok(Out::Len(4)));
    assert_eq!(rig.kernel.submit(rig.caller, 10, write.clone()), first);
    assert_eq!(test.runs(), 1, "the slot ran once");
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
    assert_eq!(test.runs(), 1);
}

#[test]
fn watch_modify_and_attrib_come_from_the_driver() {
    let (rig, test) = fresh();
    let cell = test.cell();
    let h = rig.open(cell, Rights::READ);
    let (_, cursor) = rig.since(0);
    rig.ok(Op::Watch { h, on: true });
    assert_eq!(test.inner.lock().unwrap().hints, vec![(cell, true)]);
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
fn a_close_tells_the_driver_of_the_unwatch_it_implies() {
    let (rig, test) = fresh();
    let cell = test.cell();
    let hints = || test.inner.lock().unwrap().hints.clone();
    let h = rig.open(cell, Rights::READ);
    rig.ok(Op::Watch { h, on: true });
    assert_eq!(hints(), vec![(cell, true)]);
    // `Op::Close`'s own doc says it implies unwatch, and the Driver is
    // told of this one as it is told of every other, so a Driver that
    // counts what it is told reaches zero whichever way a watcher
    // goes.
    rig.ok(Op::Close { h });
    assert_eq!(hints(), vec![(cell, true), (cell, false)]);

    // A handle that was not watching says nothing; a second watch on
    // one rd is a second hint, as two callers' would be.
    let quiet = rig.open(cell, Rights::READ);
    rig.ok(Op::Close { h: quiet });
    let both = [rig.open(cell, Rights::READ), rig.open(cell, Rights::READ)];
    for h in both {
        rig.ok(Op::Watch { h, on: true });
    }
    for h in both {
        rig.ok(Op::Close { h });
    }
    let on = (cell, true);
    let off = (cell, false);
    assert_eq!(hints(), vec![on, off, on, on, off, off]);
}

#[test]
fn watch_tree_kinds_come_from_the_kernel() {
    let (rig, test) = fresh();
    let dir = test.dir();
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
    let (rig, test) = fresh();
    let dir = test.dir();
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
    let (rig, test) = limited(Limits { watch_ring: 2, ..Limits::default() });
    let cell = test.cell();
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
fn a_cursor_past_the_head_is_no_overflow() {
    let (rig, test) = limited(Limits { watch_ring: 2, ..Limits::default() });
    let cell = test.cell();
    let h = rig.open(cell, Rights::READ);
    rig.ok(Op::Watch { h, on: true });
    for _ in 0..5 {
        rig.calls().changed(cell, Kind::Modify).unwrap();
    }
    // The cursor is the caller's own number and `u64::MAX` is one it
    // may send: at or past the head it is behind nothing, so it is no
    // slow watcher and there is nothing left for it to hear.
    assert_eq!(rig.kinds(u64::MAX), vec![]);
}

#[test]
fn close_implies_unwatch() {
    let (rig, test) = fresh();
    let cell = test.cell();
    let h = rig.open(cell, Rights::READ);
    rig.ok(Op::Watch { h, on: true });
    let (_, cursor) = rig.since(0);
    rig.ok(Op::Close { h });
    rig.calls().changed(cell, Kind::Modify).unwrap();
    assert_eq!(rig.kinds(cursor), vec![]);
}

#[test]
fn the_log_holds_the_call_pair_and_module_lines() {
    let (rig, test) = fresh();
    // The pair is measurement, not news: it is kept at `debug`.
    let level = rig.lookup(SYS, "loglevel");
    let hd = rig.open(level, Rights::WRITE);
    rig.ok(Op::Write { h: hd, at: 0, bytes: b"debug".to_vec() });
    let h = rig.open(test.cell(), Rights::READ);
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
    let said = String::from_utf8_lossy(&bytes);
    assert!(said.contains("method=0 dispatch"), "{said}");
    assert!(said.contains("method=0 reply"), "{said}");
    assert!(said.contains("kernel"), "{said}");
    assert!(
        said.contains("hello") && said.contains("\u{1}\0\0\0t"),
        "mark is the Module name"
    );
    assert_eq!(rig.ok(Op::Read { h, at: 0, len: 4 }), Out::Bytes(vec![]));
}

/// A Module whose Driver goes back through the Kernel for its own
/// work: the `read` of `loop` reads `loop`, and the listing of `dir`
/// asks for that listing. Both are the shapes a Module reaches by
/// accident when it opens a Resource it happens to serve itself.
#[derive(Default)]
struct Mirror {
    /// The cell whose `read` serves itself.
    own: Cell<Option<Rd>>,
    /// The cell that answers bytes without going anywhere.
    plain: Cell<Option<Rd>>,
    dir: Cell<Option<Rd>>,
    /// Driver `read` runs, the nested ones counted too.
    reads: Cell<usize>,
    /// Driver `lookup` runs.
    lookups: Cell<usize>,
    /// What the `Children` from inside the listing answered.
    inside: RefCell<Vec<Rd>>,
}

impl Module for Mirror {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let cell = |name| -> Result<Rd, Error> {
            let rd = ctx.register(DriverId(0), home, Some(name))?;
            ctx.report(rd, Flags::READ, &[])?;
            Ok(rd)
        };
        self.own.set(Some(cell("loop")?));
        self.plain.set(Some(cell("plain")?));
        let dir = ctx.register(DriverId(0), home, Some("dir"))?;
        ctx.report(dir, Flags::READ | Flags::CONTAINER, &[])?;
        self.dir.set(Some(dir));
        Ok(())
    }
}

impl Driver for Mirror {
    fn lookup(&self, ctx: &dyn Calls, rd: Rd, name: &str) -> Reply<Rd> {
        self.lookups.set(self.lookups.get() + 1);
        if name.is_empty() {
            *self.inside.borrow_mut() = listing(ctx, rd);
            return Reply::Ready(child(ctx, rd, "x"));
        }
        Reply::Ready(child(ctx, rd, name))
    }

    /// The cookie is the rd, so `read` knows which of the two it is on.
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
        self.reads.set(self.reads.get() + 1);
        if self.own.get() != Some(Rd(h)) {
            return Reply::ok(b"plain".to_vec());
        }
        again(ctx, Rd(h), at, len)
    }

    fn close(&self, _: &dyn Calls, _: u64) -> Reply<()> {
        Reply::ok(())
    }
}

/// The children of `rd` as the Kernel has them, asked for from inside
/// the listing of `rd` itself.
fn listing(ctx: &dyn Calls, rd: Rd) -> Vec<Rd> {
    match ctx.submit(Op::Children { rd }) {
        Reply::Ready(Ok(Out::Rds(rds))) => rds,
        other => panic!("children from inside the listing: {other:?}"),
    }
}

/// The read a Driver makes of the rd it is being read for: the chain
/// the Kernel has to end, and what it ended with.
fn again(ctx: &dyn Calls, rd: Rd, at: u64, len: usize) -> Reply<Vec<u8>> {
    let h = match ctx.open(rd, Rights::READ) {
        Ok(h) => h,
        Err(error) => return Reply::err(error),
    };
    let len = u32::try_from(len).unwrap_or(u32::MAX);
    let inner = ctx.submit(Op::Read { h, at, len });
    let _ = ctx.submit(Op::Close { h });
    match inner {
        Reply::Ready(Ok(Out::Bytes(bytes))) => Reply::ok(bytes),
        Reply::Ready(Err(error)) => Reply::err(error),
        _ => Reply::err(Error::Unsupported),
    }
}

/// The Mirror Module loaded beside the Test one, with its rds.
fn mirror(rig: &Rig) -> Rc<Mirror> {
    let mirror = Rc::new(Mirror::default());
    let driver: Rc<dyn Driver> = mirror.clone();
    rig.kernel.load("m", mirror.clone(), vec![driver]).unwrap();
    mirror
}

/// The `(level, text)` of everything `/sys/log` holds now, read by
/// the rig's own caller.
fn log(rig: &Rig) -> Vec<(u8, String)> {
    let h = rig.open(rig.lookup(SYS, "log"), Rights::READ);
    let Out::Bytes(bytes) = rig.ok(Op::Read { h, at: 1, len: 1 << 20 }) else {
        panic!("log")
    };
    rig.ok(Op::Close { h });
    entries(&bytes).into_iter().map(|(_, level, t)| (level, t)).collect()
}

#[test]
fn a_driver_serving_itself_is_refused_not_hung() {
    let rig = fresh().0;
    let mirror = mirror(&rig);
    let deep = usize::from(Limits::default().reentry);
    let h = rig.open(mirror.own.get().unwrap(), Rights::READ);

    // The chain ends where the Kernel says it does, and the answer
    // is an ordinary error on the outermost read.
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 4 }), Error::Busy);
    assert_eq!(mirror.reads.get(), deep, "one read per depth, then Busy");
    let said = log(&rig);
    let refusals: Vec<&(u8, String)> =
        said.iter().filter(|(_, text)| text.starts_with("reentry ")).collect();
    assert_eq!(refusals.len(), 1, "one line per refusal: {said:?}");
    assert_eq!(refusals[0].0, 4, "at warn");
    assert!(refusals[0].1.ends_with(&format!("depth={deep}")), "{said:?}");

    // The counter came back down with the calls: the same read is
    // refused the same way, and an ordinary read is served.
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 4 }), Error::Busy);
    assert_eq!(mirror.reads.get(), 2 * deep);
    let lines =
        log(&rig).into_iter().filter(|(_, t)| t.starts_with("reentry "));
    assert_eq!(lines.count(), 2);
    let plain = rig.open(mirror.plain.get().unwrap(), Rights::READ);
    let read = Op::Read { h: plain, at: 0, len: 8 };
    assert_eq!(rig.ok(read), Out::Bytes(b"plain".to_vec()));
}

/// The cell `/sys/limit/<name>` written `value` by a caller of its own.
fn limit(rig: &Rig, name: &str, value: &str) -> Reply<Out> {
    let rd = rig.lookup(rig.lookup(SYS, "limit"), name);
    let h = rig.open(rd, Rights::WRITE);
    rig.submit(Op::Write { h, at: 0, bytes: value.as_bytes().to_vec() })
}

#[test]
fn a_limit_written_holds_from_the_next_op_on() {
    let (rig, test) = fresh();
    let mirror = mirror(&rig);
    let deep = usize::from(Limits::default().reentry);
    let h = rig.open(mirror.own.get().unwrap(), Rights::READ);

    // The chain the default `reentry` bounds, and the same chain two
    // steps longer once the parameter says so: a limit is applied
    // where it is read, so the very next op stands under the new one.
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 4 }), Error::Busy);
    assert_eq!(mirror.reads.get(), deep);
    assert!(matches!(limit(&rig, "reentry", "10"), Reply::Ready(Ok(_))));
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 4 }), Error::Busy);
    assert_eq!(mirror.reads.get(), deep + 10, "ten more before Busy");

    // And a Pending cap of one: the second wait on a handle is `Busy`
    // where sixteen were before, with no Kernel made anew.
    assert!(matches!(
        limit(&rig, "pending_per_handle", "1"),
        Reply::Ready(Ok(_))
    ));
    let cell = rig.open(test.cell(), Rights::READ | Rights::WRITE);
    let slow = || Op::Call { h: cell, api: api(), method: 2, input: vec![] };
    assert!(matches!(rig.submit(slow()), Reply::Pending(_)));
    assert_eq!(rig.err(slow()), Error::Busy, "one wait is the cap now");
}

#[test]
fn listing_from_inside_the_listing_sees_the_cache() {
    let rig = fresh().0;
    let mirror = mirror(&rig);
    let dir = mirror.dir.get().unwrap();
    let a = rig.lookup(dir, "a");

    // The listing is marked before the Driver runs, so the Children
    // the Driver submits from inside it answers the cache as it
    // stands and the Driver is not asked a second time.
    let listed = rig.ok(Op::Children { rd: dir });
    let x = rig.lookup(dir, "x");
    assert_eq!(listed, Out::Rds(vec![a, x]));
    assert_eq!(*mirror.inside.borrow(), vec![a], "the cache as it stood");
    assert_eq!(mirror.lookups.get(), 2, "the name, then the listing");
}
