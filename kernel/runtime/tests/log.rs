//! `/sys/log` is a ring: past `log_ring` entries the oldest go, and a
//! read from before the oldest starts at the oldest.

use std::rc::Rc;

use reovim_kernel_interface::{
    Caller, Calls, Driver, Error, Module, Op, Out, Rd, Reply, Rights,
};
use reovim_kernel_runtime::{Kernel, LOG, Limits};

fn ok(kernel: &Kernel, caller: Caller, seq: u64, op: Op) -> Out {
    match kernel.submit(caller, seq, op) {
        Reply::Ready(Ok(out)) => out,
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_oldest_entries_are_trimmed() {
    let limits = Limits { log_ring: 3, ..Limits::default() };
    let kernel = Kernel::new(|| 7, limits);
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

/// Whole entries of the encoding `/sys/log` serves: `(mark, level,
/// text)` per entry.
fn entries(bytes: &[u8]) -> Vec<(String, u8, String)> {
    let mut out = Vec::new();
    let mut rest = bytes;
    while rest.len() > 20 {
        let blob = |at: usize, rest: &[u8]| {
            let len = u32::from_le_bytes(rest[at..at + 4].try_into().unwrap());
            (at + 4, len as usize)
        };
        let (at, len) = blob(16, rest);
        let mark = String::from_utf8(rest[at..at + len].to_vec()).unwrap();
        let level = rest[at + len];
        let (at, len) = blob(at + len + 1, rest);
        let text = String::from_utf8(rest[at..at + len].to_vec()).unwrap();
        out.push((mark, level, text));
        rest = &rest[at + len..];
    }
    out
}

/// Everything in the log now, read by a caller of its own.
fn written(kernel: &Kernel) -> Vec<(String, u8, String)> {
    let caller = kernel.caller_new();
    let open = Op::Open { rd: LOG, rights: Rights::READ };
    let Out::Handle(h) = ok(kernel, caller, 1, open) else { panic!("open") };
    let read = Op::Read { h, at: 1, len: 1 << 20 };
    let Out::Bytes(bytes) = ok(kernel, caller, 2, read) else { panic!("read") };
    entries(&bytes)
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
    let kernel = Kernel::new(|| 7, Limits::default());
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
    let kernel = Kernel::new(|| 7, limits);
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
