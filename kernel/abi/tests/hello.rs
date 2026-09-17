#![allow(unsafe_code)]
//! modules/hello/hello.c built by the system cc, loaded through the
//! host and this adapter into a real Kernel, then unloaded.

use std::cell::Cell;
use std::process::Command;
use std::rc::Rc;

use reovim_host::native;
use reovim_kernel_abi::{RvModule, boot};
use reovim_kernel_interface::{
    Caller, Error, Kind, Op, Out, Rd, Reply, Rights,
};
use reovim_kernel_runtime::{Heap, Kernel, Limits, ROOT};

type Entry = unsafe extern "C" fn() -> *const RvModule;

fn build(dir: &std::path::Path) -> String {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    let out = dir.join("hello.rvm");
    let built = Command::new("cc")
        .args(["-shared", "-fPIC", "-I"])
        .arg(format!("{root}/abi/include"))
        .arg(format!("{root}/../modules/hello/hello.c"))
        .arg("-o")
        .arg(&out)
        .status()
        .expect("cc");
    assert!(built.success());
    out.to_str().unwrap().to_string()
}

struct Rig {
    kernel: Kernel,
    caller: Caller,
    seq: Cell<u64>,
}

impl Rig {
    fn ok(&self, op: Op) -> Out {
        self.seq.set(self.seq.get() + 1);
        match self.kernel.submit(self.caller, self.seq.get(), op) {
            Reply::Ready(Ok(out)) => out,
            other => panic!("{other:?}"),
        }
    }

    fn resolve(&self, path: &str) -> Result<Rd, Error> {
        path.split('/').filter(|n| !n.is_empty()).try_fold(ROOT, |rd, n| {
            self.seq.set(self.seq.get() + 1);
            let op = Op::Lookup { rd, name: n.to_string() };
            match self.kernel.submit(self.caller, self.seq.get(), op) {
                Reply::Ready(Ok(Out::Rd(rd))) => Ok(rd),
                Reply::Ready(Err(e)) => Err(e),
                other => panic!("{other:?}"),
            }
        })
    }
}

#[test]
fn hello_loads_serves_and_unloads() {
    let dir = std::env::temp_dir()
        .join(format!("reovim-hello-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = build(&dir);
    assert_eq!(
        native::load("/nowhere.rvm").err(),
        Some(reovim_host::Error::NotFound)
    );

    let library = native::load(&path).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let entry = native::entry(&library).unwrap();
    // SAFETY: the symbol is `rv_module_entry` of a file built from hello.c.
    let entry: Entry = unsafe { std::mem::transmute(entry) };
    let descriptor = unsafe { entry() };
    let keep = Rc::new(library);
    let weak = Rc::downgrade(&keep);
    let (name, booted) = unsafe { boot(descriptor, keep) }.unwrap();
    assert_eq!(name, "hello");

    let kernel = Kernel::new(|| 1, Heap::default, Limits::default());
    kernel.load(&name, booted.module, booted.drivers).unwrap();
    let caller = kernel.caller_new();
    let rig = Rig { kernel, caller, seq: Cell::new(0) };
    let count = rig.resolve("/module/hello/count").unwrap();
    let Out::Handle(h) =
        rig.ok(Op::Open { rd: count, rights: Rights::READ | Rights::WRITE })
    else {
        panic!("open")
    };
    rig.ok(Op::Watch { h, on: true });
    let Out::Events { cursor, .. } = rig.ok(Op::Since { cursor: 0 }) else {
        panic!()
    };
    assert_eq!(
        rig.ok(Op::Read { h, at: 0, len: 16 }),
        Out::Bytes(b"0\n".to_vec())
    );
    assert_eq!(
        rig.ok(Op::Write { h, at: 0, bytes: b"42".to_vec() }),
        Out::Len(2)
    );
    assert_eq!(
        rig.ok(Op::Read { h, at: 0, len: 16 }),
        Out::Bytes(b"42\n".to_vec())
    );
    assert_eq!(rig.ok(Op::Read { h, at: 3, len: 16 }), Out::Bytes(Vec::new()));
    let Out::Events { events, .. } = rig.ok(Op::Since { cursor }) else {
        panic!()
    };
    assert_eq!((events[0].rd, events[0].kind), (count, Kind::Modify));
    rig.ok(Op::Close { h });

    assert_eq!(rig.kernel.unload("hello"), Ok(()));
    assert_eq!(rig.resolve("/module/hello"), Err(Error::Invalid));
    assert!(weak.upgrade().is_none(), "the library closed with the Module");
}
