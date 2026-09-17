//! The clipboard Driver through the real Kernel, over a pair of its
//! own instead of a tool.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use reovim_host::clock;
use reovim_kernel_interface::{
    Driver, Error, Handle, Op, Out, Rd, Reply, Rights,
};
use reovim_kernel_runtime::{Heap, Kernel, Limits, ROOT};
use reovim_module_clipboard::Clipboard;

struct Rig {
    kernel: Kernel,
    caller: reovim_kernel_interface::Caller,
    seq: Cell<u64>,
    store: Rc<RefCell<Option<Vec<u8>>>>,
}

impl Rig {
    /// A Kernel with the Module over `store`: `None` there is a host
    /// without a tool.
    fn new(store: Option<Vec<u8>>) -> Self {
        let store = Rc::new(RefCell::new(store));
        let (reads, writes) = (store.clone(), store.clone());
        let read = move || {
            reads.borrow().clone().ok_or(reovim_host::Error::Unsupported)
        };
        let write = move |bytes: &[u8]| {
            let mut store = writes.borrow_mut();
            store
                .as_mut()
                .map(|s| *s = bytes.to_vec())
                .ok_or(reovim_host::Error::Unsupported)
        };
        let module = Rc::new(Clipboard::new(Box::new(read), Box::new(write)));
        let driver: Rc<dyn Driver> = module.clone();
        let kernel = Kernel::new(clock::now, Heap::default, Limits::default());
        kernel.load("clipboard", module, vec![driver]).unwrap();
        let caller = kernel.caller_new();
        Self { kernel, caller, seq: Cell::new(0), store }
    }

    fn submit(&self, op: Op) -> Reply<Out> {
        self.seq.set(self.seq.get() + 1);
        self.kernel.submit(self.caller, self.seq.get(), op)
    }

    fn ok(&self, op: Op) -> Out {
        match self.submit(op) {
            Reply::Ready(Ok(out)) => out,
            other => panic!("expected Ready(Ok), got {other:?}"),
        }
    }

    fn err(&self, op: Op) -> Error {
        match self.submit(op) {
            Reply::Ready(Err(error)) => error,
            other => panic!("expected Ready(Err), got {other:?}"),
        }
    }

    fn lookup(&self, rd: Rd, name: &str) -> Rd {
        match self.ok(Op::Lookup { rd, name: name.to_string() }) {
            Out::Rd(rd) => rd,
            other => panic!("{other:?}"),
        }
    }

    /// The `text` rd, opened.
    fn text(&self) -> Handle {
        let module = self.lookup(ROOT, "module");
        let home = self.lookup(module, "clipboard");
        let text = self.lookup(home, "text");
        let rights = Rights::READ.union(Rights::WRITE);
        match self.ok(Op::Open { rd: text, rights }) {
            Out::Handle(h) => h,
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn the_text_rd_reads_and_writes_the_clipboard_whole() {
    let rig = Rig::new(Some(b"one".to_vec()));
    let h = rig.text();
    assert_eq!(
        rig.ok(Op::Read { h, at: 0, len: 8 }),
        Out::Bytes(b"one".to_vec())
    );
    assert_eq!(
        rig.ok(Op::Read { h, at: 1, len: 1 }),
        Out::Bytes(b"n".to_vec())
    );
    assert_eq!(rig.ok(Op::Read { h, at: 3, len: 8 }), Out::Bytes(Vec::new()));
    let bytes = b"two\n".to_vec();
    assert_eq!(
        rig.ok(Op::Write { h, at: 0, bytes: bytes.clone() }),
        Out::Len(4)
    );
    assert_eq!(rig.store.borrow().as_deref(), Some(b"two\n".as_slice()));
    assert_eq!(
        rig.err(Op::Write { h, at: 1, bytes }),
        Error::Invalid,
        "whole or not at all"
    );
}

#[test]
fn without_a_tool_the_rd_is_unsupported() {
    let rig = Rig::new(None);
    let h = rig.text();
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 8 }), Error::Unsupported);
    assert_eq!(
        rig.err(Op::Write { h, at: 0, bytes: b"x".to_vec() }),
        Error::Unsupported
    );
}
