//! The fs Driver on a fresh directory, through the real Kernel.

use std::path::PathBuf;
use std::rc::Rc;

use reovim_host::clock;
use reovim_kernel_interface::{
    Driver, Error, Flags, Handle, Kind, Op, Out, Rd, Reply, Rights,
};
use reovim_kernel_runtime::{Kernel, Limits, ROOT};
use reovim_module_fs::{Fs, decl};

/// A directory of its own for one test, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(test: &str) -> Self {
        let name = format!("reovim-fs-{}-{test}", std::process::id());
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
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
    caller: reovim_kernel_interface::Caller,
    seq: std::cell::Cell<u64>,
    root: Rd,
}

impl Rig {
    fn new(test: &str) -> Self {
        let dir = Dir::new(test);
        let kernel = Kernel::new(clock::now, Limits::default());
        let fs = Rc::new(Fs::new(dir.0.to_str().unwrap().to_string()));
        let driver: Rc<dyn Driver> = fs.clone();
        kernel.load("fs", fs, vec![driver]).unwrap();
        let caller = kernel.caller_new();
        let seq = std::cell::Cell::new(0);
        let mut rig = Self { dir, kernel, caller, seq, root: ROOT };
        let module = rig.lookup(ROOT, "module");
        let home = rig.lookup(module, "fs");
        rig.root = rig.lookup(home, "root");
        rig
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

    fn open(&self, rd: Rd, rights: Rights) -> Handle {
        match self.ok(Op::Open { rd, rights }) {
            Out::Handle(h) => h,
            other => panic!("{other:?}"),
        }
    }

    fn read(&self, h: Handle) -> Vec<u8> {
        match self.ok(Op::Read { h, at: 0, len: 1024 }) {
            Out::Bytes(bytes) => bytes,
            other => panic!("{other:?}"),
        }
    }

    fn write(&self, h: Handle, bytes: &[u8]) -> u32 {
        match self.ok(Op::Write { h, at: 0, bytes: bytes.to_vec() }) {
            Out::Len(len) => len,
            other => panic!("{other:?}"),
        }
    }

    fn flags(&self, rd: Rd) -> Flags {
        match self.ok(Op::Info { rd }) {
            Out::Info(info) => info.flags,
            other => panic!("{other:?}"),
        }
    }

    fn name(&self, rd: Rd) -> String {
        match self.ok(Op::Info { rd }) {
            Out::Info(info) => info.name.unwrap(),
            other => panic!("{other:?}"),
        }
    }

    /// Kinds of the events after `cursor`, and the next cursor.
    fn since(&self, cursor: u64) -> (Vec<Kind>, u64) {
        match self.ok(Op::Since { cursor }) {
            Out::Events { events, cursor } => {
                (events.iter().map(|e| e.kind).collect(), cursor)
            }
            other => panic!("{other:?}"),
        }
    }

    fn create(&self, h: Handle, name: &str) -> Reply<Out> {
        let mut input =
            u32::try_from(name.len()).unwrap().to_le_bytes().to_vec();
        input.extend_from_slice(name.as_bytes());
        let api = decl().hash();
        self.submit(Op::Call { h, api, method: 0, input })
    }
}

fn rd_of(reply: Reply<Out>) -> Rd {
    match reply {
        Reply::Ready(Ok(Out::Bytes(bytes))) => {
            Rd(u64::from_le_bytes(bytes.try_into().unwrap()))
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn lookup_mirrors_files_and_directories() {
    let rig = Rig::new("lookup");
    std::fs::write(rig.dir.path("a.txt"), b"abc").unwrap();
    std::fs::create_dir(rig.dir.path("sub")).unwrap();

    let file = rig.lookup(rig.root, "a.txt");
    assert_eq!(rig.flags(file), Flags::READ | Flags::WRITE);
    let sub = rig.lookup(rig.root, "sub");
    assert_eq!(rig.flags(sub), Flags::READ | Flags::WRITE | Flags::CONTAINER);
    assert_eq!(
        rig.flags(rig.root),
        Flags::READ | Flags::WRITE | Flags::CONTAINER
    );

    let escape = Op::Lookup { rd: rig.root, name: "..".to_string() };
    assert_eq!(rig.err(escape), Error::Invalid);
    let below_file = Op::Lookup { rd: file, name: "x".to_string() };
    assert_eq!(rig.err(below_file), Error::Unsupported);
}

#[test]
fn children_lists_the_directory_once() {
    let rig = Rig::new("children");
    std::fs::write(rig.dir.path("a"), b"").unwrap();
    std::fs::write(rig.dir.path("b"), b"").unwrap();
    let a = rig.lookup(rig.root, "a");

    let Out::Rds(rds) = rig.ok(Op::Children { rd: rig.root }) else { panic!() };
    let mut names: Vec<String> = rds.iter().map(|r| rig.name(*r)).collect();
    names.sort();
    assert_eq!(names, ["a", "b"]);
    assert!(rds.contains(&a));
}

#[test]
fn read_write_and_a_change_behind_the_driver() {
    let rig = Rig::new("rw");
    let path = rig.dir.path("f");
    std::fs::write(&path, b"abc").unwrap();
    let file = rig.lookup(rig.root, "f");
    let h = rig.open(file, Rights::READ | Rights::WRITE);
    rig.ok(Op::Watch { h, on: true });
    let (_, cursor) = rig.since(0);

    assert_eq!(rig.read(h), b"abc");
    assert_eq!(rig.write(h, b"xyz!"), 4);
    assert_eq!(std::fs::read(&path).unwrap(), b"xyz!");
    let (kinds, cursor) = rig.since(cursor);
    assert_eq!(kinds, [Kind::Modify]);

    // A read after the Driver's own write reports nothing new.
    assert_eq!(rig.read(h), b"xyz!");
    let (kinds, cursor) = rig.since(cursor);
    assert_eq!(kinds, []);

    // The host stamps files with a coarse clock; give it a tick.
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&path, b"edited").unwrap();
    assert_eq!(rig.read(h), b"edited");
    let (kinds, _) = rig.since(cursor);
    assert_eq!(kinds, [Kind::Modify]);
}

#[test]
fn a_name_not_on_disk_is_an_empty_file_until_written() {
    let rig = Rig::new("new");
    let path = rig.dir.path("new");
    let file = rig.lookup(rig.root, "new");
    assert_eq!(rig.flags(file), Flags::READ | Flags::WRITE);
    let h = rig.open(file, Rights::READ | Rights::WRITE);
    assert_eq!(rig.read(h), b"");
    assert!(!path.exists(), "nothing on disk before a write");

    assert_eq!(rig.write(h, b"first"), 5);
    assert_eq!(std::fs::read(&path).unwrap(), b"first");
    assert_eq!(rig.read(h), b"first");
    let root = rig.open(rig.root, Rights::WRITE);
    assert_eq!(rig.create(root, "new"), Reply::err(Error::Invalid));

    // Once seen, its removal is Gone as for any file.
    std::fs::remove_file(&path).unwrap();
    assert_eq!(rig.err(Op::Read { h, at: 0, len: 8 }), Error::Gone);
}

#[test]
fn create_through_call_registers_the_file() {
    let rig = Rig::new("create");
    let h = rig.open(rig.root, Rights::WRITE);

    let made = rd_of(rig.create(h, "new.txt"));
    assert_eq!(rig.name(made), "new.txt");
    assert!(rig.dir.path("new.txt").is_file());
    assert_eq!(rig.lookup(rig.root, "new.txt"), made);

    assert_eq!(rig.create(h, "new.txt"), Reply::err(Error::Invalid));
    assert_eq!(rig.create(h, "bad/name"), Reply::err(Error::Invalid));

    let read_only = rig.open(rig.root, Rights::READ);
    assert_eq!(rig.create(read_only, "x"), Reply::err(Error::Denied));
}

#[test]
fn a_file_deleted_on_disk_is_gone_and_unregistered() {
    let rig = Rig::new("gone");
    let path = rig.dir.path("f");
    std::fs::write(&path, b"abc").unwrap();
    let file = rig.lookup(rig.root, "f");
    let h = rig.open(file, Rights::READ);
    assert_eq!(rig.read(h), b"abc");

    std::fs::remove_file(&path).unwrap();
    let read = || Op::Read { h, at: 0, len: 8 };
    assert_eq!(rig.err(read()), Error::Gone);
    assert_eq!(rig.err(Op::Info { rd: file }), Error::Invalid);
    assert_eq!(rig.err(read()), Error::Gone);
}

#[test]
fn a_whole_write_through_a_symlink_keeps_the_link() {
    let rig = Rig::new("symlink");
    let target = rig.dir.path("target");
    std::fs::write(&target, b"old").unwrap();
    std::os::unix::fs::symlink(&target, rig.dir.path("link")).unwrap();
    let link = rig.lookup(rig.root, "link");
    let h = rig.open(link, Rights::READ | Rights::WRITE);
    assert_eq!(rig.write(h, b"new"), 3);
    assert!(
        std::fs::symlink_metadata(rig.dir.path("link")).unwrap().is_symlink()
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"new");
}
