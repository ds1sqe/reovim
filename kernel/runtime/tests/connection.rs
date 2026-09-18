//! One `Connection` on a Kernel with no Module: connect, ask, repeat.

use reovim_kernel_interface::wire::{Message, decode, encode};
use reovim_kernel_interface::{Error, Op, Out, Rd, Reply, Rights};
use reovim_kernel_runtime::{
    Connection, Heap, Kernel, Limits, Loader, NoLoader, ROOT, SYS,
};

fn tick() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CLOCK: AtomicU64 = AtomicU64::new(1);
    CLOCK.fetch_add(1, Ordering::Relaxed)
}

struct Rig {
    kernel: Kernel,
    link: Connection,
    seq: u64,
}

impl Rig {
    fn new() -> Self {
        let kernel = Kernel::new(tick, Heap::default, Limits::default());
        let link = Connection::new();
        Self { kernel, link, seq: 0 }
    }

    /// Send with the next sequence; the reply, decoded.
    fn send(&mut self, message: &Message) -> Reply<Out> {
        self.seq += 1;
        self.send_at(self.seq, message)
    }

    fn send_at(&mut self, seq: u64, message: &Message) -> Reply<Out> {
        let bytes = encode(seq, message);
        let answer = self
            .link
            .handle(&self.kernel, &bytes, &mut NoLoader)
            .expect("a reply");
        match decode(&answer) {
            Some((at, Message::Reply(reply))) if at == seq => reply,
            other => panic!("not the reply to {seq}: {other:?}"),
        }
    }

    fn connect(&mut self, session: Option<Rd>) -> Reply<Out> {
        self.send(&Message::Connect { credential: Vec::new(), session })
    }

    fn request(&mut self, op: Op) -> Reply<Out> {
        self.send(&Message::Request(op))
    }

    fn rd(&mut self, op: Op) -> Rd {
        match self.request(op) {
            Reply::Ready(Ok(Out::Rd(rd))) => rd,
            other => panic!("not an rd: {other:?}"),
        }
    }

    fn lookup(&mut self, rd: Rd, name: &str) -> Rd {
        self.rd(Op::Lookup { rd, name: name.to_string() })
    }

    fn rds(&mut self, rd: Rd) -> Vec<Rd> {
        match self.request(Op::Children { rd }) {
            Reply::Ready(Ok(Out::Rds(rds))) => rds,
            other => panic!("not rds: {other:?}"),
        }
    }

    fn atime(&mut self, rd: Rd) -> u64 {
        match self.request(Op::Info { rd }) {
            Reply::Ready(Ok(Out::Info(info))) => info.atime,
            other => panic!("not info: {other:?}"),
        }
    }

    /// The whole of a Kernel cell holding one number, read and closed.
    fn number(&mut self, rd: Rd) -> u64 {
        let open = Op::Open { rd, rights: Rights::READ };
        let h = match self.request(open) {
            Reply::Ready(Ok(Out::Handle(h))) => h,
            other => panic!("not a handle: {other:?}"),
        };
        let text = match self.request(Op::Read { h, at: 0, len: 32 }) {
            Reply::Ready(Ok(Out::Bytes(bytes))) => {
                String::from_utf8(bytes).unwrap()
            }
            other => panic!("not bytes: {other:?}"),
        };
        self.request(Op::Close { h });
        text.parse().unwrap_or_else(|_| panic!("not a number: {text}"))
    }

    fn log(&mut self) -> String {
        let log = self.lookup(SYS, "log");
        let h = match self.request(Op::Open { rd: log, rights: Rights::READ }) {
            Reply::Ready(Ok(Out::Handle(h))) => h,
            other => panic!("not a handle: {other:?}"),
        };
        match self.request(Op::Read { h, at: 0, len: 1 << 16 }) {
            Reply::Ready(Ok(Out::Bytes(bytes))) => {
                String::from_utf8_lossy(&bytes).into_owned()
            }
            other => panic!("not bytes: {other:?}"),
        }
    }
}

#[test]
fn connect_makes_the_session_and_the_client_rd() {
    let mut rig = Rig::new();
    let Reply::Ready(Ok(Out::Rds(ids))) = rig.connect(None) else {
        panic!("connect");
    };
    let [session, client] = ids[..] else { panic!("two rds: {ids:?}") };
    assert_eq!(rig.link.client(), Some(client));

    let sessions = rig.lookup(ROOT, "session");
    assert_eq!(rig.rds(sessions), vec![session]);
    let clients = rig.lookup(session, "client");
    assert_eq!(rig.rds(clients), vec![client]);
    assert_eq!(rig.rd(Op::Parent { rd: client }), clients);
    let line = format!("connect session={} client={}", session.0, client.0);
    assert!(rig.log().contains(&line), "log lacks {line}");

    // A second client joins the same Session.
    let mut other = Connection::new();
    let join =
        Message::Connect { credential: Vec::new(), session: Some(session) };
    let answer =
        other.handle(&rig.kernel, &encode(1, &join), &mut NoLoader).unwrap();
    let (_, reply) = decode(&answer).unwrap();
    let Message::Reply(Reply::Ready(Ok(Out::Rds(ids)))) = reply else {
        panic!("join: {reply:?}");
    };
    assert_eq!(ids[0], session);
    assert_eq!(rig.rds(clients), vec![client, ids[1]]);
}

#[test]
fn connect_is_once_per_link_and_only_to_a_session() {
    let mut rig = Rig::new();
    assert_eq!(rig.connect(Some(ROOT)), Reply::err(Error::Invalid));
    assert_eq!(rig.connect(Some(Rd(999))), Reply::err(Error::Invalid));
    let first = rig.connect(None);
    assert!(matches!(first, Reply::Ready(Ok(Out::Rds(_)))));
    // The same Connect again, as after a lost reply: the same answer.
    let again = Message::Connect { credential: Vec::new(), session: None };
    assert_eq!(rig.send_at(rig.seq, &again), first);
    assert_eq!(rig.connect(None), Reply::err(Error::Invalid));
}

#[test]
fn a_repeated_sequence_is_answered_without_running() {
    let mut rig = Rig::new();
    rig.connect(None);
    let log = rig.lookup(SYS, "log");
    let h = match rig.request(Op::Open { rd: log, rights: Rights::READ }) {
        Reply::Ready(Ok(Out::Handle(h))) => h,
        other => panic!("not a handle: {other:?}"),
    };
    let read = Message::Request(Op::Read { h, at: 0, len: 8 });
    let first = rig.send(&read);
    let stamped = rig.atime(log);
    assert_eq!(rig.send_at(rig.seq - 1, &read), first);
    assert_eq!(rig.atime(log), stamped, "the read ran twice");

    // Not a message, or a Reply: nothing comes back.
    assert!(rig.link.handle(&rig.kernel, b"junk", &mut NoLoader).is_none());
    let reply = encode(9, &Message::Reply(Reply::ok(Out::Unit)));
    assert!(rig.link.handle(&rig.kernel, &reply, &mut NoLoader).is_none());
}

/// A loader that counts; `unload` is refused.
struct Counting(u32);

impl Loader for Counting {
    fn load(&mut self, _: &Kernel, path: &str) -> Result<(), Error> {
        assert_eq!(path, "/x.rvm");
        self.0 += 1;
        Ok(())
    }

    fn unload(&mut self, _: &Kernel, _: &str) -> Result<(), Error> {
        Err(Error::Denied)
    }
}

#[test]
fn load_and_unload_go_to_the_loader_under_the_reply_window() {
    let mut rig = Rig::new();
    rig.connect(None);
    let load = encode(9, &Message::Load { path: "/x.rvm".to_string() });
    let unload = encode(10, &Message::Unload { name: "x".to_string() });
    let mut counting = Counting(0);
    let reply = |bytes: Option<Vec<u8>>| match decode(&bytes.unwrap()) {
        Some((_, Message::Reply(reply))) => reply,
        other => panic!("{other:?}"),
    };
    let once = rig.link.handle(&rig.kernel, &load, &mut counting);
    assert_eq!(reply(once), Reply::ok(Out::Unit));
    let again = rig.link.handle(&rig.kernel, &load, &mut counting);
    assert_eq!(reply(again), Reply::ok(Out::Unit));
    assert_eq!(counting.0, 1, "answered from the window");
    let refused = rig.link.handle(&rig.kernel, &unload, &mut counting);
    assert_eq!(reply(refused), Reply::err(Error::Denied));
    let none = encode(11, &Message::Load { path: "/y.rvm".to_string() });
    let nothing = rig.link.handle(&rig.kernel, &none, &mut NoLoader);
    assert_eq!(reply(nothing), Reply::err(Error::Unsupported));
}

/// `Connect` as `credential` on `link`; the reply.
fn named(
    link: &mut Connection,
    kernel: &Kernel,
    seq: u64,
    credential: &[u8],
    session: Option<Rd>,
) -> Reply<Out> {
    let connect = Message::Connect { credential: credential.to_vec(), session };
    let bytes = encode(seq, &connect);
    let answer = link.handle(kernel, &bytes, &mut NoLoader).unwrap();
    match decode(&answer) {
        Some((_, Message::Reply(reply))) => reply,
        other => panic!("{other:?}"),
    }
}

/// One request on `link` at `seq`; the reply.
fn ask(link: &mut Connection, kernel: &Kernel, seq: u64, op: Op) -> Reply<Out> {
    let bytes = encode(seq, &Message::Request(op));
    let answer = link.handle(kernel, &bytes, &mut NoLoader).unwrap();
    match decode(&answer) {
        Some((at, Message::Reply(reply))) if at == seq => reply,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_named_caller_survives_its_link_and_an_unnamed_one_ends() {
    let mut rig = Rig::new();
    let me = Message::Connect { credential: b"me".to_vec(), session: None };
    let Reply::Ready(Ok(Out::Rds(ids))) = rig.send(&me) else {
        panic!("connect")
    };
    let session = ids[0];
    let log = rig.lookup(SYS, "log");
    let open = Op::Open { rd: log, rights: Rights::READ };
    let Reply::Ready(Ok(Out::Handle(h))) = rig.request(open.clone()) else {
        panic!("open")
    };
    let mut other = Connection::new();
    let busy = named(&mut other, &rig.kernel, 1, b"me", Some(session));
    assert_eq!(busy, Reply::err(Error::Busy), "the first link is on it");

    rig.link.end(&rig.kernel);
    // The sequence goes on where the old link left it: the reply window
    // is the caller's, and a low seq would replay an old answer.
    let again = named(&mut other, &rig.kernel, 20, b"me", Some(session));
    assert_eq!(again, Reply::ok(Out::Rds(ids)), "the same ids");
    let read = Op::Read { h, at: 0, len: 8 };
    let got = ask(&mut other, &rig.kernel, 21, read);
    assert!(matches!(got, Reply::Ready(Ok(Out::Bytes(_)))), "{got:?}");
    let stranger = named(&mut Connection::new(), &rig.kernel, 1, b"me", None);
    let Reply::Ready(Ok(Out::Rds(new))) = stranger else { panic!("connect") };
    assert_ne!(new[0], session, "another Session, another caller");

    let mut unnamed = Connection::new();
    let joined = named(&mut unnamed, &rig.kernel, 1, b"", Some(session));
    assert!(matches!(joined, Reply::Ready(Ok(Out::Rds(_)))), "{joined:?}");
    let Reply::Ready(Ok(Out::Handle(h2))) =
        ask(&mut unnamed, &rig.kernel, 2, open)
    else {
        panic!("open")
    };
    unnamed.end(&rig.kernel);
    let mut third = Connection::new();
    named(&mut third, &rig.kernel, 1, b"", Some(session));
    let gone =
        ask(&mut third, &rig.kernel, 2, Op::Read { h: h2, at: 0, len: 8 });
    assert_eq!(
        gone,
        Reply::err(Error::Invalid),
        "the handle went with the link"
    );
    let early =
        ask(&mut Connection::new(), &rig.kernel, 1, Op::Info { rd: ROOT });
    assert_eq!(early, Reply::err(Error::Invalid), "nothing before Connect");
}

#[test]
fn heard_is_the_clock_at_that_callers_last_message() {
    let mut rig = Rig::new();
    let Reply::Ready(Ok(Out::Rds(ids))) = rig.connect(None) else {
        panic!("connect")
    };
    let (session, client) = (ids[0], ids[1]);
    // The two names the Kernel keeps under a client rd.
    assert_eq!(rig.rds(client).len(), 2, "capability and heard");

    // A second link that says nothing after its `Connect`.
    let mut idle = Connection::new();
    let joined = named(&mut idle, &rig.kernel, 1, b"", Some(session));
    let Reply::Ready(Ok(Out::Rds(ids))) = joined else { panic!("join") };
    let theirs = rig.lookup(ids[1], "heard");
    let at_connect = rig.number(theirs);

    // The reader's own moves with every op it sends; the idle link's
    // stands still however many the reader sends.
    let mine = rig.lookup(client, "heard");
    let first = rig.number(mine);
    assert!(rig.number(mine) > first, "the reader spoke twice");
    assert_eq!(rig.number(theirs), at_connect, "the idle link said nothing");

    // Every message counts as speech: a `Since`, which an idle tui
    // asks on its loop and an idle cli does not; the same sequence
    // sent again, answered from the window without running; and a
    // `Load` this Kernel refuses.
    let since = Op::Since { cursor: 0 };
    ask(&mut idle, &rig.kernel, 2, since.clone());
    let spoke = rig.number(theirs);
    assert!(spoke > at_connect, "a Since is speech");
    ask(&mut idle, &rig.kernel, 2, since);
    let repeated = rig.number(theirs);
    assert!(repeated > spoke, "a repeated sequence is speech");
    let load = encode(3, &Message::Load { path: "/x.rvm".to_string() });
    idle.handle(&rig.kernel, &load, &mut NoLoader).expect("a reply");
    assert!(rig.number(theirs) > repeated, "a refused Load is speech");

    // An unnamed caller's cell goes with its client rd.
    idle.end(&rig.kernel);
    let gone = rig.request(Op::Info { rd: theirs });
    assert_eq!(gone, Reply::err(Error::Invalid), "the cell outlived the link");
}

#[test]
fn a_named_callers_heard_stands_where_its_link_left_it() {
    let mut rig = Rig::new();
    let Reply::Ready(Ok(Out::Rds(ids))) = rig.connect(None) else {
        panic!("connect")
    };
    let session = ids[0];
    let mut link = Connection::new();
    let Reply::Ready(Ok(Out::Rds(theirs))) =
        named(&mut link, &rig.kernel, 1, b"me", Some(session))
    else {
        panic!("connect")
    };
    let heard = rig.lookup(theirs[1], "heard");
    let at_connect = rig.number(heard);

    // The caller waits for its next `Connect` with everything it
    // holds, the moment it last spoke among them.
    link.end(&rig.kernel);
    assert_eq!(rig.number(heard), at_connect, "an ended link is not speech");
    let mut again = Connection::new();
    let back = named(&mut again, &rig.kernel, 20, b"me", Some(session));
    assert_eq!(back, Reply::ok(Out::Rds(theirs)), "the same ids");
    assert!(rig.number(heard) > at_connect, "the Connect is its moment");
}

#[test]
fn the_client_rd_carries_a_capability_cell() {
    let mut rig = Rig::new();
    let Reply::Ready(Ok(Out::Rds(ids))) = rig.connect(None) else {
        panic!("connect")
    };
    let capability = rig.lookup(ids[1], "capability");
    let open =
        Op::Open { rd: capability, rights: Rights::READ | Rights::WRITE };
    let Reply::Ready(Ok(Out::Handle(h))) = rig.request(open) else {
        panic!("open")
    };
    let read = |rig: &mut Rig| rig.request(Op::Read { h, at: 0, len: 16 });
    assert_eq!(read(&mut rig), Reply::ok(Out::Bytes(Vec::new())));
    let write = Op::Write { h, at: 0, bytes: b"vim keys".to_vec() };
    assert_eq!(rig.request(write), Reply::ok(Out::Len(8)));
    assert_eq!(read(&mut rig), Reply::ok(Out::Bytes(b"vim keys".to_vec())));
    let past = Op::Write { h, at: 9, bytes: b"x".to_vec() };
    assert_eq!(rig.request(past), Reply::err(Error::Invalid));
    let shorter = Op::Write { h, at: 3, bytes: Vec::new() };
    assert_eq!(rig.request(shorter), Reply::ok(Out::Len(0)));
    assert_eq!(read(&mut rig), Reply::ok(Out::Bytes(b"vim".to_vec())));
}
