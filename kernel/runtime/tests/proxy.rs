//! Two links on one Kernel, where the Client on one serves what the
//! remote on the other asks of `/session/<id>/client/<cid>/self`: the
//! rows the mirror makes, what costs a message and what does not, the
//! bound on a Client that answers nothing, and the link's end.

mod common;

use std::collections::BTreeMap;

use common::tick;
use reovim_kernel_interface::wire::{Message, decode, encode};
use reovim_kernel_interface::{
    Error, Event, Flags, Handle, Info, Kind, Op, Out, Outcome, Rd, Reply,
    Rights, Token,
};
use reovim_kernel_runtime::{Connection, Heap, Kernel, Limits, NoLoader, ROOT};

/// The Client's own rd space, above every id the server hands out.
const BASE: u64 = 1 << 63;
const SELF: Rd = Rd(BASE);
const DEV: Rd = Rd(BASE + 1);
const CWD: Rd = Rd(BASE + 2);
const MODE: Rd = Rd(BASE + 3);

/// One node of the Client's own tree.
struct Node {
    name: &'static str,
    flags: Flags,
    children: Vec<Rd>,
    /// What a whole read of it answers.
    bytes: &'static [u8],
}

/// `/self` with one Container and two cells: the smallest tree with a
/// name to walk to, a listing to take and bytes to read.
fn node(rd: Rd) -> Result<Node, Error> {
    let dir = Flags::READ.union(Flags::CONTAINER);
    let cell = Flags::READ;
    let (name, flags, children, bytes): (_, _, Vec<Rd>, &[u8]) = match rd {
        SELF => ("self", dir, vec![DEV, MODE], b""),
        DEV => ("dev", dir, vec![CWD], b""),
        CWD => ("cwd", cell, Vec::new(), b"/tmp"),
        MODE => ("mode", cell, Vec::new(), b"normal"),
        _ => return Err(Error::Invalid),
    };
    Ok(Node { name, flags, children, bytes })
}

/// One end of a link: its `Connection`, the sequence it counts and its
/// own place in the Kernel's event ring.
#[derive(Default)]
struct Side {
    link: Connection,
    seq: u64,
    cursor: u64,
}

impl Side {
    /// One `Op::Since` on this side's own cursor: the events on the
    /// rds it watches, and the ops the server asks it to serve.
    fn since(&mut self, kernel: &Kernel) -> (Vec<Event>, Vec<(Token, Op)>) {
        let op = Op::Since { cursor: self.cursor };
        match self.request(kernel, op) {
            Reply::Ready(Ok(Out::Events { events, cursor, serve })) => {
                self.cursor = cursor;
                (events, serve)
            }
            other => panic!("not events: {other:?}"),
        }
    }

    /// `Connect` on a Session of its own; the client rd it was given.
    fn connect(&mut self, kernel: &Kernel) -> Rd {
        let connect =
            Message::Connect { credential: Vec::new(), session: None };
        match self.send(kernel, &connect) {
            Reply::Ready(Ok(Out::Rds(ids))) => ids[1],
            other => panic!("connect: {other:?}"),
        }
    }

    fn request(&mut self, kernel: &Kernel, op: Op) -> Reply<Out> {
        self.send(kernel, &Message::Request(op))
    }

    fn send(&mut self, kernel: &Kernel, message: &Message) -> Reply<Out> {
        self.seq += 1;
        let bytes = encode(self.seq, message);
        let answer =
            self.link.handle(kernel, &bytes, &mut NoLoader).expect("a reply");
        match decode(&answer) {
            Some((at, Message::Reply(reply))) if at == self.seq => reply,
            other => panic!("not the reply to {}: {other:?}", self.seq),
        }
    }
}

/// The Client's half, scripted: the tree above, the handles open on it
/// and the ops the server has asked it to run.
#[derive(Default)]
struct Client {
    side: Side,
    /// Every op the server has asked of it, in order.
    asked: Vec<Op>,
    handles: BTreeMap<Handle, Rd>,
    next_handle: u64,
}

impl Client {
    /// One turn of this Client's own loop: the `Since` that brings
    /// over what the server asks of it, and those ops run and
    /// answered. How many were run.
    fn turn(&mut self, kernel: &Kernel) -> usize {
        let (_, serve) = self.side.since(kernel);
        for (token, op) in &serve {
            let outcome = self.run(op.clone());
            let done = Op::Done { token: *token, outcome };
            let answer = self.side.request(kernel, done);
            assert_eq!(answer, Reply::ok(Out::Unit), "{op:?}");
        }
        serve.len()
    }

    /// The Client says one of its own rds changed, which is
    /// `Calls::changed` as it goes out.
    fn changed(&mut self, kernel: &Kernel, rd: Rd, kind: Kind) -> Reply<Out> {
        self.side.request(kernel, Op::Changed { rd, kind })
    }

    /// How many ops it has been asked, which a case snapshots around
    /// what it says costs none.
    const fn served(&self) -> usize {
        self.asked.len()
    }

    /// The ops asked since `from`, which [`Self::served`] gave, named
    /// by their variant and in order: what a case counts them by.
    fn since(&self, from: usize) -> String {
        let named = |op: &Op| {
            let shown = format!("{op:?}");
            shown.split(' ').next().unwrap_or_default().to_string()
        };
        self.asked[from..].iter().map(named).collect::<Vec<_>>().join(" ")
    }

    /// One op of the server's, run on this Client's own tree.
    fn run(&mut self, op: Op) -> Outcome {
        self.asked.push(op.clone());
        match op {
            Op::Lookup { rd, name } => found(rd, &name),
            Op::Children { rd } => Ok(Out::Rds(node(rd)?.children)),
            Op::Info { rd } => Ok(Out::Info(info(rd)?)),
            Op::Open { rd, .. } => Ok(Out::Handle(self.opened(rd)?)),
            Op::Read { h, at, len } => self.read(h, at, len),
            // A watch is on a handle this Client gave; the close that
            // ends it is the whole of the dropping.
            Op::Watch { h, .. } => {
                let held = self.handles.contains_key(&h);
                held.then_some(Out::Unit).ok_or(Error::Invalid)
            }
            Op::Close { h } => {
                self.handles.remove(&h);
                Ok(Out::Unit)
            }
            _ => Err(Error::Unsupported),
        }
    }

    fn opened(&mut self, rd: Rd) -> Result<Handle, Error> {
        node(rd)?;
        self.next_handle += 1;
        let h = Handle(BASE + self.next_handle);
        self.handles.insert(h, rd);
        Ok(h)
    }

    fn read(&self, h: Handle, at: u64, len: u32) -> Outcome {
        let rd = *self.handles.get(&h).ok_or(Error::Invalid)?;
        let bytes = node(rd)?.bytes;
        let at = usize::try_from(at).unwrap_or(usize::MAX).min(bytes.len());
        let end = at.saturating_add(len as usize).min(bytes.len());
        Ok(Out::Bytes(bytes[at..end].to_vec()))
    }
}

/// The root's `self` is the one name known without a lookup; every
/// other is a child of the node it is asked under.
fn found(rd: Rd, name: &str) -> Outcome {
    if rd == ROOT && name == "self" {
        return Ok(Out::Rd(SELF));
    }
    let named = |child: &Rd| node(*child).is_ok_and(|n| n.name == name);
    let found = node(rd)?.children.into_iter().find(named);
    found.map(Out::Rd).ok_or(Error::Invalid)
}

fn info(rd: Rd) -> Result<Info, Error> {
    let node = node(rd)?;
    Ok(Info {
        name: Some(node.name.to_string()),
        flags: node.flags,
        ctime: 1,
        mtime: 2,
        atime: 3,
        apis: Vec::new(),
    })
}

/// One Kernel with a Client on one link and a remote on another.
struct Rig {
    kernel: Kernel,
    client: Client,
    remote: Side,
    /// The client rd of the Client's link, under which its `/self`
    /// stands.
    at: Rd,
}

impl Rig {
    /// Both links connected and nothing served yet, so the mirror
    /// holds its root row and no rd of the Client's.
    fn fresh() -> Self {
        let kernel = Kernel::new(tick, Heap::default, Limits::default());
        let mut client = Client::default();
        let at = client.side.connect(&kernel);
        let mut remote = Side::default();
        remote.connect(&kernel);
        Self { kernel, client, remote, at }
    }

    /// The same, the Client turned once so the mirror knows the rd its
    /// one forwarded lookup finds.
    fn new() -> Self {
        let mut rig = Self::fresh();
        assert_eq!(rig.client.turn(&rig.kernel), 1, "the one forwarded lookup");
        rig
    }

    /// The rig's own remote's op, waited out.
    fn ask(&mut self, op: Op) -> Outcome {
        let Self { kernel, client, remote, .. } = self;
        ask_on(kernel, client, remote, op)
    }

    fn waited(&mut self, token: Token) -> Outcome {
        let Self { kernel, client, remote, .. } = self;
        waited_on(kernel, client, remote, token)
    }

    /// Another remote's op, waited out as the rig's own is.
    fn by(&mut self, side: &mut Side, op: Op) -> Outcome {
        let Self { kernel, client, .. } = self;
        ask_on(kernel, client, side, op)
    }

    /// A second remote on a link of its own, connected: what says that
    /// two watchers of one mirrored rd cost the Client one watch.
    fn other(&self) -> Side {
        let mut side = Side::default();
        side.connect(&self.kernel);
        side
    }

    /// The rig's own remote's handle on `rd`.
    fn opened(&mut self, rd: Rd) -> Handle {
        handle(self.ask(Op::Open { rd, rights: Rights::READ }))
    }

    fn rd(&mut self, op: Op) -> Rd {
        match self.ask(op) {
            Ok(Out::Rd(rd)) => rd,
            other => panic!("not an rd: {other:?}"),
        }
    }

    fn lookup(&mut self, rd: Rd, name: &str) -> Rd {
        self.rd(Op::Lookup { rd, name: name.to_string() })
    }

    fn rds(&mut self, rd: Rd) -> Vec<Rd> {
        match self.ask(Op::Children { rd }) {
            Ok(Out::Rds(rds)) => rds,
            other => panic!("not rds: {other:?}"),
        }
    }

    /// The names of `rds`, from the Kernel's own table.
    fn names(&mut self, rds: &[Rd]) -> Vec<String> {
        rds.iter().map(|rd| self.name(*rd)).collect()
    }

    fn name(&mut self, rd: Rd) -> String {
        match self.ask(Op::Info { rd }) {
            Ok(Out::Info(info)) => info.name.unwrap_or_default(),
            other => panic!("not info: {other:?}"),
        }
    }

    /// `/self` of the Client on the other link, which is one row of
    /// the Kernel's own table and costs no message.
    fn root(&mut self) -> Rd {
        self.lookup(self.at, "self")
    }

    /// A handle of `side`'s on the mirrored `rd`, watching it, with
    /// its cursor at the head: nothing the ring held before the watch
    /// is this watcher's to hear, as a following run places its own.
    fn watch(&mut self, side: &mut Side, rd: Rd) -> Handle {
        let h = handle(self.by(side, Op::Open { rd, rights: Rights::READ }));
        assert_eq!(self.by(side, Op::Watch { h, on: true }), Ok(Out::Unit));
        side.since(&self.kernel);
        h
    }

    /// The Client turned until it is asked nothing, which is what a
    /// forwarded watch's two ticks want.
    fn settled(&mut self) {
        while self.client.turn(&self.kernel) != 0 {}
    }
}

/// The handle an `Op::Open` answered.
fn handle(outcome: Outcome) -> Handle {
    match outcome {
        Ok(Out::Handle(h)) => h,
        other => panic!("not a handle: {other:?}"),
    }
}

/// `side`'s op, waited out: the Client turns until the token it was
/// given is answered.
fn ask_on(
    kernel: &Kernel,
    client: &mut Client,
    side: &mut Side,
    op: Op,
) -> Outcome {
    match side.request(kernel, op) {
        Reply::Ready(outcome) => outcome,
        Reply::Pending(token) => waited_on(kernel, client, side, token),
    }
}

fn waited_on(
    kernel: &Kernel,
    client: &mut Client,
    side: &mut Side,
    token: Token,
) -> Outcome {
    let mut asked = Err(Error::Busy);
    let mut turns = 0;
    while asked == Err(Error::Busy) && turns < 8 {
        turns += 1;
        client.turn(kernel);
        // One `Ask`, which is Ready here or nothing a case wants.
        asked = match side.request(kernel, Op::Ask { token }) {
            Reply::Ready(outcome) => outcome,
            other @ Reply::Pending(_) => panic!("not an answer: {other:?}"),
        };
    }
    asked
}

#[test]
fn a_remote_walks_a_clients_self_and_reads_a_cell() {
    let mut rig = Rig::new();
    let root = rig.root();
    let dev = rig.lookup(root, "dev");
    let cwd = rig.lookup(dev, "cwd");
    let h = rig.opened(cwd);
    let read = Op::Read { h, at: 0, len: 64 };
    assert_eq!(rig.ask(read), Ok(Out::Bytes(b"/tmp".to_vec())));
    assert_eq!(rig.ask(Op::Close { h }), Ok(Out::Unit));

    // The row is the Client's own name and flags, and a listing of the
    // mirror is what the Client lists.
    let listed = rig.rds(root);
    assert_eq!(rig.names(&listed), vec!["dev", "mode"]);
    match rig.ask(Op::Info { rd: cwd }) {
        Ok(Out::Info(info)) => assert_eq!(info.flags, Flags::READ),
        other => panic!("not info: {other:?}"),
    }
    // A name the Client does not have is what the Client says it is.
    let missing = Op::Lookup { rd: dev, name: "nothing".to_string() };
    assert_eq!(rig.ask(missing), Err(Error::Invalid));
}

#[test]
fn an_op_made_before_the_mirror_has_its_root_waits_for_it() {
    let mut rig = Rig::fresh();
    let root = rig.root();
    let op = Op::Lookup { rd: root, name: "dev".to_string() };
    let Reply::Pending(token) = rig.remote.request(&rig.kernel, op) else {
        panic!("the lookup waits on the Client")
    };
    // Nothing of it crosses until the Client has said where `/self`
    // is; the job waits in the queue rather than being lost or
    // refused, and the answer is the row it then makes.
    assert_eq!(rig.client.turn(&rig.kernel), 1, "the forwarded lookup alone");
    let dev = rig.waited(token);
    assert_eq!(dev, Ok(Out::Rd(rig.lookup(root, "dev"))));
}

#[test]
fn metadata_of_a_mirrored_rd_costs_no_message() {
    let mut rig = Rig::new();
    let root = rig.root();
    let dev = rig.lookup(root, "dev");
    let cwd = rig.lookup(dev, "cwd");
    rig.rds(dev);

    // Everything the Kernel answers from its own table: the served
    // count stands still across all of it, and so does a lookup of a
    // name the mirror already holds.
    let served = rig.client.served();
    assert_eq!(rig.rds(dev), vec![cwd]);
    assert_eq!(rig.rd(Op::Parent { rd: cwd }), dev);
    assert_eq!(rig.lookup(dev, "cwd"), cwd);
    match rig.ask(Op::Info { rd: dev }) {
        Ok(Out::Info(info)) => assert_eq!(info.name.as_deref(), Some("dev")),
        other => panic!("not info: {other:?}"),
    }
    assert_eq!(rig.client.served(), served, "the Client was asked");
}

#[test]
fn a_client_rd_holds_capability_heard_and_self() {
    let mut rig = Rig::new();
    let served = rig.client.served();
    let children = rig.rds(rig.at);
    assert_eq!(rig.names(&children), vec!["capability", "heard", "self"]);
    assert_eq!(rig.client.served(), served, "a listing of it crosses nothing");

    // The Driver that stands for a Client is no Module: `/module`
    // holds no row for it and `unload` never finds its name.
    let modules = rig.lookup(ROOT, "module");
    assert_eq!(rig.rds(modules), Vec::new());
    let name = format!("client:{}", rig.at.0);
    assert_eq!(rig.kernel.unload(&name), Err(Error::Invalid));
}

#[test]
fn a_client_that_answers_nothing_is_busy_past_the_queue() {
    let mut rig = Rig::new();
    let root = rig.root();
    for at in 0..64 {
        let op = Op::Lookup { rd: root, name: format!("n{at}") };
        let reply = rig.remote.request(&rig.kernel, op);
        assert!(matches!(reply, Reply::Pending(_)), "{at}: {reply:?}");
    }
    let over = Op::Lookup { rd: root, name: "n64".to_string() };
    let reply = rig.remote.request(&rig.kernel, over);
    assert_eq!(reply, Reply::err(Error::Busy), "the queue is the bound");

    // The queue is what the ops are in, not what they were: once the
    // Client has answered them there is room again.
    rig.client.turn(&rig.kernel);
    let again = Op::Lookup { rd: root, name: "n64".to_string() };
    let reply = rig.remote.request(&rig.kernel, again);
    assert!(matches!(reply, Reply::Pending(_)), "{reply:?}");
}

#[test]
fn the_links_end_cancels_what_waits_and_takes_the_rows_with_it() {
    let mut rig = Rig::new();
    let root = rig.root();
    let mode = rig.lookup(root, "mode");
    let h = rig.opened(mode);
    let read = Op::Read { h, at: 0, len: 8 };
    let Reply::Pending(token) = rig.remote.request(&rig.kernel, read) else {
        panic!("the read waits on the Client")
    };

    rig.client.side.link.end(&rig.kernel);
    let ask = rig.remote.request(&rig.kernel, Op::Ask { token });
    assert_eq!(ask, Reply::err(Error::Cancelled), "nobody is left to answer");
    let read = Op::Read { h, at: 0, len: 8 };
    assert_eq!(rig.ask(read), Err(Error::Gone), "the rd went with the link");
    assert_eq!(rig.ask(Op::Info { rd: root }), Err(Error::Invalid));
    assert_eq!(rig.ask(Op::Info { rd: rig.at }), Err(Error::Invalid));
}

/// The kinds of `side`'s events on `rd` since it last looked.
fn kinds(kernel: &Kernel, side: &mut Side, rd: Rd) -> Vec<Kind> {
    let (events, _) = side.since(kernel);
    events.iter().filter(|e| e.rd == rd).map(|e| e.kind).collect()
}

/// A rig whose Client's `mode` cell one remote watches, with the
/// forwarded watch live and nothing left to serve: the rig, that
/// remote, the mirrored rd and the remote's own handle on it.
fn watching() -> (Rig, Side, Rd, Handle) {
    let mut rig = Rig::new();
    let root = rig.root();
    let mode = rig.lookup(root, "mode");
    let mut side = rig.other();
    let h = rig.watch(&mut side, mode);
    rig.settled();
    (rig, side, mode, h)
}

#[test]
fn a_watch_costs_the_client_one_open_and_one_watch_however_many_hold_it() {
    let mut rig = Rig::new();
    let root = rig.root();
    let mode = rig.lookup(root, "mode");
    let mut first = rig.other();

    // The remote's own open is forwarded; what the watch adds is the
    // proxy's own handle and the watch behind it, two ticks and no
    // more, which nobody waits on.
    let at = rig.client.served();
    let h = rig.watch(&mut first, mode);
    rig.settled();
    assert_eq!(rig.client.since(at), "Open Open Watch", "the remote's own");

    // A second remote on the same rd is the same one handle.
    let mut second = rig.other();
    let at = rig.client.served();
    let other = rig.watch(&mut second, mode);
    rig.settled();
    assert_eq!(rig.client.since(at), "Open", "its own open and no watch");

    // One `Op::Changed` from the Client, on the Client's own rd, is an
    // event on the mirrored rd for both of them.
    let said = rig.client.changed(&rig.kernel, MODE, Kind::Modify);
    assert_eq!(said, Reply::ok(Out::Unit));
    assert_eq!(kinds(&rig.kernel, &mut first, mode), vec![Kind::Modify]);
    assert_eq!(kinds(&rig.kernel, &mut second, mode), vec![Kind::Modify]);

    // The first to leave takes nothing with it; the last takes the
    // proxy's handle, and a change after that reaches nobody.
    let at = rig.client.served();
    let off = Op::Watch { h, on: false };
    assert_eq!(rig.by(&mut first, off), Ok(Out::Unit));
    rig.settled();
    assert_eq!(rig.client.since(at), "", "one watcher is left");
    let off = Op::Watch { h: other, on: false };
    assert_eq!(rig.by(&mut second, off), Ok(Out::Unit));
    rig.settled();
    assert_eq!(rig.client.since(at), "Close", "the proxy's own handle");
    let _ = rig.client.changed(&rig.kernel, MODE, Kind::Modify);
    assert_eq!(kinds(&rig.kernel, &mut first, mode), Vec::new());
}

#[test]
fn a_second_watch_off_leaves_another_handles_watch_alone() {
    let mut rig = Rig::new();
    let root = rig.root();
    let mode = rig.lookup(root, "mode");
    let mut first = rig.other();
    let mut second = rig.other();
    let h = rig.watch(&mut first, mode);
    rig.watch(&mut second, mode);
    rig.settled();

    // The same `off` twice is one unwatch: the hint follows the slot's
    // own state, so the second says nothing and the count the proxy
    // keeps still holds the watcher that is left.
    let at = rig.client.served();
    for _ in 0..2 {
        let off = Op::Watch { h, on: false };
        assert_eq!(rig.by(&mut first, off), Ok(Out::Unit));
        rig.settled();
    }
    assert_eq!(rig.client.since(at), "", "one watcher is left");
}

#[test]
fn a_second_watch_on_costs_nothing_and_the_close_still_ends_it() {
    let (mut rig, mut remote, _, h) = watching();

    // A slot that watches already is told nothing, so the count does
    // not climb past the one handle and the close that ends the watch
    // brings it back to none.
    let at = rig.client.served();
    let on = Op::Watch { h, on: true };
    assert_eq!(rig.by(&mut remote, on), Ok(Out::Unit));
    rig.settled();
    assert_eq!(rig.client.since(at), "", "the slot watched already");
    assert_eq!(rig.by(&mut remote, Op::Close { h }), Ok(Out::Unit));
    rig.settled();
    let both = "Close Close";
    assert_eq!(rig.client.since(at), both, "the remote's and the proxy's");
}

#[test]
fn a_watch_flipped_while_its_open_is_in_flight_queues_nothing_more() {
    let mut rig = Rig::new();
    let root = rig.root();
    let mode = rig.lookup(root, "mode");
    let mut side = rig.other();
    let open = Op::Open { rd: mode, rights: Rights::READ };
    let h = handle(rig.by(&mut side, open));

    // The row holds what is wanted and makes its op once: a remote
    // that never lets the Client answer flips the watch as often as it
    // likes and the queue does not grow with it.
    let at = rig.client.served();
    for on in [true, false, true, false, true, false] {
        assert_eq!(rig.by(&mut side, Op::Watch { h, on }), Ok(Out::Unit));
    }
    rig.settled();
    let both = "Open Close";
    assert_eq!(rig.client.since(at), both, "the open and its own close");
}

#[test]
fn a_watch_whose_open_the_client_refuses_leaves_the_rd_to_the_next() {
    let mut rig = Rig::new();
    let root = rig.root();
    let mode = rig.lookup(root, "mode");
    let mut side = rig.other();
    let open = Op::Open { rd: mode, rights: Rights::READ };
    let h = handle(rig.by(&mut side, open));
    assert_eq!(rig.by(&mut side, Op::Watch { h, on: true }), Ok(Out::Unit));

    // The Client refuses the proxy's own `open`, by hand: the row has
    // no handle and never will.
    let (_, serve) = rig.client.side.since(&rig.kernel);
    let [(own, Op::Open { .. })] = serve.as_slice() else {
        panic!("not the watch's open alone: {serve:?}")
    };
    let done = Op::Done { token: *own, outcome: Err(Error::Denied) };
    let answered = rig.client.side.request(&rig.kernel, done);
    assert_eq!(answered, Reply::ok(Out::Unit));

    // A later watcher of the same rd costs the `Open` it should: the
    // refusal took the row with it rather than standing in the way of
    // every watch of this rd for ever.
    let mut other = rig.other();
    let at = rig.client.served();
    let open = Op::Open { rd: mode, rights: Rights::READ };
    let second = handle(rig.by(&mut other, open));
    let on = Op::Watch { h: second, on: true };
    assert_eq!(rig.by(&mut other, on), Ok(Out::Unit));
    rig.settled();
    let all = "Open Open Watch";
    assert_eq!(rig.client.since(at), all, "the remote's and the proxy's");
}

#[test]
fn a_close_past_the_queue_still_reaches_the_client() {
    let mut rig = Rig::new();
    let root = rig.root();
    let mode = rig.lookup(root, "mode");
    let h = rig.opened(mode);
    for at in 0..64 {
        let op = Op::Lookup { rd: root, name: format!("n{at}") };
        let reply = rig.remote.request(&rig.kernel, op);
        assert!(matches!(reply, Reply::Pending(_)), "{at}: {reply:?}");
    }
    let over = Op::Lookup { rd: root, name: "n64".to_string() };
    assert_eq!(rig.remote.request(&rig.kernel, over), Reply::err(Error::Busy));

    // The handle is gone at the Kernel, so its close is not one of the
    // ops the bound refuses: dropping it would leave a handle at the
    // Client for ever and no record of it anywhere.
    let close = rig.remote.request(&rig.kernel, Op::Close { h });
    assert_eq!(close, Reply::ok(Out::Unit));
    let at = rig.client.served();
    rig.settled();
    let asked = rig.client.since(at);
    assert!(asked.split(' ').any(|op| op == "Close"), "{asked}");
}

#[test]
fn an_answer_of_a_shape_the_op_never_promises_is_invalid() {
    let mut rig = Rig::new();
    let root = rig.root();
    let dev = rig.lookup(root, "dev");
    let cwd = rig.lookup(dev, "cwd");
    let h = rig.opened(cwd);
    let read = Op::Read { h, at: 0, len: 4 };
    let Reply::Pending(token) = rig.remote.request(&rig.kernel, read) else {
        panic!("the read waits on the Client")
    };

    // A Client may answer with anything; what is not the shape the op
    // promises is no answer to it, and the caller hears so rather than
    // a handle out of a read.
    let (_, serve) = rig.client.side.since(&rig.kernel);
    let [(own, Op::Read { .. })] = serve.as_slice() else {
        panic!("not the read alone: {serve:?}")
    };
    let done = Op::Done { token: *own, outcome: Ok(Out::Handle(Handle(7))) };
    let answered = rig.client.side.request(&rig.kernel, done);
    assert_eq!(answered, Reply::ok(Out::Unit));
    let ask = rig.remote.request(&rig.kernel, Op::Ask { token });
    assert_eq!(ask, Reply::Ready(Err(Error::Invalid)));
}

#[test]
fn a_remote_that_closes_without_unwatching_drops_the_forwarded_watch() {
    let (mut rig, mut remote, _, h) = watching();

    // `dispatch::close` tells the Driver of the unwatch a close
    // implies, so the count reaches zero this way too and the Client
    // is not left sending a message a frame for ever.
    let at = rig.client.served();
    assert_eq!(rig.by(&mut remote, Op::Close { h }), Ok(Out::Unit));
    rig.settled();
    let both = "Close Close";
    assert_eq!(rig.client.since(at), both, "the remote's and the proxy's");
}

#[test]
fn a_change_the_mirror_cannot_place_is_answered_and_dropped() {
    let (mut rig, mut remote, mode, _) = watching();

    // A Client may say anything and the Kernel believes none of it: a
    // kind `Calls::changed` does not take, and an rd the mirror has no
    // row for, are answered `Unit` and change nothing.
    for kind in [Kind::Create, Kind::Delete, Kind::Move, Kind::Gone] {
        let said = rig.client.changed(&rig.kernel, MODE, kind);
        assert_eq!(said, Reply::ok(Out::Unit), "{kind:?}");
    }
    let stranger = rig.client.changed(&rig.kernel, Rd(BASE + 99), Kind::Modify);
    assert_eq!(stranger, Reply::ok(Out::Unit));
    assert_eq!(kinds(&rig.kernel, &mut remote, mode), Vec::new());

    // `/sys/info/watchers` counts the Kernel's own watch table, where
    // the remote's watch stands; the proxy's watch at the Client is
    // the Client's and is counted nowhere, so the two do not double.
    let sys = rig.lookup(ROOT, "sys");
    let facts = rig.lookup(sys, "info");
    let counted = rig.lookup(facts, "watchers");
    let read = Op::Read { h: rig.opened(counted), at: 0, len: 16 };
    assert_eq!(rig.ask(read), Ok(Out::Bytes(b"1".to_vec())));
}
