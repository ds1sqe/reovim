//! Both Drivers in one Kernel: a Buffer over a fs file.

use std::path::PathBuf;

use reovim_host::clock;
use reovim_kernel_interface::{
    Booted, Caller, Error, Handle, Kind, Op, Out, Rd, Reply, Rights,
};
use reovim_kernel_runtime::{Kernel, Limits, ROOT};
use reovim_module_buffer::{attach_decl, buffer_decl};

/// A directory of its own for one test, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(test: &str) -> Self {
        let name = format!("reovim-buffer-{}-{test}", std::process::id());
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Rig {
    dir: Dir,
    kernel: Kernel,
    caller: Caller,
    seq: std::cell::Cell<u64>,
    /// `/module/fs/root`.
    root: Rd,
    /// `/module/buffer/attach`.
    attach: Rd,
}

fn load(kernel: &Kernel, name: &str, booted: &Booted) {
    kernel.load(name, booted.module.as_ref(), booted.drivers.clone()).unwrap();
}

impl Rig {
    fn new(test: &str) -> Self {
        let dir = Dir::new(test);
        let kernel = Kernel::new(clock::now, Limits::default());
        load(&kernel, "fs", &reovim_module_fs::boot(dir.0.to_str().unwrap()));
        load(&kernel, "buffer", &reovim_module_buffer::boot(""));
        let caller = kernel.caller_new();
        let seq = std::cell::Cell::new(0);
        let mut rig =
            Self { dir, kernel, caller, seq, root: ROOT, attach: ROOT };
        let modules = rig.lookup(ROOT, "module");
        rig.root = rig.lookup(rig.lookup(modules, "fs"), "root");
        rig.attach = rig.lookup(rig.lookup(modules, "buffer"), "attach");
        rig
    }

    fn file(&self, name: &str, content: &[u8]) -> Rd {
        std::fs::write(self.dir.0.join(name), content).unwrap();
        self.lookup(self.root, name)
    }

    fn disk(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.dir.0.join(name)).unwrap()
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

    fn lookup(&self, rd: Rd, name: &str) -> Rd {
        match self.ok(Op::Lookup { rd, name: name.to_string() }) {
            Out::Rd(rd) => rd,
            other => panic!("{other:?}"),
        }
    }

    fn open(&self, rd: Rd, rights: Rights) -> Handle {
        match self.ok(Op::Open { rd, rights }) {
            Out::Handle(h) => h,
            other => panic!("{other:?}"),
        }
    }

    fn read(&self, rd: Rd) -> Vec<u8> {
        let h = self.open(rd, Rights::READ);
        match self.ok(Op::Read { h, at: 0, len: 1024 }) {
            Out::Bytes(bytes) => bytes,
            other => panic!("{other:?}"),
        }
    }

    fn write(&self, h: Handle, at: u64, bytes: &[u8]) -> Reply<Out> {
        self.submit(Op::Write { h, at, bytes: bytes.to_vec() })
    }

    fn name(&self, rd: Rd) -> String {
        match self.ok(Op::Info { rd }) {
            Out::Info(info) => info.name.unwrap(),
            other => panic!("{other:?}"),
        }
    }

    /// A watched handle on `rd`, and the cursor after its opening.
    fn watch(&self, rd: Rd) -> (Handle, u64) {
        let h = self.open(rd, Rights::READ | Rights::WRITE);
        self.ok(Op::Watch { h, on: true });
        (h, self.since(0).1)
    }

    /// `(rd, kind)` of the events after `cursor`, and the next cursor.
    fn since(&self, cursor: u64) -> (Vec<(Rd, Kind)>, u64) {
        match self.ok(Op::Since { cursor }) {
            Out::Events { events, cursor } => {
                (events.iter().map(|e| (e.rd, e.kind)).collect(), cursor)
            }
            other => panic!("{other:?}"),
        }
    }

    fn attach(&self, source: Rd) -> Reply<Out> {
        let h = self.open(self.attach, Rights::WRITE);
        let input = source.0.to_le_bytes().to_vec();
        let api = attach_decl().hash();
        self.submit(Op::Call { h, api, method: 0, input })
    }

    fn buffer(&self, source: Rd) -> Rd {
        match self.attach(source) {
            Reply::Ready(Ok(Out::Bytes(bytes))) => {
                Rd(u64::from_le_bytes(bytes.try_into().unwrap()))
            }
            other => panic!("{other:?}"),
        }
    }

    fn save(&self, h: Handle) -> Reply<Out> {
        let api = buffer_decl().hash();
        self.submit(Op::Call { h, api, method: 0, input: Vec::new() })
    }
}

#[test]
fn the_draft_reaches_disk_on_save() {
    let rig = Rig::new("save");
    let source = rig.file("f", b"abc");
    let buffer = rig.buffer(source);
    assert_eq!(rig.name(buffer), source.0.to_string());
    assert_eq!(rig.read(buffer), b"abc");

    let (h, cursor) = rig.watch(buffer);
    rig.watch(source);
    assert_eq!(rig.write(h, 1, b"xyz"), Reply::ok(Out::Len(3)));
    assert_eq!(rig.read(buffer), b"axyz");
    assert_eq!(rig.disk("f"), b"abc");
    let (events, cursor) = rig.since(cursor);
    assert_eq!(events, [(buffer, Kind::Modify)]);

    assert_eq!(rig.save(h), Reply::ok(Out::Bytes(Vec::new())));
    assert_eq!(rig.disk("f"), b"axyz");
    let (events, _) = rig.since(cursor);
    assert_eq!(events, [(source, Kind::Modify)]);

    let pointer = rig.lookup(buffer, "source");
    assert_eq!(rig.read(pointer), source.0.to_le_bytes());
    let past_end = rig.write(h, 9, b"x");
    assert_eq!(past_end, Reply::err(Error::Invalid));
}

#[test]
fn one_buffer_per_source_and_the_rd_is_checked() {
    let rig = Rig::new("once");
    let source = rig.file("f", b"");
    assert_eq!(rig.buffer(source), rig.buffer(source));
    assert_eq!(rig.attach(Rd(999_999)), Reply::err(Error::Invalid));
}

#[test]
fn a_read_only_source_attaches_but_never_saves() {
    let rig = Rig::new("readonly");
    let source = rig.file("f", b"abc");
    let pointer = rig.lookup(rig.buffer(source), "source");

    let buffer = rig.buffer(pointer);
    assert_eq!(rig.read(buffer), source.0.to_le_bytes());
    let h = rig.open(buffer, Rights::WRITE);
    assert_eq!(rig.save(h), Reply::err(Error::Denied));
}
