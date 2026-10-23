//! `/sys/log` is a ring: past `log_ring` entries the oldest go, and a
//! read from before the oldest starts at the oldest.

mod common;

use std::rc::Rc;

use common::{ok, tick, written};
use reovim_kernel_interface::{
    Calls, Decl, Driver, DriverId, Error, Flags, Hash, Method, Module, Op, Out,
    Rd, Reply, Rights, Ty,
};
use reovim_kernel_runtime::{Heap, Kernel, LOG, LOGLEVEL, Limits};

#[test]
fn the_oldest_entries_are_trimmed() {
    let limits = Limits { log_ring: 3, ..Limits::default() };
    let kernel = Kernel::new(|| 7, Heap::default, limits);
    let caller = kernel.caller_new();
    let open = Op::Open { rd: LOG, rights: Rights::READ | Rights::WRITE };
    let Out::Handle(h) = ok(&kernel, caller, 1, open) else { panic!("open") };
    for (i, line) in [b"a", b"b", b"c", b"d"].iter().enumerate() {
        let write = Op::Write { h, at: 0, bytes: line.to_vec() };
        ok(&kernel, caller, 2 + i as u64, write);
    }
    let Out::Bytes(bytes) =
        ok(&kernel, caller, 9, Op::Read { h, at: 1, len: 4096 })
    else {
        panic!("read")
    };
    let first = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    assert_eq!(first, 2, "entry 1 is gone");
    assert!(bytes.ends_with(b"d"), "the newest is kept: {bytes:?}");
    let Out::Bytes(none) =
        ok(&kernel, caller, 10, Op::Read { h, at: 5, len: 64 })
    else {
        panic!("read")
    };
    assert_eq!(none, Vec::new(), "nothing at the next seq yet");
}

/// A Module that refuses to boot, so `load` says so at `err`.
struct Broken;

impl Module for Broken {
    fn start(&self, _: &dyn Calls, _: Rd) -> Result<(), Error> {
        Err(Error::NoSpace)
    }
}

impl Driver for Broken {}

#[test]
fn a_written_entry_takes_its_level_from_the_printk_prefix() {
    let kernel = Kernel::new(|| 7, Heap::default, Limits::default());
    let caller = kernel.caller_new();
    let open = Op::Open { rd: LOG, rights: Rights::READ | Rights::WRITE };
    let Out::Handle(h) = ok(&kernel, caller, 1, open) else { panic!("open") };
    let write = |seq: u64, bytes: &[u8]| {
        let op = Op::Write { h, at: 0, bytes: bytes.to_vec() };
        ok(&kernel, caller, seq, op)
    };
    // The prefix goes into the byte; the count is of what was given.
    assert_eq!(write(2, b"<3>boom"), Out::Len(7));
    write(3, b"plain");
    // Only one digit of the eight, and only at the front.
    write(4, b"<9>high");
    write(5, b"a <3>late");

    let said = written(&kernel);
    let text = |at: usize| (said[at].1, said[at].2.clone());
    assert_eq!(text(0), (3, "boom".to_string()));
    assert_eq!(text(1), (6, "plain".to_string()));
    assert_eq!(text(2), (6, "<9>high".to_string()));
    assert_eq!(text(3), (6, "a <3>late".to_string()));
    assert_eq!(said[0].0, "caller", "a caller's own mark");
}

#[test]
fn every_level_the_kernel_writes_has_its_source() {
    let limits = Limits { watch_ring: 2, ..Limits::default() };
    let kernel = Kernel::new(|| 7, Heap::default, limits);
    let broken = Rc::new(Broken);
    assert!(kernel.load("broken", broken.clone(), vec![broken]).is_err());

    // A caller that watches the log, falls behind the ring and ends
    // with its handle open: the overflow and the end.
    let caller = kernel.caller_new();
    let open = Op::Open { rd: LOG, rights: Rights::READ };
    let Out::Handle(h) = ok(&kernel, caller, 1, open) else { panic!("open") };
    ok(&kernel, caller, 2, Op::Watch { h, on: true });
    kernel.session_new();
    kernel.connect(kernel.session_new(), b"").unwrap();
    ok(&kernel, caller, 3, Op::Since { cursor: 0 });
    kernel.caller_end(caller);

    let said = written(&kernel);
    let at = |level: u8| said.iter().find(|e| e.1 == level).map(|e| &e.2);
    assert_eq!(at(3).unwrap(), "load module=broken refused=NoSpace");
    assert_eq!(at(6).unwrap(), "load module=broken");
    assert!(at(4).unwrap().starts_with("overflow caller="), "{said:?}");
    assert!(at(5).unwrap().starts_with("caller end caller="), "{said:?}");
    assert!(said.iter().all(|e| e.0 == "kernel"), "{said:?}");
}

/// The level cell written, as a caller of its own writes it.
fn set_level(kernel: &Kernel, level: &str) {
    let caller = kernel.caller_new();
    let open = Op::Open { rd: LOGLEVEL, rights: Rights::WRITE };
    let Out::Handle(h) = ok(kernel, caller, 1, open) else { panic!("open") };
    let write = Op::Write { h, at: 0, bytes: level.as_bytes().to_vec() };
    ok(kernel, caller, 2, write);
}

/// The text of every entry now.
fn texts(kernel: &Kernel) -> Vec<String> {
    written(kernel).into_iter().map(|(_, _, text)| text).collect()
}

/// A traced line without its measure: what the clock and the host's
/// allocator counted are the host's, and only that the Kernel measured
/// the op at all belongs here.
fn measured(line: &str) -> String {
    let (words, spent) = line.rsplit_once(" took=").unwrap_or_else(|| {
        panic!("no took=: {line}");
    });
    let (nanos, bytes) = spent.split_once(" heap=").unwrap_or_else(|| {
        panic!("no heap=: {line}");
    });
    assert!(nanos.parse::<u64>().unwrap() > 0, "{line}");
    assert!(bytes.starts_with(['+', '-']), "{line}");
    assert!(bytes[1..].parse::<u64>().is_ok(), "{line}");
    words.to_string()
}

#[test]
fn the_level_is_read_back_by_name_and_written_by_name_or_number() {
    let kernel = Kernel::new(tick, Heap::default, Limits::default());
    let caller = kernel.caller_new();
    let open = Op::Open { rd: LOGLEVEL, rights: Rights::READ | Rights::WRITE };
    let Out::Handle(h) = ok(&kernel, caller, 1, open) else { panic!("open") };
    let seq = std::cell::Cell::new(1);
    let next = || {
        seq.set(seq.get() + 1);
        seq.get()
    };
    let read = || {
        let op = Op::Read { h, at: 0, len: 64 };
        let Out::Bytes(bytes) = ok(&kernel, caller, next(), op) else {
            panic!("read")
        };
        String::from_utf8(bytes).unwrap()
    };
    assert_eq!(read(), "info", "the level at boot");

    let write = |at: u64, bytes: &[u8]| {
        let op = Op::Write { h, at, bytes: bytes.to_vec() };
        kernel.submit(caller, next(), op)
    };
    // A name, its number, and the trailing newline of a write by hand.
    assert_eq!(write(0, b"debug"), Reply::ok(Out::Len(5)));
    assert_eq!(read(), "debug");
    assert_eq!(write(0, b"4"), Reply::ok(Out::Len(1)));
    assert_eq!(read(), "warn");
    write(0, b"info\n");
    assert_eq!(read(), "info");
    // A word that is not one of the eight, a number past them, and a
    // write that is not at the front: the level stands where it was.
    assert_eq!(write(0, b"loud"), Reply::err(Error::Invalid));
    assert_eq!(write(0, b"8"), Reply::err(Error::Invalid));
    assert_eq!(write(1, b"debug"), Reply::err(Error::Invalid));
    assert_eq!(read(), "info");
}

#[test]
fn at_debug_every_op_on_a_resource_is_one_line() {
    let kernel = Kernel::new(tick, Heap::default, Limits::default());
    let session = kernel.session_new();
    let (caller, client) = kernel.connect(session, b"").unwrap();
    set_level(&kernel, "debug");

    // The capability cell of this link: a Resource the Kernel serves
    // itself, so the trace needs no Module.
    let look = Op::Lookup { rd: client, name: "capability".to_string() };
    let Out::Rd(cell) = ok(&kernel, caller, 1, look) else { panic!("lookup") };
    let open = Op::Open { rd: cell, rights: Rights::READ | Rights::WRITE };
    let Out::Handle(h) = ok(&kernel, caller, 2, open) else { panic!("open") };
    ok(&kernel, caller, 3, Op::Write { h, at: 0, bytes: b"hi".to_vec() });
    ok(&kernel, caller, 4, Op::Read { h, at: 0, len: 64 });
    // A handle that holds no WRITE: the refusal is the same one line.
    let open = Op::Open { rd: cell, rights: Rights::READ };
    let Out::Handle(r) = ok(&kernel, caller, 5, open) else { panic!("open") };
    let write = Op::Write { h: r, at: 0, bytes: b"no".to_vec() };
    assert_eq!(kernel.submit(caller, 6, write), Reply::err(Error::Denied));

    let said = texts(&kernel);
    let one = |head: String| {
        let mut found = said.iter().filter(|text| text.starts_with(&head));
        let line = found.next().unwrap_or_else(|| panic!("{head}: {said:?}"));
        assert!(found.next().is_none(), "{head} twice: {said:?}");
        measured(line)
    };
    let (c, rd) = (caller.0, cell.0);
    assert_eq!(
        one(format!("op=lookup caller={c} rd={}", client.0)),
        format!("op=lookup caller={c} rd={} name=capability ok={rd}", client.0)
    );
    assert_eq!(
        one(format!("op=open caller={c} rd={rd} rights=rw")),
        format!("op=open caller={c} rd={rd} rights=rw ok={}", h.0)
    );
    assert_eq!(
        one(format!("op=write caller={c} rd={rd} h={}", h.0)),
        format!("op=write caller={c} rd={rd} h={} at=0 len=2 ok=2", h.0)
    );
    assert_eq!(
        one(format!("op=read caller={c} rd={rd}")),
        format!("op=read caller={c} rd={rd} h={} at=0 len=64 ok=2", h.0)
    );
    assert_eq!(
        one(format!("op=write caller={c} rd={rd} h={}", r.0)),
        format!("op=write caller={c} rd={rd} h={} at=0 len=2 err=Denied", r.0)
    );
    // The write moved the cell, so the event it emitted is a line too.
    let event = format!("event rd={rd} kind=modify seq=");
    assert!(said.iter().any(|text| text.starts_with(&event)), "{said:?}");

    // Nothing the log's own reading does reaches the log: neither the
    // ops on it nor the ops on the level beside it.
    let traced = || said.iter().filter(|text| text.starts_with("op="));
    let quiet = |rd: Rd| format!(" rd={} ", rd.0);
    assert!(traced().all(|text| !text.contains(&quiet(LOG))), "{said:?}");
    assert!(traced().all(|text| !text.contains(&quiet(LOGLEVEL))), "{said:?}");
    assert_eq!(texts(&kernel).len(), said.len(), "reading the log is quiet");
}

#[test]
fn at_info_no_trace_is_kept() {
    let kernel = Kernel::new(tick, Heap::default, Limits::default());
    let session = kernel.session_new();
    let (caller, client) = kernel.connect(session, b"").unwrap();
    let look = Op::Lookup { rd: client, name: "capability".to_string() };
    let Out::Rd(cell) = ok(&kernel, caller, 1, look) else { panic!("lookup") };
    let open = Op::Open { rd: cell, rights: Rights::WRITE };
    let Out::Handle(h) = ok(&kernel, caller, 2, open) else { panic!("open") };
    ok(&kernel, caller, 3, Op::Write { h, at: 0, bytes: b"hi".to_vec() });

    let said = texts(&kernel);
    let traced =
        |text: &String| text.starts_with("op=") || text.starts_with("event ");
    assert!(!said.iter().any(traced), "{said:?}");
    assert_eq!(said.len(), 1, "the connect line alone: {said:?}");
}

/// The `connect` line of a link, which its `disconnect` answers.
fn connected(session: Rd, client: Rd) -> String {
    format!("connect session={} client={}", session.0, client.0)
}

#[test]
fn a_link_that_ends_says_it_disconnected() {
    let kernel = Kernel::new(tick, Heap::default, Limits::default());
    let session = kernel.session_new();
    let (caller, client) = kernel.connect(session, b"").unwrap();
    kernel.caller_end(caller);
    // A named caller keeps its client rd for the next `Connect` and
    // says its link ended all the same.
    let named = kernel.session_new();
    let (caller, kept) = kernel.connect(named, b"tui").unwrap();
    kernel.caller_end(caller);

    let said = texts(&kernel);
    let gone = |session: Rd, client: Rd| {
        format!("disconnect session={} client={}", session.0, client.0)
    };
    let at = |text: &String| said.iter().position(|said| said == text);
    assert!(at(&gone(session, client)).is_some(), "{said:?}");
    assert!(at(&gone(named, kept)).is_some(), "{said:?}");
    assert!(at(&gone(session, client)) > at(&connected(session, client)));
}

/// A Module with one cell and one method on it, so a `call` has a pair.
#[derive(Default)]
struct Api(core::cell::Cell<Option<Rd>>);

fn decl() -> Decl {
    Decl {
        api: "say".to_string(),
        ver: 1,
        methods: vec![Method {
            number: 2,
            name: "hello".to_string(),
            input_name: None,
            input: Ty::Unit,
            output: Ty::Unit,
            right: Rights::READ,
        }],
        records: Vec::new(),
    }
}

impl Module for Api {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let cell = ctx.register(DriverId(0), home, Some("cell"))?;
        ctx.report(cell, Flags::READ, &[decl()])?;
        self.0.set(Some(cell));
        Ok(())
    }
}

impl Driver for Api {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn call(
        &self,
        _: &dyn Calls,
        _: u64,
        _: Hash,
        _: u16,
        _: &[u8],
    ) -> Reply<Vec<u8>> {
        Reply::ok(Vec::new())
    }
}

#[test]
fn a_call_is_a_pair_and_the_reply_says_what_it_took() {
    let kernel = Kernel::new(tick, Heap::default, Limits::default());
    let module = Rc::new(Api::default());
    kernel.load("api", module.clone(), vec![module.clone()]).unwrap();
    let cell = module.0.get().unwrap();
    set_level(&kernel, "debug");

    let caller = kernel.caller_new();
    let open = Op::Open { rd: cell, rights: Rights::READ };
    let Out::Handle(h) = ok(&kernel, caller, 1, open) else { panic!("open") };
    let api = decl().hash();
    let hello = Op::Call { h, api, method: 2, input: Vec::new() };
    ok(&kernel, caller, 2, hello);

    let said = texts(&kernel);
    let pair: Vec<&String> =
        said.iter().filter(|text| text.starts_with("call ")).collect();
    let head = format!("call rd={} api={:016x} method=2", cell.0, api.0);
    assert_eq!(pair.len(), 2, "{said:?}");
    assert_eq!(*pair[0], format!("{head} dispatch"));
    // The round trip is the Kernel's own measure, in the clock's unit.
    assert_eq!(measured(pair[1]), format!("{head} reply"));
}
