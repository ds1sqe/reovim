//! The Client's own tree: a `Calls` over the Drivers this Client
//! loaded, under `/self`, unioned with the wire by rd and handle range.
//!
//! One constant splits both spaces. The server allocates rds and
//! handles from below and never reuses one, so an id at or above
//! [`LOCAL_BASE`] is the Host's and every other one is the server's;
//! there is no second rule and no table of which side an id came from.
//! Two points are special and no others: the root's `self` child, which
//! is the Host's own root, and a `Lookup` under the server's
//! `/sys/decls` for a hash a local `report` published.
//!
//! The Kernel's shape holds here (kernel/runtime/src/kernel.rs): no
//! borrow of the state and no borrow of the wire outlives a Driver slot
//! call. `submit` takes what it needs, drops the borrow, calls the
//! slot, and takes it again to record.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::{Cell, RefCell, RefMut};

use reovim_kernel_interface::{
    Calls, Decl, Driver, DriverId, Error, Event, Flags, Handle, Hash, Info,
    Kind, ModuleId, Op, Out, Outcome, Rd, Reply, Rights, Token, value,
};

use crate::client::{Client, ROOT};
use crate::fault::Fault;

/// The bottom of the Client's half of the rd space and of the handle
/// space. An id at or above it is the Host's, every other one the
/// wire's.
pub const LOCAL_BASE: u64 = 1 << 63;

/// `/self`, the Host's root and the root's only local child.
pub const SELF: Rd = Rd(LOCAL_BASE);

/// Events the ring keeps: the Kernel's `watch_ring` default.
const RING: usize = 1024;

/// Ops standing inside ops: the Kernel's `reentry` default.
const REENTRY: u8 = 8;

/// An op's answer. Nothing here is ever Pending: a local Driver's
/// Pending is refused, and a wire Pending is waited out by
/// `Tree::exchange` before the op comes back.
type Answer = Result<Out, Error>;

/// Whether `id` is the Client's own.
const fn local(id: u64) -> bool {
    id >= LOCAL_BASE
}

/// Who serves a local rd.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Owner {
    /// `/self` itself.
    Container,
    /// A Driver this Client loaded.
    Driver { module: ModuleId, driver: DriverId },
    /// The cell of a declaration a local `report` published, which
    /// stands under the server's `/sys/decls`.
    Decl(Hash),
    /// A cell a write of sends one of the two wire messages that are
    /// not ops: `unload` of a Module's name, else `load` of a path.
    /// The Client may speak to the link, so it is the Client that
    /// serves them; a caller reaches them as it reaches anything.
    Loader { unload: bool },
}

/// One row of the local table: the Kernel's `Entry` less what a Client
/// has no use for.
struct Node {
    owner: Owner,
    parent: Rd,
    name: Option<String>,
    flags: Flags,
    ctime: u64,
    mtime: u64,
    atime: u64,
    apis: Vec<Hash>,
    children: Vec<Rd>,
    /// `lookup("")` was asked once.
    listed: bool,
}

impl Node {
    fn new(
        owner: Owner,
        parent: Rd,
        name: Option<&str>,
        flags: Flags,
        now: u64,
    ) -> Self {
        Self {
            owner,
            parent,
            name: name.map(ToString::to_string),
            flags,
            ctime: now,
            mtime: now,
            atime: now,
            apis: Vec::new(),
            children: Vec::new(),
            listed: false,
        }
    }

    fn info(&self) -> Info {
        Info {
            name: self.name.clone(),
            flags: self.flags,
            ctime: self.ctime,
            mtime: self.mtime,
            atime: self.atime,
            apis: self.apis.clone(),
        }
    }
}

/// One open local handle.
#[derive(Clone, Copy)]
struct Slot {
    rd: Rd,
    rights: Rights,
    /// The Driver's own handle value.
    cookie: u64,
    watching: bool,
}

/// Everything the Host holds: the tree it serves, the handles on it,
/// the Drivers behind it and the one event ring.
struct State {
    nodes: BTreeMap<Rd, Node>,
    next_rd: u64,
    handles: BTreeMap<Handle, Slot>,
    next_handle: u64,
    /// The Drivers loaded here, by `ModuleId` then `DriverId`, as the
    /// Kernel indexes them.
    drivers: Vec<Vec<Rc<dyn Driver>>>,
    /// What a local `report` published, by hash.
    decls: BTreeMap<Hash, Decl>,
    /// The server's `/sys/decls`, once a lookup has walked to it.
    decls_rd: Option<Rd>,
    events: VecDeque<Event>,
    next_seq: u64,
}

impl State {
    fn node(&self, rd: Rd) -> Result<&Node, Error> {
        self.nodes.get(&rd).ok_or(Error::Invalid)
    }

    fn node_mut(&mut self, rd: Rd) -> Result<&mut Node, Error> {
        self.nodes.get_mut(&rd).ok_or(Error::Invalid)
    }

    /// The node when `module` serves it. An rd the wire owns is
    /// `Denied`: a Client registers nothing at the server.
    fn owned(&self, rd: Rd, module: ModuleId) -> Result<&Node, Error> {
        if !local(rd.0) {
            return Err(Error::Denied);
        }
        let node = self.node(rd)?;
        match node.owner {
            Owner::Driver { module: m, .. } if m == module => Ok(node),
            _ => Err(Error::Denied),
        }
    }

    fn driver(
        &self,
        module: ModuleId,
        driver: DriverId,
    ) -> Result<Rc<dyn Driver>, Error> {
        self.drivers
            .get(module.0 as usize)
            .and_then(|loaded| loaded.get(driver.0 as usize))
            .cloned()
            .ok_or(Error::Invalid)
    }

    /// The child of `parent` named `name`, from the table.
    fn child(&self, parent: Rd, name: &str) -> Option<Rd> {
        let named = |rd: &Rd| self.nodes[rd].name.as_deref() == Some(name);
        self.nodes.get(&parent)?.children.iter().copied().find(named)
    }

    /// `Invalid` when the name is malformed or taken under `parent`.
    fn check_name(&self, parent: Rd, name: Option<&str>) -> Result<(), Error> {
        let Some(name) = name else { return Ok(()) };
        let malformed = name.is_empty() || name.contains(['/', '\0']);
        if malformed || self.child(parent, name).is_some() {
            return Err(Error::Invalid);
        }
        Ok(())
    }

    /// A new rd under `parent`; the caller has checked parent and name.
    /// A parent the wire owns keeps no child list here, which is what a
    /// declaration cell under `/sys/decls` stands on.
    fn insert(
        &mut self,
        parent: Rd,
        name: Option<&str>,
        owner: Owner,
        flags: Flags,
        now: u64,
    ) -> Rd {
        let rd = Rd(self.next_rd);
        self.next_rd += 1;
        self.nodes.insert(rd, Node::new(owner, parent, name, flags, now));
        if let Some(parent) = self.nodes.get_mut(&parent) {
            parent.children.push(rd);
        }
        rd
    }

    /// `rd` and its subtree gone, deepest first, `Delete` on `parent`.
    fn unregister(&mut self, rd: Rd, parent: Rd) {
        let mut order = alloc::vec![rd];
        let mut at = 0;
        while let Some(next) = order.get(at).copied() {
            order.extend_from_slice(&self.nodes[&next].children);
            at += 1;
        }
        for gone in order.into_iter().rev() {
            self.emit(gone, Kind::Gone, None);
            self.nodes.remove(&gone);
        }
        if let Some(node) = self.nodes.get_mut(&parent) {
            node.children.retain(|child| *child != rd);
        }
        self.emit(parent, Kind::Delete, Some(rd));
    }

    /// One event on a local rd, kept only while a local handle watches
    /// it, so nothing accumulates for a resource no one is reading.
    fn emit(&mut self, rd: Rd, kind: Kind, child: Option<Rd>) {
        let watched = |slot: &Slot| slot.watching && slot.rd == rd;
        if self.handles.values().any(watched) {
            self.push(Event { seq: 0, rd, kind, child });
        }
    }

    /// One event, numbered here whichever side it came from, the oldest
    /// trimmed until [`RING`] are left.
    fn push(&mut self, event: Event) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.events.push_back(Event { seq, ..event });
        while self.events.len() > RING {
            self.events.pop_front();
        }
    }

    /// Events after `cursor` and the cursor for the next ask. A cursor
    /// the ring no longer covers yields one `Overflow` on `ROOT`.
    fn since(&self, cursor: u64) -> (Vec<Event>, u64) {
        let head = self.next_seq - 1;
        let oldest = self.events.front().map_or(self.next_seq, |e| e.seq);
        if cursor + 1 < oldest {
            let kind = Kind::Overflow;
            let event = Event { seq: head, rd: ROOT, kind, child: None };
            return (alloc::vec![event], head);
        }
        let after = |event: &&Event| event.seq > cursor;
        (self.events.iter().filter(after).copied().collect(), head)
    }
}

/// How deep `run` stands while one call is in it. It falls on drop, so
/// an early return, an error or a panic in a slot leaves the count
/// where it found it.
struct Depth<'h>(&'h Cell<u8>);

impl Drop for Depth<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get().saturating_sub(1));
    }
}

/// The Client's own `Calls`: the wire beside the local tree, and one
/// door over both.
pub struct Host {
    wire: RefCell<Client>,
    state: RefCell<State>,
    depth: Cell<u8>,
    clock: fn() -> u64,
}

impl Host {
    /// The Host over `wire`, holding `/self` with the two cells the
    /// Client writes a wire message by, and the Client's own
    /// `ModuleId(0)`, which serves no Driver yet. `clock` is the host's
    /// monotonic tick, which stamps the times of a local rd.
    #[must_use]
    pub fn over(wire: Client, clock: fn() -> u64) -> Self {
        let now = clock();
        let flags = Flags::READ.union(Flags::CONTAINER);
        let root = Node::new(Owner::Container, ROOT, Some("self"), flags, now);
        let mut state = State {
            nodes: BTreeMap::from([(SELF, root)]),
            next_rd: LOCAL_BASE + 1,
            handles: BTreeMap::new(),
            next_handle: LOCAL_BASE,
            drivers: alloc::vec![Vec::new()],
            decls: BTreeMap::new(),
            decls_rd: None,
            events: VecDeque::new(),
            next_seq: 1,
        };
        let bin = state.insert(SELF, Some("bin"), Owner::Container, flags, now);
        for name in ["load", "unload"] {
            let owner = Owner::Loader { unload: name == "unload" };
            state.insert(bin, Some(name), owner, Flags::WRITE, now);
        }
        let depth = Cell::new(0);
        Self {
            wire: RefCell::new(wire),
            state: RefCell::new(state),
            depth,
            clock,
        }
    }

    /// The `Client` behind the Host, for a caller that must speak to
    /// the link and not to the tree: its capability, its client rd, and
    /// the two messages `Calls` cannot spell, `load` and `unload`.
    #[must_use]
    pub fn wire(&self) -> RefMut<'_, Client> {
        self.wire.borrow_mut()
    }

    /// A Module's Drivers, in `DriverId` order; the Module registers
    /// what it serves through the `Calls` this answers with.
    pub fn load(&self, drivers: Vec<Rc<dyn Driver>>) -> ModuleId {
        let mut s = self.state.borrow_mut();
        let id = u32::try_from(s.drivers.len()).unwrap_or(u32::MAX);
        s.drivers.push(drivers);
        ModuleId(id)
    }

    /// What `module` holds to speak to this Host, so one Module cannot
    /// speak for another.
    #[must_use]
    pub const fn calls(&self, module: ModuleId) -> HostCalls<'_> {
        HostCalls { host: self, module }
    }

    /// The Client's own, which serves nothing and so may not register.
    #[must_use]
    pub const fn own(&self) -> HostCalls<'_> {
        self.calls(ModuleId(0))
    }

    fn now(&self) -> u64 {
        (self.clock)()
    }

    /// One step deeper into `run`, or `None` at [`REENTRY`]: the guard
    /// is what holds the step, and dropping it takes it back.
    fn deeper(&self) -> Option<Depth<'_>> {
        let depth = self.depth.get();
        if depth >= REENTRY {
            return None;
        }
        self.depth.set(depth + 1);
        Some(Depth(&self.depth))
    }

    fn driver(
        &self,
        module: ModuleId,
        driver: DriverId,
    ) -> Result<Rc<dyn Driver>, Error> {
        self.state.borrow().driver(module, driver)
    }

    /// One op to the server. The borrow lasts exactly this statement:
    /// the in-process Carrier runs the Kernel on this thread and the
    /// Kernel reaches no Client, so a wire exchange cannot re-enter.
    fn sent(&self, op: Op) -> Answer {
        self.wire.borrow_mut().request(op).map_err(|f| refused(&f))
    }

    /// One op on the union, one step deeper; past the cap it is `Busy`,
    /// so a chain of Drivers ends at an error and not at the stack.
    fn run(&self, op: Op) -> Answer {
        let Some(_depth) = self.deeper() else { return Err(Error::Busy) };
        match op {
            Op::Lookup { rd, name } => lookup(self, rd, &name),
            Op::Info { rd } => info(self, rd),
            Op::Parent { rd } => parent(self, rd),
            Op::Children { rd } => children(self, rd),
            Op::Open { rd, rights } => open(self, rd, rights),
            Op::Read { h, at, len } => read(self, h, at, len),
            Op::Write { h, at, bytes } => write(self, h, at, bytes),
            Op::Call { h, api, method, input } => {
                call(self, h, api, method, input)
            }
            Op::Watch { h, on } => watch(self, h, on),
            Op::Close { h } => close(self, h),
            Op::Since { cursor } => since(self, cursor),
            // No local token exists, so a caller asking after one is
            // asking the server.
            op @ (Op::Ask { .. } | Op::Cancel { .. }) => self.sent(op),
        }
    }
}

/// What a Module holds to speak to the Host, as `ModuleCalls` is what
/// it holds to speak to the Kernel.
pub struct HostCalls<'h> {
    host: &'h Host,
    module: ModuleId,
}

impl Calls for HostCalls<'_> {
    fn register(
        &self,
        driver: DriverId,
        parent: Rd,
        name: Option<&str>,
    ) -> Result<Rd, Error> {
        if !local(parent.0) {
            return Err(Error::Denied);
        }
        let now = self.host.now();
        let mut s = self.host.state.borrow_mut();
        s.driver(self.module, driver)?;
        check_parent(&s, parent, self.module)?;
        s.check_name(parent, name)?;
        let owner = Owner::Driver { module: self.module, driver };
        let rd = s.insert(parent, name, owner, Flags::empty(), now);
        s.emit(parent, Kind::Create, Some(rd));
        Ok(rd)
    }

    fn name(&self, rd: Rd, name: Option<&str>) -> Result<(), Error> {
        let mut s = self.host.state.borrow_mut();
        let parent = s.owned(rd, self.module)?.parent;
        s.check_name(parent, name)?;
        s.node_mut(rd)?.name = name.map(ToString::to_string);
        s.emit(parent, Kind::Move, Some(rd));
        Ok(())
    }

    fn report(&self, rd: Rd, flags: Flags, apis: &[Decl]) -> Result<(), Error> {
        let now = self.host.now();
        let mut s = self.host.state.borrow_mut();
        s.owned(rd, self.module)?;
        let hashes = apis
            .iter()
            .map(|decl| store_decl(&mut s, decl))
            .collect::<Result<_, _>>()?;
        let node = s.node_mut(rd)?;
        node.flags = flags;
        node.apis = hashes;
        node.ctime = now;
        s.emit(rd, Kind::Attrib, None);
        Ok(())
    }

    fn unregister(&self, rd: Rd) -> Result<(), Error> {
        let mut s = self.host.state.borrow_mut();
        let parent = s.owned(rd, self.module)?.parent;
        s.unregister(rd, parent);
        Ok(())
    }

    fn changed(&self, rd: Rd, kind: Kind) -> Result<(), Error> {
        let now = self.host.now();
        let mut s = self.host.state.borrow_mut();
        s.owned(rd, self.module)?;
        let node = s.node_mut(rd)?;
        match kind {
            Kind::Modify => node.mtime = now,
            Kind::Attrib => node.ctime = now,
            _ => return Err(Error::Invalid),
        }
        s.emit(rd, kind, None);
        Ok(())
    }

    /// Always `Invalid`: nothing is pending on one stack, so there is
    /// no token to retire.
    fn done(&self, _: Token, _: Outcome) -> Result<(), Error> {
        Err(Error::Invalid)
    }

    fn open(&self, rd: Rd, rights: Rights) -> Result<Handle, Error> {
        match self.host.run(Op::Open { rd, rights })? {
            Out::Handle(handle) => Ok(handle),
            _ => Err(Error::Unsupported),
        }
    }

    fn submit(&self, op: Op) -> Reply<Out> {
        Reply::Ready(self.host.run(op))
    }
}

/// `register` is allowed under `/self` and under any rd this Module
/// serves; a Container either way.
fn check_parent(s: &State, parent: Rd, module: ModuleId) -> Result<(), Error> {
    let node = s.node(parent)?;
    if !node.flags.contains(Flags::CONTAINER) {
        return Err(Error::Invalid);
    }
    let allowed = match node.owner {
        Owner::Container => true,
        Owner::Driver { module: m, .. } => m == module,
        Owner::Decl(_) | Owner::Loader { .. } => false,
    };
    if allowed { Ok(()) } else { Err(Error::Denied) }
}

/// Check and store one declaration by hash; refuse a collision. There
/// is no `/sys/decls` row to register: a lookup there is answered from
/// this map.
fn store_decl(s: &mut State, decl: &Decl) -> Result<Hash, Error> {
    decl.check()?;
    let hash = decl.hash();
    if let Some(known) = s.decls.get(&hash) {
        if known.canonical() == decl.canonical() {
            return Ok(hash);
        }
        return Err(Error::Invalid);
    }
    s.decls.insert(hash, decl.clone());
    Ok(hash)
}

fn lookup(host: &Host, rd: Rd, name: &str) -> Answer {
    if rd == ROOT && name == "self" {
        return Ok(Out::Rd(SELF));
    }
    if !local(rd.0) {
        if let Some(cell) = published(host, rd, name) {
            return Ok(Out::Rd(cell));
        }
        return host.sent(Op::Lookup { rd, name: name.to_string() });
    }
    let (owner, child) = {
        let s = host.state.borrow();
        (s.node(rd)?.owner, s.child(rd, name))
    };
    if let Some(child) = child {
        return Ok(Out::Rd(child));
    }
    let Owner::Driver { module, driver } = owner else {
        return Err(Error::Invalid);
    };
    let d = host.driver(module, driver)?;
    Ok(Out::Rd(ready(d.lookup(&host.calls(module), rd, name))?))
}

/// The cell of the declaration `name` names under the server's
/// `/sys/decls`, made here the first time it is asked for. `None` for
/// every other lookup on the wire's side.
fn published(host: &Host, rd: Rd, name: &str) -> Option<Rd> {
    let hash = u64::from_str_radix(name, 16).map(Hash).ok()?;
    if !host.state.borrow().decls.contains_key(&hash) {
        return None;
    }
    if rd != decls_rd(host)? {
        return None;
    }
    let now = host.now();
    let mut s = host.state.borrow_mut();
    let made = |(at, node): (&Rd, &Node)| {
        (node.owner == Owner::Decl(hash)).then_some(*at)
    };
    if let Some(cell) = s.nodes.iter().find_map(made) {
        return Some(cell);
    }
    let name = format!("{:016x}", hash.0);
    let owner = Owner::Decl(hash);
    Some(s.insert(rd, Some(&name), owner, Flags::READ, now))
}

/// The server's `/sys/decls`, walked to once and kept. It is asked for
/// only when a lookup names a hash this Host published, so no other
/// local op pays a wire message for it.
fn decls_rd(host: &Host) -> Option<Rd> {
    if let Some(rd) = host.state.borrow().decls_rd {
        return Some(rd);
    }
    let sys = found(host.sent(Op::Lookup { rd: ROOT, name: "sys".into() }))?;
    let rd = found(host.sent(Op::Lookup { rd: sys, name: "decls".into() }))?;
    host.state.borrow_mut().decls_rd = Some(rd);
    Some(rd)
}

fn info(host: &Host, rd: Rd) -> Answer {
    if !local(rd.0) {
        return host.sent(Op::Info { rd });
    }
    Ok(Out::Info(host.state.borrow().node(rd)?.info()))
}

fn parent(host: &Host, rd: Rd) -> Answer {
    if !local(rd.0) {
        return host.sent(Op::Parent { rd });
    }
    Ok(Out::Rd(host.state.borrow().node(rd)?.parent))
}

/// The root's children are the server's with `/self` after them, the
/// one merge; a local Container lists through its Driver once, as the
/// Kernel does.
fn children(host: &Host, rd: Rd) -> Answer {
    if rd == ROOT {
        let Out::Rds(mut rds) = host.sent(Op::Children { rd })? else {
            return Err(Error::Invalid);
        };
        rds.push(SELF);
        return Ok(Out::Rds(rds));
    }
    if !local(rd.0) {
        return host.sent(Op::Children { rd });
    }
    let first = {
        let mut s = host.state.borrow_mut();
        let node = s.node_mut(rd)?;
        let first = !node.listed;
        node.listed = true;
        first.then_some(node.owner)
    };
    if let Some(Owner::Driver { module, driver }) = first {
        // The table is what `Children` answers from; the reply itself
        // carries nothing.
        let d = host.driver(module, driver)?;
        let _ = d.lookup(&host.calls(module), rd, "");
    }
    Ok(Out::Rds(host.state.borrow().node(rd)?.children.clone()))
}

fn open(host: &Host, rd: Rd, rights: Rights) -> Answer {
    if !local(rd.0) {
        let opened = host.wire.borrow_mut().open(rd, rights);
        return Ok(Out::Handle(opened.map_err(|f| refused(&f))?));
    }
    let owner = {
        let s = host.state.borrow();
        let node = s.node(rd)?;
        if !allows(node.flags, rights) {
            return Err(Error::Denied);
        }
        node.owner
    };
    let cookie = match owner {
        Owner::Driver { module, driver } => {
            let d = host.driver(module, driver)?;
            ready(d.open(&host.calls(module), rd, rights))?
        }
        Owner::Container | Owner::Decl(_) | Owner::Loader { .. } => 0,
    };
    let mut s = host.state.borrow_mut();
    let h = Handle(s.next_handle);
    s.next_handle += 1;
    s.handles.insert(h, Slot { rd, rights, cookie, watching: false });
    Ok(Out::Handle(h))
}

fn read(host: &Host, h: Handle, at: u64, len: u32) -> Answer {
    if !local(h.0) {
        return host.sent(Op::Read { h, at, len });
    }
    let (slot, owner) = held(host, h, Rights::READ)?;
    let len = len as usize;
    let bytes = match owner {
        Owner::Driver { module, driver } => {
            let d = host.driver(module, driver)?;
            ready(d.read(&host.calls(module), slot.cookie, at, len))?
        }
        Owner::Decl(hash) => {
            let s = host.state.borrow();
            part(&s.decls.get(&hash).ok_or(Error::Gone)?.canonical(), at, len)
        }
        Owner::Container | Owner::Loader { .. } => {
            return Err(Error::Unsupported);
        }
    };
    let now = host.now();
    if let Ok(node) = host.state.borrow_mut().node_mut(slot.rd) {
        node.atime = now;
    }
    Ok(Out::Bytes(bytes))
}

fn write(host: &Host, h: Handle, at: u64, bytes: Vec<u8>) -> Answer {
    if !local(h.0) {
        return host.sent(Op::Write { h, at, bytes });
    }
    let (slot, owner) = held(host, h, Rights::WRITE)?;
    if let Owner::Loader { unload } = owner {
        return sent_message(host, unload, &bytes);
    }
    let Owner::Driver { module, driver } = owner else {
        return Err(Error::Unsupported);
    };
    let d = host.driver(module, driver)?;
    let calls = host.calls(module);
    let took = ready(d.write(&calls, slot.cookie, at, &bytes))?;
    Ok(Out::Len(u32::try_from(took).unwrap_or(u32::MAX)))
}

/// One of the two messages, sent: the borrow of the wire lasts exactly
/// the statement that speaks, as every wire op's does.
fn sent_message(host: &Host, unload: bool, bytes: &[u8]) -> Answer {
    let text = core::str::from_utf8(bytes).map_err(|_| Error::Invalid)?;
    let sent = if unload {
        host.wire.borrow_mut().unload(text)
    } else {
        host.wire.borrow_mut().load(text)
    };
    sent.map_err(|f| refused(&f))?;
    Ok(Out::Len(u32::try_from(bytes.len()).unwrap_or(u32::MAX)))
}

/// What the Kernel checks before the slot, because a Driver's `call` is
/// written trusting it: the rd offers the api, the declaration has the
/// method, the handle holds its right, the input fits.
fn call(
    host: &Host,
    h: Handle,
    api: Hash,
    method: u16,
    input: Vec<u8>,
) -> Answer {
    if !local(h.0) {
        return host.sent(Op::Call { h, api, method, input });
    }
    let (slot, owner) = held(host, h, Rights::empty())?;
    check_call(&host.state.borrow(), &slot, api, method, &input)?;
    let Owner::Driver { module, driver } = owner else {
        return Err(Error::Unsupported);
    };
    let d = host.driver(module, driver)?;
    let calls = host.calls(module);
    let out = ready(d.call(&calls, slot.cookie, api, method, &input))?;
    Ok(Out::Bytes(out))
}

fn check_call(
    s: &State,
    slot: &Slot,
    api: Hash,
    method: u16,
    input: &[u8],
) -> Result<(), Error> {
    let node = s.nodes.get(&slot.rd).ok_or(Error::Gone)?;
    if !node.apis.contains(&api) {
        return Err(Error::Unsupported);
    }
    let decl = s.decls.get(&api).ok_or(Error::Unsupported)?;
    let method = decl.method(method).ok_or(Error::Unsupported)?;
    if !slot.rights.contains(method.right) {
        return Err(Error::Denied);
    }
    if !value::fits(input, &method.input) {
        return Err(Error::Invalid);
    }
    Ok(())
}

fn watch(host: &Host, h: Handle, on: bool) -> Answer {
    if !local(h.0) {
        return host.sent(Op::Watch { h, on });
    }
    let (slot, owner) = held(host, h, Rights::READ)?;
    if let Some(open) = host.state.borrow_mut().handles.get_mut(&h) {
        open.watching = on;
    }
    if let Owner::Driver { module, driver } = owner {
        // A hint only; the Driver's answer does not change the watch.
        let d = host.driver(module, driver)?;
        let _ = d.watch(&host.calls(module), slot.rd, on);
    }
    Ok(Out::Unit)
}

fn close(host: &Host, h: Handle) -> Answer {
    if !local(h.0) {
        host.wire.borrow_mut().close(h).map_err(|f| refused(&f))?;
        return Ok(Out::Unit);
    }
    let slot = {
        let mut s = host.state.borrow_mut();
        s.handles.remove(&h).ok_or(Error::Invalid)?
    };
    let owner = host.state.borrow().nodes.get(&slot.rd).map(|n| n.owner);
    if let Some(Owner::Driver { module, driver }) = owner {
        // The handle is gone whatever the Driver says.
        let d = host.driver(module, driver)?;
        let _ = d.close(&host.calls(module), slot.cookie);
    }
    Ok(Out::Unit)
}

/// The wire drained into the ring first and the answer taken from the
/// ring, so one number means one thing on both sides and a caller holds
/// one cursor and not two.
fn since(host: &Host, cursor: u64) -> Answer {
    let drained = host.wire.borrow_mut().since().map_err(|f| refused(&f))?;
    let mut s = host.state.borrow_mut();
    for event in drained {
        s.push(event);
    }
    let (events, cursor) = s.since(cursor);
    Ok(Out::Events { events, cursor })
}

/// The slot for `h` if its rd still exists and it holds `need`, with
/// the owner that serves it.
fn held(host: &Host, h: Handle, need: Rights) -> Result<(Slot, Owner), Error> {
    let s = host.state.borrow();
    let slot = *s.handles.get(&h).ok_or(Error::Invalid)?;
    let owner = s.nodes.get(&slot.rd).ok_or(Error::Gone)?.owner;
    if !slot.rights.contains(need) {
        return Err(Error::Denied);
    }
    Ok((slot, owner))
}

/// The rd's flags cap the asked rights; asking nothing is `Denied` too.
fn allows(flags: Flags, rights: Rights) -> bool {
    !rights.is_empty()
        && Flags::from_bits(rights.bits())
            .is_some_and(|asked| flags.contains(asked))
}

/// A local Driver's answer. `Pending` is `Invalid`: nothing is pending
/// on one stack, so there would be no token to retire.
fn ready<T>(reply: Reply<T>) -> Result<T, Error> {
    match reply {
        Reply::Ready(result) => result,
        Reply::Pending(_) => Err(Error::Invalid),
    }
}

/// The rd a finished `Lookup` answered.
fn found(answer: Answer) -> Option<Rd> {
    match answer.ok()? {
        Out::Rd(rd) => Some(rd),
        _ => None,
    }
}

/// Up to `len` bytes of `bytes` from `at`, or as many as there are.
fn part(bytes: &[u8], at: u64, len: usize) -> Vec<u8> {
    let start = usize::try_from(at).unwrap_or(usize::MAX).min(bytes.len());
    let end = start.saturating_add(len).min(bytes.len());
    bytes[start..end].to_vec()
}

/// What the link failed with, in the one closed error set: a link that
/// is gone is `Gone`, bytes that are no reply are `Invalid`.
const fn refused(fault: &Fault) -> Error {
    match fault {
        Fault::Kernel(error) => *error,
        Fault::Carrier => Error::Gone,
        Fault::Wire | Fault::Usage(_) => Error::Invalid,
    }
}
