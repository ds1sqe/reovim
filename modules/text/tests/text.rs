//! Both Drivers in one Kernel: walkthrough 1 of 03-walkthroughs.md, a
//! text Buffer over a fs file.

use std::path::PathBuf;
use std::rc::Rc;

use reovim_host::clock;
use reovim_kernel_interface::{
    Booted, Caller, Error, Handle, Kind, Op, Out, Rd, Reply, Rights,
};
use reovim_kernel_runtime::{Kernel, Limits, ROOT};
use reovim_module_text::{Text, history, open, publish, text};
use reovim_sdk::{decode, encode};

/// A directory of its own for one test, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(test: &str) -> Self {
        let name = format!("reovim-text-{}-{test}", std::process::id());
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
    /// `/module/text/open`.
    open: Rd,
}

/// One node of `tree`: `(id, parent, label)`.
type Node = (u64, u64, String);

fn load(kernel: &Kernel, name: &str, booted: &Booted) {
    kernel.load(name, booted.module.clone(), booted.drivers.clone()).unwrap();
}

/// The `Tree`: its nodes as tuples, head, published.
fn tree_of(bytes: &[u8]) -> (Vec<Node>, u64, u64) {
    let tree: history::Tree = decode(bytes).unwrap();
    let nodes = tree.nodes.into_iter().map(|n| (n.id, n.parent, n.label));
    (nodes.collect(), tree.head, tree.published)
}

impl Rig {
    fn new(test: &str) -> Self {
        Self::with_limit(test, 64 << 20)
    }

    /// A text Driver holding at most `limit` bytes of states.
    fn with_limit(test: &str, limit: usize) -> Self {
        let dir = Dir::new(test);
        let kernel = Kernel::new(clock::now, Limits::default());
        load(&kernel, "fs", &reovim_module_fs::boot(dir.0.to_str().unwrap()));
        let driver = Rc::new(Text::with_limit(limit));
        let booted = Booted { module: driver.clone(), drivers: vec![driver] };
        load(&kernel, "text", &booted);
        let caller = kernel.caller_new();
        let seq = std::cell::Cell::new(0);
        let mut rig = Self { dir, kernel, caller, seq, root: ROOT, open: ROOT };
        let modules = rig.lookup(ROOT, "module");
        rig.root = rig.lookup(rig.lookup(modules, "fs"), "root");
        rig.open = rig.lookup(rig.lookup(modules, "text"), "open");
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

    fn call(
        &self,
        h: Handle,
        api: &str,
        method: u16,
        input: Vec<u8>,
    ) -> Reply<Out> {
        let api = match api {
            "open" => open::hash(),
            "text" => text::hash(),
            "history" => history::hash(),
            _ => publish::hash(),
        };
        self.submit(Op::Call { h, api, method, input })
    }

    fn open_call(&self, source: Rd) -> Reply<Out> {
        let h = self.open(self.open, Rights::WRITE);
        self.call(h, "open", 0, encode(&source))
    }

    fn buffer(&self, source: Rd) -> Rd {
        match self.open_call(source) {
            Reply::Ready(Ok(Out::Bytes(bytes))) => decode(&bytes).unwrap(),
            other => panic!("{other:?}"),
        }
    }

    fn replace(&self, h: Handle, at: u64, len: u64, text: &str) -> Reply<Out> {
        let edit = text::Edit { at, len, text: text.to_string() };
        self.call(h, "text", 0, encode(&edit))
    }

    /// `replace_many` with `expected`; each edit `(at, len, text)`.
    fn replace_many(
        &self,
        h: Handle,
        expected: u64,
        edits: &[(u64, u64, &str)],
    ) -> Reply<Out> {
        let edit = |&(at, len, text): &(u64, u64, &str)| text::Edit {
            at,
            len,
            text: text.to_string(),
        };
        let many =
            text::Many { expected, edits: edits.iter().map(edit).collect() };
        self.call(h, "text", 1, encode(&many))
    }

    /// `undo`, `redo` or `goto`.
    fn history(&self, h: Handle, method: u16, input: Vec<u8>) -> Reply<Out> {
        self.call(h, "history", method, input)
    }

    fn tree(&self, h: Handle) -> (Vec<Node>, u64, u64) {
        match self.call(h, "history", 0, Vec::new()) {
            Reply::Ready(Ok(Out::Bytes(bytes))) => tree_of(&bytes),
            other => panic!("{other:?}"),
        }
    }

    fn save(&self, h: Handle) -> Reply<Out> {
        self.call(h, "publish", 0, Vec::new())
    }
}

const DONE: Reply<Out> = Reply::Ready(Ok(Out::Bytes(Vec::new())));

fn node(id: u64, parent: u64, label: &str) -> Node {
    (id, parent, label.to_string())
}

#[test]
fn walkthrough_one_open_edit_undo_save() {
    let rig = Rig::new("walkthrough");
    let source = rig.file("notes.md", b"abc");
    let buffer = rig.buffer(source);
    assert_eq!(rig.name(buffer), source.0.to_string());
    assert_eq!(rig.read(buffer), b"abc");
    let pointer = rig.lookup(buffer, "source");
    assert_eq!(rig.read(pointer), encode(&source));

    let (h, cursor) = rig.watch(buffer);
    rig.watch(source);
    assert_eq!(rig.replace(h, 1, 1, "xyz"), DONE);
    assert_eq!(rig.read(buffer), b"axyzc");
    assert_eq!(rig.disk("notes.md"), b"abc");
    let (events, cursor) = rig.since(cursor);
    assert_eq!(events, [(buffer, Kind::Modify)]);

    assert_eq!(rig.history(h, 2, Vec::new()), DONE);
    assert_eq!(rig.read(buffer), b"abc");
    assert_eq!(rig.history(h, 3, Vec::new()), DONE);
    assert_eq!(rig.read(buffer), b"axyzc");
    let (events, cursor) = rig.since(cursor);
    assert_eq!(events, [(buffer, Kind::Modify), (buffer, Kind::Modify)]);
    let (nodes, head, published) = rig.tree(h);
    assert_eq!(nodes, [node(1, 0, "open"), node(2, 1, "replace")]);
    assert_eq!((head, published), (2, 1));

    assert_eq!(rig.save(h), DONE);
    assert_eq!(rig.disk("notes.md"), b"axyzc");
    let (events, _) = rig.since(cursor);
    assert_eq!(events, [(source, Kind::Modify)]);
    assert_eq!(rig.tree(h).2, 2);
}

#[test]
fn write_makes_a_state_and_an_edit_after_undo_branches() {
    let rig = Rig::new("branch");
    let source = rig.file("f", b"abc");
    let buffer = rig.buffer(source);
    let (h, cursor) = rig.watch(buffer);
    assert_eq!(rig.write(h, 1, b"xyz"), Reply::ok(Out::Len(3)));
    assert_eq!(rig.read(buffer), b"axyz");
    assert_eq!(rig.history(h, 2, Vec::new()), DONE);
    assert_eq!(rig.write(h, 3, b"d"), Reply::ok(Out::Len(1)));
    assert_eq!(rig.read(buffer), b"abcd");
    let (nodes, head, _) = rig.tree(h);
    assert_eq!(nodes.len(), 3);
    assert_eq!((nodes[1].1, nodes[2].1, nodes[2].2.as_str()), (1, 1, "write"));
    assert_eq!(head, 3);

    assert_eq!(rig.history(h, 1, encode(&1u64)), DONE);
    assert_eq!(rig.read(buffer), b"abc");
    assert_eq!(rig.history(h, 3, Vec::new()), DONE);
    assert_eq!(rig.read(buffer), b"abcd");
    let (events, cursor) = rig.since(cursor);
    assert_eq!(events.len(), 5, "{events:?}");

    let invalid = Reply::err(Error::Invalid);
    assert_eq!(rig.history(h, 3, Vec::new()), invalid);
    assert_eq!(rig.history(h, 1, encode(&9u64)), invalid);
    assert_eq!(rig.write(h, 9, b"x"), invalid);
    assert_eq!(rig.replace(h, 3, 2, ""), invalid);
    assert_eq!(rig.history(h, 1, encode(&1u64)), DONE);
    assert_eq!(rig.history(h, 2, Vec::new()), invalid);
    let (events, _) = rig.since(cursor);
    assert_eq!(events, [(buffer, Kind::Modify)]);
}

#[test]
fn one_buffer_per_source_and_the_rd_is_checked() {
    let rig = Rig::new("once");
    let source = rig.file("f", b"");
    assert_eq!(rig.buffer(source), rig.buffer(source));
    assert_eq!(rig.open_call(Rd(999_999)), Reply::err(Error::Invalid));
}

#[test]
fn a_read_only_source_opens_but_never_saves() {
    let rig = Rig::new("readonly");
    let source = rig.file("f", b"abc");
    let pointer = rig.lookup(rig.buffer(source), "source");

    let buffer = rig.buffer(pointer);
    assert_eq!(rig.read(buffer), encode(&source));
    let h = rig.open(buffer, Rights::WRITE);
    assert_eq!(rig.save(h), Reply::err(Error::Denied));
}

#[test]
fn the_driver_holds_no_more_than_its_limit() {
    let rig = Rig::with_limit("limit", 16);
    let source = rig.file("f", b"abc");
    let buffer = rig.buffer(source);
    let (h, cursor) = rig.watch(buffer);
    assert_eq!(rig.replace(h, 3, 0, "d"), DONE, "3 + 4 held");
    let big = rig.write(h, 0, b"0123456789");
    assert_eq!(big, Reply::err(Error::NoSpace), "10 past the 9 left");
    assert_eq!(rig.read(buffer), b"abcd", "nothing changed");
    assert_eq!(rig.tree(h).1, 2);
    assert_eq!(rig.write(h, 0, b"012345678"), Reply::ok(Out::Len(9)));
    let (events, _) = rig.since(cursor);
    assert_eq!(events.len(), 2, "one Modify per accepted change");
    let other = rig.file("g", b"x");
    assert_eq!(rig.open_call(other), Reply::err(Error::NoSpace), "full");
}

#[test]
fn replace_many_is_one_state_behind_a_precondition() {
    let rig = Rig::new("many");
    let source = rig.file("f", b"abcdef");
    let buffer = rig.buffer(source);
    let (h, cursor) = rig.watch(buffer);
    let edits = [(0, 1, "A"), (5, 1, "F")];
    assert_eq!(rig.replace_many(h, 1, &edits), DONE);
    assert_eq!(rig.read(buffer), b"AbcdeF");
    let (nodes, head, _) = rig.tree(h);
    assert_eq!((nodes.len(), head), (2, 2), "one state for two edits");
    assert_eq!(
        rig.replace_many(h, 1, &[(0, 0, "x")]),
        Reply::err(Error::Stale)
    );
    assert_eq!(rig.read(buffer), b"AbcdeF", "nothing changed");
    assert_eq!(rig.replace_many(h, 0, &[(0, 0, "x")]), DONE, "0 is any");
    assert_eq!(
        rig.replace_many(h, 3, &[(9, 0, "x")]),
        Reply::err(Error::Invalid)
    );
    let (events, _) = rig.since(cursor);
    assert_eq!(events.len(), 2);
}
