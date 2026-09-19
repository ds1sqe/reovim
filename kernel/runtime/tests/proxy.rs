//! Two links on one Kernel, where the Client on one serves what the
//! remote on the other asks of `/session/<id>/client/<cid>/self`: the
//! rows the mirror makes, what costs a message and what does not, the
//! bound on a Client that answers nothing, and the link's end.

use std::collections::BTreeMap;

use reovim_kernel_interface::wire::{Message, decode, encode};
use reovim_kernel_interface::{
    Error, Flags, Handle, Info, Op, Out, Outcome, Rd, Reply, Rights, Token,
};
use reovim_kernel_runtime::{Connection, Heap, Kernel, Limits, NoLoader, ROOT};

fn tick() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CLOCK: AtomicU64 = AtomicU64::new(1);
    CLOCK.fetch_add(1, Ordering::Relaxed)
}

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

/// One end of a link: its `Connection` and the sequence it counts.
#[derive(Default)]
struct Side {
    link: Connection,
    seq: u64,
}

impl Side {
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
    cursor: u64,
    served: usize,
    handles: BTreeMap<Handle, Rd>,
    next_handle: u64,
}

impl Client {
    /// One turn of this Client's own loop: the `Since` that brings
    /// over what the server asks of it, and those ops run and
    /// answered. How many were run.
    fn turn(&mut self, kernel: &Kernel) -> usize {
        let since = Op::Since { cursor: self.cursor };
        let (cursor, serve) = match self.side.request(kernel, since) {
            Reply::Ready(Ok(Out::Events { cursor, serve, .. })) => {
                (cursor, serve)
            }
            other => panic!("not events: {other:?}"),
        };
        self.cursor = cursor;
        for (token, op) in &serve {
            let outcome = self.run(op.clone());
            let done = Op::Done { token: *token, outcome };
            let answer = self.side.request(kernel, done);
            assert_eq!(answer, Reply::ok(Out::Unit), "{op:?}");
        }
        serve.len()
    }

    /// One op of the server's, run on this Client's own tree.
    fn run(&mut self, op: Op) -> Outcome {
        self.served += 1;
        match op {
            Op::Lookup { rd, name } => found(rd, &name),
            Op::Children { rd } => Ok(Out::Rds(node(rd)?.children)),
            Op::Info { rd } => Ok(Out::Info(info(rd)?)),
            Op::Open { rd, .. } => Ok(Out::Handle(self.opened(rd)?)),
            Op::Read { h, at, len } => self.read(h, at, len),
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

    /// The remote's op, waited out: the Client turns until the token
    /// it was given is answered.
    fn ask(&mut self, op: Op) -> Outcome {
        match self.remote.request(&self.kernel, op) {
            Reply::Ready(outcome) => outcome,
            Reply::Pending(token) => self.waited(token),
        }
    }

    fn waited(&mut self, token: Token) -> Outcome {
        let mut asked = Err(Error::Busy);
        let mut turns = 0;
        while asked == Err(Error::Busy) && turns < 8 {
            turns += 1;
            self.client.turn(&self.kernel);
            asked = self.asked(token);
        }
        asked
    }

    /// One `Ask`, which is Ready here or nothing a case wants.
    fn asked(&mut self, token: Token) -> Outcome {
        match self.remote.request(&self.kernel, Op::Ask { token }) {
            Reply::Ready(outcome) => outcome,
            other @ Reply::Pending(_) => panic!("not an answer: {other:?}"),
        }
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
}

#[test]
fn a_remote_walks_a_clients_self_and_reads_a_cell() {
    let mut rig = Rig::new();
    let root = rig.root();
    let dev = rig.lookup(root, "dev");
    let cwd = rig.lookup(dev, "cwd");
    let h = match rig.ask(Op::Open { rd: cwd, rights: Rights::READ }) {
        Ok(Out::Handle(h)) => h,
        other => panic!("not a handle: {other:?}"),
    };
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
    let served = rig.client.served;
    assert_eq!(rig.rds(dev), vec![cwd]);
    assert_eq!(rig.rd(Op::Parent { rd: cwd }), dev);
    assert_eq!(rig.lookup(dev, "cwd"), cwd);
    match rig.ask(Op::Info { rd: dev }) {
        Ok(Out::Info(info)) => assert_eq!(info.name.as_deref(), Some("dev")),
        other => panic!("not info: {other:?}"),
    }
    assert_eq!(rig.client.served, served, "the Client was asked");
}

#[test]
fn a_client_rd_holds_capability_heard_and_self() {
    let mut rig = Rig::new();
    let served = rig.client.served;
    let children = rig.rds(rig.at);
    assert_eq!(rig.names(&children), vec!["capability", "heard", "self"]);
    assert_eq!(rig.client.served, served, "a listing of it crosses nothing");

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
    let h = match rig.ask(Op::Open { rd: mode, rights: Rights::READ }) {
        Ok(Out::Handle(h)) => h,
        other => panic!("not a handle: {other:?}"),
    };
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
