#![no_std]
#![forbid(unsafe_code)]
//! The text Driver: a Buffer over any rd it can open, as
//! 01-core-runtime.md describes one. Every accepted edit is a stored
//! state, `history` walks them and `publish.save` writes the head back
//! to the source. It speaks to the Kernel only: the source is opened
//! READ once at `open` and WRITE once at `save`, never watched.

extern crate alloc;

mod states;

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Handle, Hash, Kind,
    Module, Op, Out, Rd, Reply, Rights,
};
pub use reovim_text_api::{history, open, publish, text};

use states::History;

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
    /// Bytes held across every state of every Buffer.
    held: usize,
}

/// The Driver and its Module in one; pass it twice at `load`.
///
/// It holds at most `limit` bytes across every state of every Buffer
/// and answers `NoSpace` past that, as 01-core-runtime.md leaves
/// pressure to the Driver.
pub struct Text {
    /// `open`, then what a Buffer offers: `text`, `history`, `publish`.
    decls: [Decl; 4],
    hashes: [Hash; 4],
    state: RefCell<State>,
    limit: usize,
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

/// `many`'s edits on one copy of the draft as one state, once the head
/// is what it expected (0 for any); how many bytes the state took.
fn many(
    history: &mut History,
    room: usize,
    many: &text::Many,
) -> Result<((), usize), Error> {
    if many.expected != 0 && history.head() != many.expected {
        return Err(Error::Stale);
    }
    let mut next = history.draft().to_vec();
    for edit in &many.edits {
        next = spliced(&next, edit.at, edit.len, edit.text.as_bytes())?;
    }
    Ok(((), stored(history, room, "replace", next)?))
}

/// `len` when a state of that size fits in `room`; `NoSpace` otherwise.
const fn fits(len: usize, room: usize) -> Result<usize, Error> {
    if len > room { Err(Error::NoSpace) } else { Ok(len) }
}

/// `next` as a new state labelled `label`, when it fits in `room`;
/// the bytes it took.
fn stored(
    history: &mut History,
    room: usize,
    label: &str,
    next: Vec<u8>,
) -> Result<usize, Error> {
    let took = fits(next.len(), room)?;
    history.commit(label, next);
    Ok(took)
}

/// The text Module, ready for `load`; `root` is unused.
#[must_use]
pub fn boot(_root: &str) -> Booted {
    let text = Rc::new(Text::new());
    Booted { module: text.clone(), drivers: alloc::vec![text] }
}

impl Default for Text {
    /// 64 MiB of states.
    fn default() -> Self {
        Self::with_limit(64 << 20)
    }
}

impl Text {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A Driver holding at most `limit` bytes of states.
    #[must_use]
    pub fn with_limit(limit: usize) -> Self {
        let decls =
            [open::decl(), text::decl(), history::decl(), publish::decl()];
        let hashes = decls.each_ref().map(Decl::hash);
        Self { decls, hashes, state: RefCell::new(State::default()), limit }
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
    fn buffer(&self, ctx: &dyn Calls, source: Rd) -> Result<Rd, Error> {
        let state = self.state.borrow();
        let (home, known) = (state.home, state.opened.get(&source).copied());
        drop(state);
        if let Some(rd) = known {
            return Ok(rd);
        }
        let home = home.ok_or(Error::Invalid)?;
        let bytes = fetch(ctx, source)?;
        if bytes.len() > self.limit - self.state.borrow().held {
            return Err(Error::NoSpace);
        }
        self.state.borrow_mut().held += bytes.len();
        let history = History::new("open", bytes);
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
                Ok(slice(&reovim_sdk::encode(source), at, len))
            }
            _ => Err(Error::Unsupported),
        }
    }

    /// One change of head on the Buffer `rd`: `f` moves it or makes a
    /// state within `room` bytes, answering how many it took, and its
    /// `Ok` is one Modify.
    fn change<T>(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        f: impl FnOnce(&mut History, usize) -> Result<(T, usize), Error>,
    ) -> Result<T, Error> {
        let mut state = self.state.borrow_mut();
        let room = self.limit - state.held;
        let Some(Node::Buffer { history, .. }) = state.nodes.get_mut(&rd)
        else {
            return Err(Error::Unsupported);
        };
        let (value, took) = f(history, room)?;
        state.held += took;
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
        self.change(ctx, rd, |history, room| {
            let draft = history.draft();
            let len = draft.len() as u64 - at.min(draft.len() as u64);
            let next = spliced(draft, at, len, bytes)?;
            Ok((bytes.len(), stored(history, room, "write", next)?))
        })
    }

    /// The head stored to the source, then published.
    fn store_head(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
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
    ) -> Result<(), Error> {
        let ok = |history: &mut History, _: usize| {
            f(history).then_some(((), 0)).ok_or(Error::Invalid)
        };
        self.change(ctx, rd, ok)
    }

    /// The api `api` on `rd`: each of the four dispatches its own.
    fn call(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        api: Hash,
        number: u16,
        input: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let [open, text, history, publish] = self.hashes;
        match api {
            a if a == open => open::dispatch(self, ctx, rd, number, input),
            a if a == text => text::dispatch(self, ctx, rd, number, input),
            a if a == history => {
                history::dispatch(self, ctx, rd, number, input)
            }
            a if a == publish => {
                publish::dispatch(self, ctx, rd, number, input)
            }
            _ => Err(Error::Unsupported),
        }
    }
}

impl open::Api for Text {
    fn open(&self, ctx: &dyn Calls, _: Rd, source: Rd) -> Result<Rd, Error> {
        self.buffer(ctx, source)
    }
}

impl text::Api for Text {
    fn replace(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        edit: text::Edit,
    ) -> Result<(), Error> {
        self.change(ctx, rd, |history, room| {
            let text = edit.text.as_bytes();
            let next = spliced(history.draft(), edit.at, edit.len, text)?;
            Ok(((), stored(history, room, "replace", next)?))
        })
    }

    fn replace_many(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        edits: text::Many,
    ) -> Result<(), Error> {
        self.change(ctx, rd, |history, room| many(history, room, &edits))
    }

    /// The head state with `edit` applied, in place: the room it had
    /// is free again, and a head that is the first state is `Invalid`.
    fn amend(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        edit: text::Edit,
    ) -> Result<(), Error> {
        let mut state = self.state.borrow_mut();
        let held = state.held;
        let Some(Node::Buffer { history, .. }) = state.nodes.get_mut(&rd)
        else {
            return Err(Error::Unsupported);
        };
        let had = history.draft().len();
        let text = edit.text.as_bytes();
        let next = spliced(history.draft(), edit.at, edit.len, text)?;
        let took = fits(next.len(), self.limit - held + had)?;
        if !history.amend(next) {
            return Err(Error::Invalid);
        }
        state.held = held - had + took;
        drop(state);
        ctx.changed(rd, Kind::Modify)
    }
}

impl history::Api for Text {
    fn tree(&self, _: &dyn Calls, rd: Rd) -> Result<history::Tree, Error> {
        match self.state.borrow().nodes.get(&rd) {
            Some(Node::Buffer { history, .. }) => Ok(history.tree()),
            _ => Err(Error::Unsupported),
        }
    }

    fn goto(&self, ctx: &dyn Calls, rd: Rd, rev: u64) -> Result<(), Error> {
        self.moved(ctx, rd, |history| history.goto(rev))
    }

    fn undo(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        self.moved(ctx, rd, History::undo)
    }

    fn redo(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        self.moved(ctx, rd, History::redo)
    }
}

impl publish::Api for Text {
    fn save(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        self.store_head(ctx, rd)
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
