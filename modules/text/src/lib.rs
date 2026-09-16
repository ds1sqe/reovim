#![no_std]
#![forbid(unsafe_code)]
//! The text Driver: a Buffer over any rd it can open, as
//! 01-core-runtime.md describes one. Every accepted edit is a stored
//! state, `history` walks them and `publish.save` writes the head back
//! to the source. It speaks to the Kernel only: the source is opened
//! READ once at `open` and WRITE once at `save`, never watched.

extern crate alloc;

mod history;

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Field, Flags, Handle, Hash,
    Kind, Method, Module, Op, Out, Rd, Reply, Rights, Ty,
};

use history::History;

fn method(
    number: u16,
    name: &str,
    input: Ty,
    output: Ty,
    right: Rights,
) -> Method {
    Method { number, name: name.to_string(), input, output, right }
}

fn field(name: &str, ty: Ty) -> Field {
    Field { name: name.to_string(), ty }
}

/// `/module/text/open`: `0 open(source: Rd) -> Rd`, WRITE.
#[must_use]
pub fn open_decl() -> Decl {
    let open = method(0, "open", Ty::Rd, Ty::Rd, Rights::WRITE);
    Decl { api: "open".to_string(), ver: 1, methods: vec![open] }
}

/// Every Buffer: `0 replace(Edit{at, len, text}) -> Unit`, WRITE.
#[must_use]
pub fn text_decl() -> Decl {
    let edit = Ty::Record(vec![
        field("at", Ty::U64),
        field("len", Ty::U64),
        field("text", Ty::Utf8(1 << 20)),
    ]);
    let replace = method(0, "replace", edit, Ty::Unit, Rights::WRITE);
    Decl { api: "text".to_string(), ver: 1, methods: vec![replace] }
}

/// Every Buffer: the `history` api of 00-core.md. Revisions count
/// from 1; 0 is "none" as a parent or as `published`.
#[must_use]
pub fn history_decl() -> Decl {
    let node = Ty::Record(vec![
        field("id", Ty::U64),
        field("parent", Ty::U64),
        field("label", Ty::Utf8(16)),
    ]);
    let tree = Ty::Record(vec![
        field("nodes", Ty::Sequence(4096, alloc::boxed::Box::new(node))),
        field("head", Ty::U64),
        field("published", Ty::U64),
    ]);
    let methods = vec![
        method(0, "tree", Ty::Unit, tree, Rights::READ),
        method(1, "goto", Ty::U64, Ty::Unit, Rights::WRITE),
        method(2, "undo", Ty::Unit, Ty::Unit, Rights::WRITE),
        method(3, "redo", Ty::Unit, Ty::Unit, Rights::WRITE),
    ];
    Decl { api: "history".to_string(), ver: 1, methods }
}

/// Every Buffer: `0 save() -> Unit`, WRITE.
#[must_use]
pub fn publish_decl() -> Decl {
    let save = method(0, "save", Ty::Unit, Ty::Unit, Rights::WRITE);
    Decl { api: "publish".to_string(), ver: 1, methods: vec![save] }
}

/// The api rd.
const OPEN: Flags = Flags::READ.union(Flags::WRITE);
/// A Buffer: bytes, and the `source` child below it.
const BUFFER: Flags = OPEN.union(Flags::CONTAINER);
/// Bytes asked of the source per `Read`.
const CHUNK: u32 = 64 * 1024;

enum Node {
    Open,
    Buffer {
        source: Rd,
        history: History,
    },
    /// The `source` child of a Buffer: reads give the source rd id.
    Pointer(Rd),
}

#[derive(Default)]
struct State {
    home: Option<Rd>,
    nodes: BTreeMap<Rd, Node>,
    /// Source rd to its one Buffer.
    opened: BTreeMap<Rd, Rd>,
}

/// The Driver and its Module in one; pass it twice at `load`.
pub struct Text {
    /// `open`, then what a Buffer offers: `text`, `history`, `publish`.
    decls: [Decl; 4],
    hashes: [Hash; 4],
    state: RefCell<State>,
}

/// A `Reply` the Driver needs now: `Pending` is `Busy`, it cannot wait.
fn ready(reply: Reply<Out>) -> Result<Out, Error> {
    match reply {
        Reply::Ready(result) => result,
        Reply::Pending(_) => Err(Error::Busy),
    }
}

/// Every byte of `h` from 0, one `Read` at a time.
fn read_all(ctx: &dyn Calls, h: Handle) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    loop {
        let at = bytes.len() as u64;
        let out = ready(ctx.submit(Op::Read { h, at, len: CHUNK }))?;
        let Out::Bytes(chunk) = out else { return Err(Error::Invalid) };
        if chunk.is_empty() {
            return Ok(bytes);
        }
        bytes.extend(chunk);
    }
}

/// The whole content of `source`, through a handle opened and closed
/// here.
fn fetch(ctx: &dyn Calls, source: Rd) -> Result<Vec<u8>, Error> {
    let h = ctx.open(source, Rights::READ)?;
    let bytes = read_all(ctx, h);
    let _ = ctx.submit(Op::Close { h });
    bytes
}

/// `draft` as the whole content of `source`.
fn store(ctx: &dyn Calls, source: Rd, draft: &[u8]) -> Result<(), Error> {
    let h = ctx.open(source, Rights::WRITE)?;
    let op = Op::Write { h, at: 0, bytes: draft.to_vec() };
    let out = ready(ctx.submit(op));
    let _ = ctx.submit(Op::Close { h });
    match out? {
        Out::Len(n) if usize::try_from(n) == Ok(draft.len()) => Ok(()),
        _ => Err(Error::Busy),
    }
}

/// `len` bytes of `bytes` from `at`; none past the end.
fn slice(bytes: &[u8], at: u64, len: usize) -> Vec<u8> {
    let start = usize::try_from(at).unwrap_or(usize::MAX).min(bytes.len());
    let end = start.saturating_add(len).min(bytes.len());
    bytes[start..end].to_vec()
}

/// `draft` with its `len` bytes from `at` replaced by `text`.
fn spliced(
    draft: &[u8],
    at: u64,
    len: u64,
    text: &[u8],
) -> Result<Vec<u8>, Error> {
    let at = usize::try_from(at).map_err(|_| Error::Invalid)?;
    let len = usize::try_from(len).map_err(|_| Error::Invalid)?;
    let end = at.checked_add(len).ok_or(Error::Invalid)?;
    if end > draft.len() {
        return Err(Error::Invalid);
    }
    let mut out = draft[..at].to_vec();
    out.extend_from_slice(text);
    out.extend_from_slice(&draft[end..]);
    Ok(out)
}

/// A `U64` or `Rd` payload, already checked to fit.
fn u64_of(input: &[u8]) -> Result<u64, Error> {
    let bytes = input.try_into().map_err(|_| Error::Invalid)?;
    Ok(u64::from_le_bytes(bytes))
}

/// The `Edit` record: `at`, `len`, then `text` with its length.
fn edit_of(input: &[u8]) -> Result<(u64, u64, &[u8]), Error> {
    let at = u64_of(input.get(..8).ok_or(Error::Invalid)?)?;
    let len = u64_of(input.get(8..16).ok_or(Error::Invalid)?)?;
    let text = input.get(20..).ok_or(Error::Invalid)?;
    Ok((at, len, text))
}

/// The text Module, ready for `load`; `root` is unused.
#[must_use]
pub fn boot(_root: &str) -> Booted {
    let text = Rc::new(Text::new());
    Booted { module: text.clone(), drivers: alloc::vec![text] }
}

impl Default for Text {
    fn default() -> Self {
        let decls = [open_decl(), text_decl(), history_decl(), publish_decl()];
        let hashes = [
            decls[0].hash(),
            decls[1].hash(),
            decls[2].hash(),
            decls[3].hash(),
        ];
        Self { decls, hashes, state: RefCell::new(State::default()) }
    }
}

impl Text {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `name` under `parent` and record what it is.
    fn add(
        &self,
        ctx: &dyn Calls,
        parent: Rd,
        name: &str,
        flags: Flags,
        apis: &[Decl],
        node: Node,
    ) -> Result<Rd, Error> {
        let rd = ctx.register(DriverId(0), parent, Some(name))?;
        ctx.report(rd, flags, apis)?;
        self.state.borrow_mut().nodes.insert(rd, node);
        Ok(rd)
    }

    /// The Buffer for `source`, made now if there is none.
    fn open(&self, ctx: &dyn Calls, source: Rd) -> Result<Rd, Error> {
        let state = self.state.borrow();
        let (home, known) = (state.home, state.opened.get(&source).copied());
        drop(state);
        if let Some(rd) = known {
            return Ok(rd);
        }
        let home = home.ok_or(Error::Invalid)?;
        let history = History::new("open", fetch(ctx, source)?);
        let node = Node::Buffer { source, history };
        let name = source.0.to_string();
        let rd = self.add(ctx, home, &name, BUFFER, &self.decls[1..], node)?;
        let pointer = Node::Pointer(source);
        self.add(ctx, rd, "source", Flags::READ, &[], pointer)?;
        self.state.borrow_mut().opened.insert(source, rd);
        Ok(rd)
    }

    fn read(&self, rd: Rd, at: u64, len: usize) -> Result<Vec<u8>, Error> {
        let state = self.state.borrow();
        match state.nodes.get(&rd) {
            Some(Node::Buffer { history, .. }) => {
                Ok(slice(history.draft(), at, len))
            }
            Some(Node::Pointer(source)) => {
                Ok(slice(&source.0.to_le_bytes(), at, len))
            }
            _ => Err(Error::Unsupported),
        }
    }

    /// One change of head on the Buffer `rd`: `f` moves it or makes a
    /// state, and its `Ok` is one Modify.
    fn change<T>(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        f: impl FnOnce(&mut History) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut state = self.state.borrow_mut();
        let Some(Node::Buffer { history, .. }) = state.nodes.get_mut(&rd)
        else {
            return Err(Error::Unsupported);
        };
        let value = f(history)?;
        drop(state);
        ctx.changed(rd, Kind::Modify)?;
        Ok(value)
    }

    /// The draft from `at` becomes `bytes` and ends there.
    fn write(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        at: u64,
        bytes: &[u8],
    ) -> Result<usize, Error> {
        self.change(ctx, rd, |history| {
            let draft = history.draft();
            let len = draft.len() as u64 - at.min(draft.len() as u64);
            let next = spliced(draft, at, len, bytes)?;
            history.commit("write", next);
            Ok(bytes.len())
        })
    }

    /// The `Edit` of `replace` as a new state.
    fn replace(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        input: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let (at, len, text) = edit_of(input)?;
        self.change(ctx, rd, |history| {
            let next = spliced(history.draft(), at, len, text)?;
            history.commit("replace", next);
            Ok(Vec::new())
        })
    }

    fn save(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        let state = self.state.borrow();
        let Some(Node::Buffer { source, history }) = state.nodes.get(&rd)
        else {
            return Err(Error::Unsupported);
        };
        let (source, draft) = (*source, history.draft().to_vec());
        drop(state);
        store(ctx, source, &draft)?;
        let mut state = self.state.borrow_mut();
        if let Some(Node::Buffer { history, .. }) = state.nodes.get_mut(&rd) {
            history.published();
        }
        Ok(())
    }

    /// A move of head; a refused one is `Invalid` and emits nothing.
    fn moved(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        f: impl FnOnce(&mut History) -> bool,
    ) -> Result<Vec<u8>, Error> {
        let ok = |history: &mut History| {
            f(history).then_some(()).ok_or(Error::Invalid)
        };
        self.change(ctx, rd, ok).map(|()| Vec::new())
    }

    fn tree(&self, rd: Rd) -> Result<Vec<u8>, Error> {
        match self.state.borrow().nodes.get(&rd) {
            Some(Node::Buffer { history, .. }) => Ok(history.tree()),
            _ => Err(Error::Unsupported),
        }
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        api: Hash,
        number: u16,
        input: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let [open, text, history, publish] = self.hashes;
        match (api, number) {
            (a, 0) if a == open => {
                let made = self.open(ctx, Rd(u64_of(input)?))?;
                Ok(made.0.to_le_bytes().to_vec())
            }
            (a, 0) if a == text => self.replace(ctx, rd, input),
            (a, 0) if a == history => self.tree(rd),
            (a, 1) if a == history => {
                let rev = u64_of(input)?;
                self.moved(ctx, rd, |h| h.goto(rev))
            }
            (a, 2) if a == history => self.moved(ctx, rd, History::undo),
            (a, 3) if a == history => self.moved(ctx, rd, History::redo),
            (a, 0) if a == publish => self.save(ctx, rd).map(|()| Vec::new()),
            _ => Err(Error::Unsupported),
        }
    }
}

impl Module for Text {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        self.state.borrow_mut().home = Some(home);
        let apis = core::slice::from_ref(&self.decls[0]);
        self.add(ctx, home, "open", OPEN, apis, Node::Open).map(|_| ())
    }
}

impl Driver for Text {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        _: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        self.read(Rd(h), at, len).into()
    }

    fn write(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        buf: &[u8],
    ) -> Reply<usize> {
        self.write(ctx, Rd(h), at, buf).into()
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        api: Hash,
        method: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        self.call(ctx, Rd(h), api, method, input).into()
    }
}
