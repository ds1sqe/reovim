//! One `Connection` on a Kernel with no Module: connect, ask, repeat.

use reovim_kernel_interface::wire::{Message, decode, encode};
use reovim_kernel_interface::{Error, Op, Out, Rd, Reply, Rights};
use reovim_kernel_runtime::{
    Connection, Kernel, Limits, Loader, NoLoader, ROOT, SYS,
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
        let kernel = Kernel::new(tick, Limits::default());
        let link = Connection::new(&kernel);
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
    let mut other = Connection::new(&rig.kernel);
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
