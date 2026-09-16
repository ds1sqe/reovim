#![no_std]
#![forbid(unsafe_code)]
//! The buffer Driver: an in-memory draft of any rd it can open. The
//! draft is edited with `write` and pushed back with `save`. It speaks
//! to the Kernel only: the source is opened READ once at `attach` and
//! WRITE once at `save`, never watched. Reload and detach come with
//! history in milestone 2.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Handle, Hash, Kind,
    Method, Module, Op, Out, Rd, Reply, Rights, Ty,
};

/// `/module/buffer/attach`: `0 attach(source: Rd) -> Rd`, WRITE.
#[must_use]
pub fn attach_decl() -> Decl {
    let attach = Method {
        number: 0,
        name: "attach".to_string(),
        input: Ty::Rd,
        output: Ty::Rd,
        right: Rights::WRITE,
    };
    Decl { api: "buffers".to_string(), ver: 1, methods: vec![attach] }
}

/// Every Buffer: `0 save() -> Unit`, WRITE.
#[must_use]
pub fn buffer_decl() -> Decl {
    let save = Method {
        number: 0,
        name: "save".to_string(),
        input: Ty::Unit,
        output: Ty::Unit,
        right: Rights::WRITE,
    };
    Decl { api: "buffer".to_string(), ver: 1, methods: vec![save] }
}

/// The api rd.
const ATTACH: Flags = Flags::READ.union(Flags::WRITE);
/// A Buffer: bytes, and the `source` child below it.
const BUFFER: Flags = ATTACH.union(Flags::CONTAINER);
/// Bytes asked of the source per `Read`.
const CHUNK: u32 = 64 * 1024;

enum Node {
    Attach,
    Buffer {
        source: Rd,
        draft: Vec<u8>,
    },
    /// The `source` child of a Buffer: reads give the source rd id.
    Pointer(Rd),
}

#[derive(Default)]
struct State {
    home: Option<Rd>,
    nodes: BTreeMap<Rd, Node>,
    /// Source rd to its one Buffer.
    attached: BTreeMap<Rd, Rd>,
}

/// The Driver and its Module in one; pass it twice at `load`.
pub struct Buffers {
    decls: (Decl, Decl),
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

/// The `Rd` payload of `attach`, already checked to fit.
fn source_of(input: &[u8]) -> Result<Rd, Error> {
    let bytes = input.try_into().map_err(|_| Error::Invalid)?;
    Ok(Rd(u64::from_le_bytes(bytes)))
}

/// The buffer Module, ready for `load`; `root` is unused.
#[must_use]
pub fn boot(_root: &str) -> Booted {
    let buffers = Rc::new(Buffers::new());
    Booted { module: buffers.clone(), drivers: alloc::vec![buffers] }
}

impl Default for Buffers {
    fn default() -> Self {
        let decls = (attach_decl(), buffer_decl());
        Self { decls, state: RefCell::new(State::default()) }
    }
}

impl Buffers {
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
    fn attach(&self, ctx: &dyn Calls, source: Rd) -> Result<Rd, Error> {
        let state = self.state.borrow();
        let (home, known) = (state.home, state.attached.get(&source).copied());
        drop(state);
        if let Some(rd) = known {
            return Ok(rd);
        }
        let home = home.ok_or(Error::Invalid)?;
        let draft = fetch(ctx, source)?;
        let node = Node::Buffer { source, draft };
        let name = source.0.to_string();
        let apis = core::slice::from_ref(&self.decls.1);
        let rd = self.add(ctx, home, &name, BUFFER, apis, node)?;
        let pointer = Node::Pointer(source);
        self.add(ctx, rd, "source", Flags::READ, &[], pointer)?;
        self.state.borrow_mut().attached.insert(source, rd);
        Ok(rd)
    }

    fn read(&self, rd: Rd, at: u64, len: usize) -> Result<Vec<u8>, Error> {
        let state = self.state.borrow();
        match state.nodes.get(&rd) {
            Some(Node::Buffer { draft, .. }) => Ok(slice(draft, at, len)),
            Some(Node::Pointer(source)) => {
                Ok(slice(&source.0.to_le_bytes(), at, len))
            }
            _ => Err(Error::Unsupported),
        }
    }

    /// The draft from `at` becomes `bytes` and ends there.
    fn write(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        at: u64,
        bytes: &[u8],
    ) -> Result<usize, Error> {
        let mut state = self.state.borrow_mut();
        let Some(Node::Buffer { draft, .. }) = state.nodes.get_mut(&rd) else {
            return Err(Error::Unsupported);
        };
        let at = usize::try_from(at).map_err(|_| Error::Invalid)?;
        if at > draft.len() {
            return Err(Error::Invalid);
        }
        draft.truncate(at);
        draft.extend_from_slice(bytes);
        drop(state);
        ctx.changed(rd, Kind::Modify)?;
        Ok(bytes.len())
    }

    fn save(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        let state = self.state.borrow();
        let Some(Node::Buffer { source, draft }) = state.nodes.get(&rd) else {
            return Err(Error::Unsupported);
        };
        let (source, draft) = (*source, draft.clone());
        drop(state);
        store(ctx, source, &draft)
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        input: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let attach =
            matches!(self.state.borrow().nodes.get(&rd), Some(Node::Attach));
        if attach {
            let made = self.attach(ctx, source_of(input)?)?;
            return Ok(made.0.to_le_bytes().to_vec());
        }
        self.save(ctx, rd).map(|()| Vec::new())
    }
}

impl Module for Buffers {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        self.state.borrow_mut().home = Some(home);
        let apis = core::slice::from_ref(&self.decls.0);
        self.add(ctx, home, "attach", ATTACH, apis, Node::Attach).map(|_| ())
    }
}

impl Driver for Buffers {
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
        _: Hash,
        _: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        self.call(ctx, Rd(h), input).into()
    }
}
