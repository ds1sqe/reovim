//! `/sys/log` is a ring: past `log_ring` entries the oldest go, and a
//! read from before the oldest starts at the oldest.

use reovim_kernel_interface::{Caller, Op, Out, Reply, Rights};
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
