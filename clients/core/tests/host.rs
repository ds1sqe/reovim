//! The Host over a Kernel: one `&dyn Calls` on the Client's own tree
//! under `/self` and on the server's, with nothing but the id telling
//! the two apart.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use reovim_client::{Carrier, Client, Fault, Host, LOCAL_BASE, ROOT, SELF};
use reovim_host::clock;
use reovim_kernel_interface::wire::{Message, decode};
use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Hash, Kind, Method,
    Module, ModuleId, Op, Out, Rd, Reply, Rights, Token, Ty,
};
use reovim_kernel_runtime::{Connection, Heap, Kernel, Limits, NoLoader};
use reovim_sdk::{cell, ops};

/// `/module/slow/late`: the first `read` is Pending until someone looks
/// up `finish` on it; then it reads `late`.
#[derive(Default)]
struct Slow {
    late: Cell<Option<Rd>>,
    done: Cell<bool>,
}

impl Module for Slow {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let rd = ctx.register(DriverId(0), home, Some("late"))?;
        ctx.report(rd, Flags::READ, &[])?;
        self.late.set(Some(rd));
        Ok(())
    }
}

impl Driver for Slow {
    fn lookup(&self, ctx: &dyn Calls, _: Rd, name: &str) -> Reply<Rd> {
        if name == "finish" {
            self.done.set(true);
            let out = Out::Bytes(b"late".to_vec());
            ctx.done(Token(1), Ok(out)).unwrap();
        }
        Reply::err(Error::Invalid)
    }

    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(&self, _: &dyn Calls, _: u64, at: u64, _: usize) -> Reply<Vec<u8>> {
        if !self.done.get() {
            return Reply::Pending(Token(1));
        }
        let bytes = b"late".get(usize::try_from(at).unwrap()..);
        Reply::ok(bytes.unwrap_or_default().to_vec())
    }
}

/// The Carrier: one `Connection` on a shared Kernel. On a Busy `Ask` it
/// finishes the slow read from a second caller, as a Driver would in
/// its own time.
struct Local {
    kernel: Rc<Kernel>,
    connection: Connection,
    slow: Rc<Slow>,
    asks: Rc<Cell<u32>>,
    sent: Rc<Cell<u32>>,
    /// The bytes out and the bytes back, which `Link` is held against.
    moved: Rc<Cell<(u64, u64)>>,
}

impl Carrier for Local {
    fn exchange(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Fault> {
        self.sent.set(self.sent.get() + 1);
        let answer = self
            .connection
            .handle(&self.kernel, bytes, &mut NoLoader)
            .ok_or(Fault::Wire)?;
        let asking = matches!(
            decode(bytes),
            Some((_, Message::Request(Op::Ask { .. })))
        );
        let busy = matches!(
            decode(&answer),
            Some((_, Message::Reply(Reply::Ready(Err(Error::Busy)))))
        );
        if asking {
            self.asks.set(self.asks.get() + 1);
        }
        if asking && busy {
            let late = self.slow.late.get().unwrap();
            let poke = Op::Lookup { rd: late, name: "finish".to_string() };
            self.kernel.submit(self.kernel.caller_new(), 1, poke);
        }
        let (out, back) = self.moved.get();
        self.moved.set((out + bytes.len() as u64, back + answer.len() as u64));
        Ok(answer)
    }
}

/// One Kernel with text and the slow Driver, the Clients on it, and
/// what each of them has put on the link.
struct Rig {
    kernel: Rc<Kernel>,
    slow: Rc<Slow>,
    asks: Rc<Cell<u32>>,
    sent: Rc<Cell<u32>>,
    moved: Rc<Cell<(u64, u64)>>,
}

impl Rig {
    fn new() -> Self {
        let kernel = Kernel::new(clock::now, Heap::default, Limits::default());
        let text = reovim_module_text::boot("");
        kernel.load("text", text.module, text.drivers).unwrap();
        let slow = Rc::new(Slow::default());
        let booted =
            Booted { module: slow.clone(), drivers: vec![slow.clone()] };
        kernel.load("slow", booted.module, booted.drivers).unwrap();
        let asks = Rc::new(Cell::new(0));
        let sent = Rc::new(Cell::new(0));
        let moved = Rc::new(Cell::new((0, 0)));
        Self { kernel: Rc::new(kernel), slow, asks, sent, moved }
    }

    fn client(&self) -> Client {
        let carrier = Local {
            kernel: self.kernel.clone(),
            connection: Connection::new(),
            slow: self.slow.clone(),
            asks: self.asks.clone(),
            sent: self.sent.clone(),
            moved: self.moved.clone(),
        };
        Client::connect(Box::new(carrier), Vec::new(), clock::now).unwrap()
    }
}

/// `/self/x`: bytes with one declared method beside its write, and the
/// one child it registers when it is looked up.
struct Store {
    note: Cell<Option<Rd>>,
    bytes: RefCell<Vec<u8>>,
}

impl Driver for Store {
    fn lookup(&self, ctx: &dyn Calls, rd: Rd, name: &str) -> Reply<Rd> {
        // An empty name lists, and this Container has the one child,
        // registered here and once.
        if !name.is_empty() && name != "note" {
            return Reply::err(Error::Invalid);
        }
        if let Some(note) = self.note.get() {
            return Reply::ok(note);
        }
        let note = ctx.register(DriverId(0), rd, Some("note")).unwrap();
        ctx.report(note, Flags::READ, &[]).unwrap();
        self.note.set(Some(note));
        Reply::ok(note)
    }

    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        _: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        let held = self.bytes.borrow();
        let bytes: &[u8] =
            if self.note.get() == Some(Rd(h)) { b"note\n" } else { &held };
        let from = usize::try_from(at).unwrap().min(bytes.len());
        let to = from.saturating_add(len).min(bytes.len());
        Reply::ok(bytes[from..to].to_vec())
    }

    fn write(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        buf: &[u8],
    ) -> Reply<usize> {
        // The borrow is gone before the Host is asked anything.
        let mut bytes = self.bytes.borrow_mut();
        let at = usize::try_from(at).unwrap().min(bytes.len());
        bytes.truncate(at);
        bytes.extend_from_slice(buf);
        drop(bytes);
        ctx.changed(Rd(h), Kind::Modify).unwrap();
        Reply::ok(buf.len())
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        _: Hash,
        _: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        // The declared `utf8(64)`: its length, then its bytes.
        *self.bytes.borrow_mut() = input[4..].to_vec();
        ctx.changed(Rd(h), Kind::Modify).unwrap();
        Reply::ok(Vec::new())
    }
}

/// `/self/deep`: its read submits into itself, and its write cannot
/// answer now.
struct Deep;

impl Driver for Deep {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        ctx: &dyn Calls,
        h: u64,
        _: u64,
        _: usize,
    ) -> Reply<Vec<u8>> {
        ops::bytes(ctx, Rd(h)).into()
    }

    fn write(&self, _: &dyn Calls, _: u64, _: u64, _: &[u8]) -> Reply<usize> {
        Reply::Pending(Token(1))
    }
}

/// A Module that serves nothing of its own: what `/self/x` and
/// `/self/deep` are registered through is the test's own `Calls`.
struct Nothing;

impl Module for Nothing {
    fn start(&self, _: &dyn Calls, _: Rd) -> Result<(), Error> {
        Ok(())
    }
}

/// The api `/self/x` offers.
fn decl() -> Decl {
    let put = Method {
        number: 0,
        name: "put".to_string(),
        input_name: Some("input".into()),
        input: Ty::Utf8(64),
        output: Ty::Unit,
        right: Rights::WRITE,
    };
    Decl {
        api: "store".to_string(),
        ver: 1,
        methods: vec![put],
        records: Vec::new(),
    }
}

/// A Host over a Client of `rig`, with the test's two Drivers loaded
/// and `/self/x` and `/self/deep` registered; its `ModuleId` with it.
fn over(rig: &Rig) -> (Host, ModuleId) {
    let host = Host::over(rig.client(), clock::now);
    let store = Rc::new(Store {
        note: Cell::new(None),
        bytes: RefCell::new(b"local\n".to_vec()),
    });
    let drivers: Vec<Rc<dyn Driver>> = vec![store, Rc::new(Deep)];
    let module = host.load("test", Rc::new(Nothing), drivers).unwrap();
    let calls = host.calls(module);
    let rw = Flags::READ.union(Flags::WRITE);
    let x = calls.register(DriverId(0), SELF, Some("x")).unwrap();
    calls.report(x, rw.union(Flags::CONTAINER), &[decl()]).unwrap();
    let deep = calls.register(DriverId(1), SELF, Some("deep")).unwrap();
    calls.report(deep, rw, &[]).unwrap();
    (host, module)
}

/// A Buffer of the server's, over nothing: `/module/text/open new()`.
fn buffer(ctx: &dyn Calls) -> Rd {
    let open = cell::resolve(ctx, ROOT, "/module/text/open").unwrap();
    let api = ops::info(ctx, open).unwrap().apis[0];
    let h = ctx.open(open, Rights::WRITE).unwrap();
    let call = Op::Call { h, api, method: 1, input: Vec::new() };
    let out = ops::ready(ctx.submit(call)).unwrap();
    ops::shut(ctx, &[h]);
    let Out::Bytes(bytes) = out else { panic!("{out:?}") };
    reovim_sdk::decode(&bytes).unwrap()
}

/// One `put` argument in the declared encoding.
fn put(text: &str) -> Vec<u8> {
    let mut input = u32::try_from(text.len()).unwrap().to_le_bytes().to_vec();
    input.extend_from_slice(text.as_bytes());
    input
}

/// Everything `rd` gained since `at`, read the way a run reads.
fn wrote(ctx: &dyn Calls, rd: Rd, bytes: &[u8]) {
    let h = ctx.open(rd, Rights::WRITE).unwrap();
    let write = Op::Write { h, at: 0, bytes: bytes.to_vec() };
    ops::ready(ctx.submit(write)).unwrap();
    ops::shut(ctx, &[h]);
}

#[test]
fn one_calls_reaches_the_client_tree_and_the_server_tree() {
    let rig = Rig::new();
    let (host, _) = over(&rig);
    let calls = host.own();
    let ctx: &dyn Calls = &calls;

    let x = cell::resolve(ctx, ROOT, "/self/x").unwrap();
    assert!(x.0 >= LOCAL_BASE);
    assert_eq!(ops::where_is(ctx, x).unwrap(), "/self/x");
    let text = cell::resolve(ctx, ROOT, "/module/text").unwrap();
    assert!(text.0 < LOCAL_BASE);
    assert_eq!(ops::where_is(ctx, text).unwrap(), "/module/text");

    // `/self` is the root's child, once, and the root is its parent.
    // The merge is one message, the server's own listing.
    let sent = rig.sent.get();
    let children = ops::children(ctx, ROOT).unwrap();
    assert_eq!(rig.sent.get(), sent + 1);
    assert_eq!(children.iter().filter(|rd| **rd == SELF).count(), 1);
    assert_eq!(cell::resolve(ctx, SELF, "..").unwrap(), ROOT);
    assert_eq!(ops::info(ctx, SELF).unwrap().name.unwrap(), "self");

    // A local Container lists through its Driver's own lookup slot.
    let note = cell::resolve(ctx, x, "note").unwrap();
    assert_eq!(ops::children(ctx, x).unwrap(), [note]);
    assert_eq!(ops::where_is(ctx, note).unwrap(), "/self/x/note");

    // The link is still there for what `Calls` cannot spell, and the
    // Client's own rd is in the same tree.
    let cid = host.wire().cid();
    let at = ops::where_is(ctx, cid).unwrap();
    assert!(at.starts_with("/session/"), "{at}");
}

#[test]
fn a_handle_reaches_a_local_driver_and_the_wire_the_same_way() {
    let rig = Rig::new();
    let (host, _) = over(&rig);
    let calls = host.own();
    let ctx: &dyn Calls = &calls;

    // The Client's own cell: read, write, and the declared method,
    // none of it over the link.
    let x = cell::resolve(ctx, ROOT, "/self/x").unwrap();
    let sent = rig.sent.get();
    let link = host.wire().link();
    assert_eq!(ops::bytes(ctx, x).unwrap(), b"local\n");
    let h = ctx.open(x, Rights::WRITE).unwrap();
    assert!(h.0 >= LOCAL_BASE);
    let write = Op::Write { h, at: 0, bytes: b"written\n".to_vec() };
    assert_eq!(ops::ready(ctx.submit(write)).unwrap(), Out::Len(8));
    let api = ops::info(ctx, x).unwrap().apis[0];
    let call = Op::Call { h, api, method: 0, input: put("called\n") };
    assert_eq!(ops::ready(ctx.submit(call)).unwrap(), Out::Bytes(Vec::new()));
    ops::shut(ctx, &[h]);
    assert_eq!(ops::bytes(ctx, x).unwrap(), b"called\n");
    assert_eq!(rig.sent.get(), sent, "a local op reaches no link");
    // So no counter of the link moves for one, while what it has
    // carried is what the carrier moved: the `Connect` and each op
    // `over` made, and the clock in the order it runs.
    assert_eq!(host.wire().link(), link, "a local op moved a counter");
    assert_eq!(link.messages, u64::from(sent));
    assert_eq!((link.bytes_out, link.bytes_in), rig.moved.get());
    assert!(link.worst_us >= link.last_us, "{link:?}");

    // What the rd offers is what may be asked of it, on either side.
    assert_eq!(ctx.open(x, Rights::empty()), Err(Error::Denied));
    let read = ctx.open(x, Rights::READ).unwrap();
    let call = Op::Call { h: read, api, method: 0, input: put("no") };
    assert_eq!(ops::ready(ctx.submit(call)), Err(Error::Denied));
    let call = Op::Call { h: read, api, method: 7, input: put("no") };
    assert_eq!(ops::ready(ctx.submit(call)), Err(Error::Unsupported));
    ops::shut(ctx, &[read]);

    // A Buffer of the server's, through the same `&dyn Calls`.
    let buffer = buffer(ctx);
    assert!(buffer.0 < LOCAL_BASE);
    wrote(ctx, buffer, b"server\n");
    assert_eq!(ops::bytes(ctx, buffer).unwrap(), b"server\n");
    // Those did reach it, and the two counts agree again.
    let after = host.wire().link();
    assert!(after.messages > link.messages, "the Buffer took no message");
    assert_eq!(after.messages, u64::from(rig.sent.get()));
    assert_eq!((after.bytes_out, after.bytes_in), rig.moved.get());
}

#[test]
fn a_local_pending_is_invalid_and_a_chain_ends_at_the_cap() {
    let rig = Rig::new();
    let (host, _) = over(&rig);
    let calls = host.own();
    let ctx: &dyn Calls = &calls;
    let deep = cell::resolve(ctx, ROOT, "/self/deep").unwrap();

    // Nothing is pending on one stack, so there is no token either.
    let h = ctx.open(deep, Rights::WRITE).unwrap();
    let write = Op::Write { h, at: 0, bytes: b"x".to_vec() };
    assert_eq!(ops::ready(ctx.submit(write)), Err(Error::Invalid));
    assert_eq!(ctx.done(Token(1), Ok(Out::Unit)), Err(Error::Invalid));
    ops::shut(ctx, &[h]);

    // Its read submits into itself: the chain ends at an error and not
    // at the stack.
    assert_eq!(ops::bytes(ctx, deep), Err(Error::Busy));
    // And the count came back down with it.
    assert_eq!(
        ops::bytes(ctx, cell::resolve(ctx, ROOT, "/self/x").unwrap()),
        Ok(b"local\n".to_vec())
    );
}

#[test]
fn an_unregistered_rd_is_invalid_and_its_handle_gone() {
    let rig = Rig::new();
    let (host, module) = over(&rig);
    let calls = host.own();
    let ctx: &dyn Calls = &calls;
    let x = cell::resolve(ctx, ROOT, "/self/x").unwrap();
    let h = ctx.open(x, Rights::READ).unwrap();

    host.calls(module).unregister(x).unwrap();
    assert_eq!(cell::resolve(ctx, ROOT, "/self/x"), Err(Error::Invalid));
    let read = Op::Read { h, at: 0, len: 16 };
    assert_eq!(ops::ready(ctx.submit(read)), Err(Error::Gone));

    // A Client registers nothing at the server.
    let text = cell::resolve(ctx, ROOT, "/module/text").unwrap();
    let mine = host.calls(module);
    assert_eq!(mine.register(DriverId(0), text, Some("y")), Err(Error::Denied));
    assert_eq!(mine.unregister(text), Err(Error::Denied));
}

#[test]
fn a_name_a_listed_container_does_not_hold_is_invalid() {
    let rig = Rig::new();
    let (host, module) = over(&rig);
    let calls = host.own();
    let ctx: &dyn Calls = &calls;
    let mine = host.calls(module);

    // A Container served by a Driver with no `lookup` of its own
    // (`Deep`): what it holds is what it registered, and `children`
    // answers from the table, the Driver having nothing to add.
    let dir = Flags::READ.union(Flags::CONTAINER);
    let bag = mine.register(DriverId(1), SELF, Some("bag")).unwrap();
    mine.report(bag, dir, &[]).unwrap();
    let one = mine.register(DriverId(1), bag, Some("one")).unwrap();
    mine.report(one, Flags::READ, &[]).unwrap();
    assert_eq!(ops::children(ctx, bag), Ok(vec![one]));
    assert_eq!(cell::resolve(ctx, bag, "one"), Ok(one));

    // So a name it does not hold is not there, which is what the
    // Kernel says of a name that is not under a row of its own, and
    // not something this Client cannot do.
    assert_eq!(cell::resolve(ctx, bag, "nope"), Err(Error::Invalid));

    // A row that is no Container holds no names at all: nothing can
    // look one up under it, which is what `Unsupported` says.
    let deep = cell::resolve(ctx, ROOT, "/self/deep").unwrap();
    assert_eq!(cell::resolve(ctx, deep, "nope"), Err(Error::Unsupported));

    // A Driver with a `lookup` of its own answers for itself, as ever.
    let x = cell::resolve(ctx, ROOT, "/self/x").unwrap();
    assert_eq!(cell::resolve(ctx, x, "nope"), Err(Error::Invalid));
    assert_eq!(cell::resolve(ctx, ROOT, "/self/nope"), Err(Error::Invalid));
}

#[test]
fn one_cursor_and_one_sys_decls_over_both_sides() {
    let rig = Rig::new();
    let (host, _) = over(&rig);
    let calls = host.own();
    let ctx: &dyn Calls = &calls;
    let x = cell::resolve(ctx, ROOT, "/self/x").unwrap();
    let buffer = buffer(ctx);

    // The declaration of a local Driver and of a Module, both under
    // `/sys/decls` as far as the reader can tell.
    let mine = ops::info(ctx, x).unwrap().apis[0];
    assert_eq!(ops::decl(ctx, mine).unwrap(), decl());
    let at = format!("/sys/decls/{:016x}", mine.0);
    let cell = cell::resolve(ctx, ROOT, &at).unwrap();
    assert_eq!(ops::where_is(ctx, cell).unwrap(), at);
    let theirs = ops::info(ctx, buffer).unwrap().apis[0];
    assert_eq!(ops::decl(ctx, theirs).unwrap().api, "text");

    // One ring and one cursor: a local change and a server change come
    // back from one `since` in one numbered order.
    let watched = ops::watch_all(ctx, &[x, buffer]).unwrap();
    let (_, cursor) = ops::since(ctx, 0).unwrap();
    wrote(ctx, x, b"one");
    wrote(ctx, buffer, b"two");
    let (events, next) = ops::since(ctx, cursor).unwrap();
    let seen: Vec<(Rd, Kind)> = events.iter().map(|e| (e.rd, e.kind)).collect();
    assert_eq!(seen, [(x, Kind::Modify), (buffer, Kind::Modify)]);
    assert!(events[0].seq < events[1].seq, "{events:?}");
    assert_eq!(next, events[1].seq);
    assert!(ops::since(ctx, next).unwrap().0.is_empty(), "the cursor moved");
    ops::shut(ctx, &watched);
}

#[test]
fn a_cursor_at_the_head_of_the_ring_has_missed_nothing() {
    let rig = Rig::new();
    let (host, _) = over(&rig);
    let calls = host.own();
    let ctx: &dyn Calls = &calls;

    // A cursor the ring no longer covers is one `Overflow`, and the
    // one number no event can stand past is no such cursor: it is the
    // head of the ring, and what it has missed is nothing. The wire
    // carries a u64, so the number arrives as any other would.
    let (events, cursor) = ops::since(ctx, u64::MAX).unwrap();
    assert_eq!(events, Vec::new());
    assert!(cursor < u64::MAX, "the head is a seq the ring gave out");
}

#[test]
fn a_wire_reply_that_is_not_ready_comes_back_ready() {
    let rig = Rig::new();
    let (host, _) = over(&rig);
    let calls = host.own();
    let ctx: &dyn Calls = &calls;
    let late = cell::resolve(ctx, ROOT, "/module/slow/late").unwrap();
    assert_eq!(ops::bytes(ctx, late).unwrap(), b"late");
    assert!(rig.asks.get() >= 2, "asked {} times", rig.asks.get());
}

struct Short;

impl Driver for Short {
    fn open(&self, _: &dyn Calls, _: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(0)
    }

    fn call(
        &self,
        _: &dyn Calls,
        _: u64,
        _: Hash,
        _: u16,
        _: &[u8],
    ) -> Reply<Vec<u8>> {
        Reply::ok(vec![1, 2, 3])
    }
}

#[test]
fn a_local_reply_must_fit_the_declared_output() {
    let rig = Rig::new();
    let host = Host::over(rig.client(), clock::now);
    let module =
        host.load("short", Rc::new(Nothing), vec![Rc::new(Short)]).unwrap();
    let calls = host.calls(module);
    let rd = calls.register(DriverId(0), SELF, Some("value")).unwrap();
    let mut declared = decl();
    declared.methods[0].output = Ty::U64;
    calls.report(rd, Flags::WRITE, &[declared.clone()]).unwrap();
    let h = calls.open(rd, Rights::WRITE).unwrap();
    let call =
        Op::Call { h, api: declared.hash(), method: 0, input: put("changed") };
    assert_eq!(calls.submit(call), Reply::err(Error::Invalid));
}
