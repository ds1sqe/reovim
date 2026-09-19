//! What the Kernel's cases share: a clock that moves, the `Out` an op
//! was answered with, `/sys/log` read and decoded, and a Kernel with
//! one Module loaded and one caller of its own whose ops are numbered
//! in the order they are submitted. Each test binary compiles this on
//! its own and uses what it needs.
#![allow(dead_code)]

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use reovim_kernel_interface::{
    Caller, Calls, Driver, Error, Event, Handle, Kind, Module, ModuleId, Op,
    Out, Rd, Reply, Rights, Token,
};
use reovim_kernel_runtime::{Heap, Kernel, LOG, Limits};

static CLOCK: AtomicU64 = AtomicU64::new(1);

/// A clock that moves, so a measured span is more than nothing.
pub fn tick() -> u64 {
    CLOCK.fetch_add(1, Ordering::Relaxed)
}

/// `op` from `caller` at `seq`, answered at once: what a case that
/// numbers its own ops asks through.
pub fn ok(kernel: &Kernel, caller: Caller, seq: u64, op: Op) -> Out {
    match kernel.submit(caller, seq, op) {
        Reply::Ready(Ok(out)) => out,
        other => panic!("{other:?}"),
    }
}

/// Whole entries of the encoding `/sys/log` serves: `(mark, level,
/// text)` per entry.
pub fn entries(bytes: &[u8]) -> Vec<(String, u8, String)> {
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
pub fn written(kernel: &Kernel) -> Vec<(String, u8, String)> {
    let caller = kernel.caller_new();
    let open = Op::Open { rd: LOG, rights: Rights::READ };
    let Out::Handle(h) = ok(kernel, caller, 1, open) else { panic!("open") };
    let read = Op::Read { h, at: 1, len: 1 << 20 };
    let Out::Bytes(bytes) = ok(kernel, caller, 2, read) else { panic!("read") };
    entries(&bytes)
}

/// A Kernel with one Module loaded and one caller of its own.
pub struct Rig {
    pub kernel: Kernel,
    module: ModuleId,
    pub caller: Caller,
    seq: Cell<u64>,
    cursor: Cell<u64>,
}

impl Rig {
    /// `module` loaded under `name` with `drivers`, on a Kernel whose
    /// clock is `clock` and whose bounds are `limits`. A clock that
    /// stands still is `|| 1`.
    pub fn over(
        clock: fn() -> u64,
        limits: Limits,
        name: &str,
        module: Rc<dyn Module>,
        drivers: Vec<Rc<dyn Driver>>,
    ) -> Self {
        let kernel = Kernel::new(clock, Heap::default, limits);
        let module = kernel.load(name, module, drivers).unwrap();
        let caller = kernel.caller_new();
        let (seq, cursor) = (Cell::new(0), Cell::new(0));
        Self { kernel, module, caller, seq, cursor }
    }

    /// `op` from the rig's own caller, with the next sequence.
    pub fn submit(&self, op: Op) -> Reply<Out> {
        self.seq.set(self.seq.get() + 1);
        self.kernel.submit(self.caller, self.seq.get(), op)
    }

    pub fn ok(&self, op: Op) -> Out {
        match self.submit(op) {
            Reply::Ready(Ok(out)) => out,
            other => panic!("expected Ready(Ok), got {other:?}"),
        }
    }

    pub fn err(&self, op: Op) -> Error {
        match self.submit(op) {
            Reply::Ready(Err(error)) => error,
            other => panic!("expected Ready(Err), got {other:?}"),
        }
    }

    pub fn waits(&self, op: Op) -> Token {
        match self.submit(op) {
            Reply::Pending(token) => token,
            other @ Reply::Ready(_) => {
                panic!("expected Pending, got {other:?}")
            }
        }
    }

    /// The Module's own side of the Kernel.
    pub fn calls(&self) -> impl Calls + '_ {
        self.kernel.calls(self.module)
    }

    pub fn lookup(&self, rd: Rd, name: &str) -> Rd {
        match self.ok(Op::Lookup { rd, name: name.to_string() }) {
            Out::Rd(rd) => rd,
            other => panic!("{other:?}"),
        }
    }

    pub fn open(&self, rd: Rd, rights: Rights) -> Handle {
        match self.ok(Op::Open { rd, rights }) {
            Out::Handle(h) => h,
            other => panic!("{other:?}"),
        }
    }

    /// The events since `cursor`, and the cursor to ask from next.
    pub fn since(&self, cursor: u64) -> (Vec<Event>, u64) {
        match self.ok(Op::Since { cursor }) {
            Out::Events { events, cursor, .. } => (events, cursor),
            other => panic!("{other:?}"),
        }
    }

    /// The `(rd, kind, child)` of the events since `cursor`.
    pub fn kinds(&self, cursor: u64) -> Vec<(Rd, Kind, Option<Rd>)> {
        kinds_of(&self.since(cursor).0)
    }

    /// The same from where this rig last looked, which is what a case
    /// that never names a cursor asks for.
    pub fn seen(&self) -> Vec<(Rd, Kind, Option<Rd>)> {
        let (events, cursor) = self.since(self.cursor.get());
        self.cursor.set(cursor);
        kinds_of(&events)
    }
}

fn kinds_of(events: &[Event]) -> Vec<(Rd, Kind, Option<Rd>)> {
    events.iter().map(|e| (e.rd, e.kind, e.child)).collect()
}
