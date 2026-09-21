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
    Kind, Module, ModuleId, Op, Out, Outcome, Rd, Reply, Rights, Token, allows,
    check_call, part,
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
    /// `/self/<name>`, a Module's home: it registers under it and
    /// `unload` takes it with the rest.
    Home(ModuleId),
    /// A Driver this Client loaded.
    Driver { module: ModuleId, driver: DriverId },
    /// The cell of a declaration a local `report` published, which
    /// stands under the server's `/sys/decls`.
    Decl(Hash),
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

/// A Module this Client loaded: its name under `/self` and its home,
/// which `unload` names it by and takes with it.
struct Loaded {
    name: String,
    module: Rc<dyn Module>,
}

/// The rds on the way to the server's `/sys/decls`, remembered as a
/// lookup walks to them. Both are fixed rows of the Kernel's table, so
/// what was found once stays found: the walk the sdk makes to read a
/// declaration costs one pair of messages per Client and no more, and
/// a run of a local Driver after that reaches the link for nothing.
#[derive(Clone, Copy, Default)]
struct Fixed {
    sys: Option<Rd>,
    decls: Option<Rd>,
}

impl Fixed {
    /// The rd of `name` under `rd`, when it is one of the two steps and
    /// the walk has been made already.
    fn known(self, rd: Rd, name: &str) -> Option<Rd> {
        match name {
            "sys" if rd == ROOT => self.sys,
            "decls" if Some(rd) == self.sys => self.decls,
            _ => None,
        }
    }

    /// One answered step remembered; every other lookup passes.
    fn saw(&mut self, rd: Rd, name: &str, found: Rd) {
        match name {
            "sys" if rd == ROOT => self.sys = Some(found),
            "decls" if Some(rd) == self.sys => self.decls = Some(found),
            _ => {}
        }
    }
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
    /// The Modules behind them, by `ModuleId`; `None` for the Client's
    /// own and for one that has been unloaded.
    modules: Vec<Option<Loaded>>,
    /// Which Module opened each handle, either side of the union: what
    /// `unload` closes for a Module that is going.
    openers: BTreeMap<Handle, ModuleId>,
    /// What a local `report` published, by hash.
    decls: BTreeMap<Hash, Decl>,
    /// The two rds on the way to the server's `/sys/decls`.
    fixed: Fixed,
    events: VecDeque<Event>,
    next_seq: u64,
    /// The handles a served `Op::Watch` turned on, by the rd each is
    /// on: what a change on this Client is announced for, and nothing
    /// else. A served `Op::Close` takes one out, as the Host's own
    /// `close` takes the slot.
    remote: BTreeMap<Handle, Rd>,
    /// The rds `unregister` took out and the sweep has not said `Gone`
    /// of yet, the root of each subtree and not what stood under it:
    /// what retires their rows from the Kernel's mirror. A Client does
    /// not know which of its rds a remote mirrored, so every one is
    /// said and the mirror drops what it has no row for.
    gone: Vec<Rd>,
    /// The names [`Host::unload`] refuses: the Host's own row and the
    /// Modules a Client is nothing without.
    kept: Vec<String>,
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

    /// `rd` and its subtree gone, deepest first, `Delete` on `parent`,
    /// and `rd` kept for the sweep to say `Gone` of.
    fn unregister(&mut self, rd: Rd, parent: Rd) {
        self.gone.push(rd);
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

    /// The Module loaded under `name`, taken out of the table: it is
    /// loaded no more, whatever the rest of `unload` then does.
    fn take_module(&mut self, name: &str) -> Result<(ModuleId, Loaded), Error> {
        let named = |m: &&mut Option<Loaded>| {
            m.as_ref().is_some_and(|m| m.name == name)
        };
        let at = self.modules.iter_mut().position(|m| named(&m));
        let index = at.ok_or(Error::Invalid)?;
        let id = ModuleId(u32::try_from(index).map_err(|_| Error::Invalid)?);
        let loaded = self.modules[index].take().ok_or(Error::Invalid)?;
        Ok((id, loaded))
    }

    /// Every handle `module` opened, either side of the union, and
    /// every handle anybody holds on the rds `mine`.
    fn held_by(&self, module: ModuleId, mine: &[Rd]) -> Vec<Handle> {
        let on_mine =
            |(h, slot): (&Handle, &Slot)| mine.contains(&slot.rd).then_some(*h);
        let opened = self.openers.iter().filter(|(_, m)| **m == module);
        let mut held: Vec<Handle> =
            self.handles.iter().filter_map(on_mine).collect();
        held.extend(opened.map(|(h, _)| *h));
        held
    }

    /// The rds of `mine` whose parent is not one of them: what
    /// `unregister` takes, each with the subtree below it.
    fn roots_of(&self, mine: &[Rd]) -> Vec<(Rd, Rd)> {
        let outside = |rd: &&Rd| {
            let parent = self.nodes.get(rd).map(|node| node.parent);
            parent.is_some_and(|parent| !mine.contains(&parent))
        };
        mine.iter()
            .filter(outside)
            .map(|rd| (*rd, self.nodes[rd].parent))
            .collect()
    }

    /// Every rd `module` serves, its home included, by owner and not by
    /// subtree: a Module may register outside its home, as one under a
    /// `bin` does.
    fn served_by(&self, module: ModuleId) -> Vec<Rd> {
        let mine = |owner: Owner| match owner {
            Owner::Driver { module: m, .. } | Owner::Home(m) => m == module,
            Owner::Container | Owner::Decl(_) => false,
        };
        self.nodes
            .iter()
            .filter(|(_, node)| mine(node.owner))
            .map(|(rd, _)| *rd)
            .collect()
    }

    /// Whether this event is one a remote is owed: a change of the two
    /// kinds `Calls::changed` takes, on an rd a served watch holds.
    /// Everything else the ring holds is somebody's local business.
    fn announces(&self, event: &Event) -> bool {
        matches!(event.kind, Kind::Modify | Kind::Attrib)
            && self.remote.values().any(|rd| *rd == event.rd)
    }

    /// What a remote is owed since `cursor`, and where the next sweep
    /// begins: one change of the two kinds per rd a remote watches.
    fn owed(&self, cursor: u64) -> (Vec<(Rd, Kind)>, u64) {
        let (events, head) = self.since(cursor);
        if events.iter().any(|event| event.kind == Kind::Overflow) {
            return (self.missed(), head);
        }
        let told = |event: &&Event| self.announces(event);
        let of = |event: &Event| (event.rd, event.kind);
        (events.iter().filter(told).map(of).collect(), head)
    }

    /// Every rd a remote watches, once, as a `Modify`: what an
    /// overflow is owed, there being no change left to name.
    fn missed(&self) -> Vec<(Rd, Kind)> {
        let mut rds: Vec<Rd> = self.remote.values().copied().collect();
        rds.sort_unstable();
        rds.dedup();
        rds.into_iter().map(|rd| (rd, Kind::Modify)).collect()
    }

    /// Events after `cursor` and the cursor for the next ask. A cursor
    /// the ring no longer covers yields one `Overflow` on `ROOT`; the
    /// one number no event can stand past is the head of the ring and
    /// has missed nothing.
    fn since(&self, cursor: u64) -> (Vec<Event>, u64) {
        let head = self.next_seq - 1;
        let oldest = self.events.front().map_or(self.next_seq, |e| e.seq);
        if cursor.saturating_add(1) < oldest {
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
    /// How far the sweep of [`Host::serve`] has read the ring. It is
    /// the Host's own and is held for this and nothing else, the ring
    /// being read and not drained, so a local Plugin's cursor means
    /// what it always meant.
    said: Cell<u64>,
}

impl Host {
    /// The Host over `wire`, holding `/self` and the `bin` a Module
    /// this Client loads registers its names under, and the Client's
    /// own `ModuleId(0)`, whose Driver [`Self::serves`] gives it.
    /// `clock` is the host's monotonic tick, which stamps the times of
    /// a local rd.
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
            modules: alloc::vec![None],
            openers: BTreeMap::new(),
            decls: BTreeMap::new(),
            fixed: Fixed::default(),
            events: VecDeque::new(),
            next_seq: 1,
            remote: BTreeMap::new(),
            gone: Vec::new(),
            // The row below stands under no Module, so `unload` finds
            // none to take out: the name is refused and not mistaken
            // for one nobody loaded.
            kept: alloc::vec![String::from("bin")],
        };
        // A Container of the Host's own, as `/bin` is a fixed row of
        // the Kernel's table: a Module the Client loads registers its
        // names here, and `check_parent` admits any Module under it.
        state.insert(SELF, Some("bin"), Owner::Container, flags, now);
        let depth = Cell::new(0);
        Self {
            wire: RefCell::new(wire),
            state: RefCell::new(state),
            depth,
            clock,
            said: Cell::new(0),
        }
    }

    /// The `Client` behind the Host, for a caller that must speak to
    /// the link and not to the tree: its capability, its client rd, and
    /// the two messages `Calls` cannot spell, `load` and `unload`.
    #[must_use]
    pub fn wire(&self) -> RefMut<'_, Client> {
        self.wire.borrow_mut()
    }

    /// Every op a remote asked of this Client's tree, run here and
    /// answered, and then every change on an rd a remote watches
    /// announced; how many ops were run.
    ///
    /// Called from the host's own loop and nowhere else. At any depth
    /// but zero the Host is inside a slot of its own, and a served
    /// read of a region's view inside that region's own view read is
    /// the one `RefCell` borrowed twice; both guards below make that a
    /// nothing rather than a panic. The list itself rides a `Since`
    /// the loop already makes, so serving nothing costs no message.
    ///
    /// With nobody watching, the sweep walks an empty map and sends
    /// nothing, and the Host's own `emit` kept no event for it to walk
    /// in the first place: nothing crosses while nobody reads, and it
    /// is those two gates and no third.
    pub fn serve(&self) -> usize {
        if self.depth.get() != 0 {
            return 0;
        }
        let Ok(mut wire) = self.wire.try_borrow_mut() else {
            return 0;
        };
        let jobs = wire.take_served();
        drop(wire);
        for (token, op) in &jobs {
            // `ModuleId(0)` is the Client's own, which no `unload`
            // reaches, so a remote's handle outlives every Plugin.
            let outcome = self.run(ModuleId(0), op.clone());
            self.noted(op, &outcome);
            let done = Op::Done { token: *token, outcome };
            let _ = self.wire.borrow_mut().request(done);
        }
        self.announce();
        jobs.len()
    }

    /// What a served op does to the rds this Client announces for: an
    /// `Op::Watch` that answered puts its handle's rd in or takes it
    /// out, and an `Op::Close` takes it out whatever it answered, the
    /// handle being gone either way. Every other op leaves it alone.
    fn noted(&self, op: &Op, answer: &Answer) {
        let mut s = self.state.borrow_mut();
        let on = match (op, answer) {
            (Op::Close { h }, _) | (Op::Watch { h, on: false }, Ok(_)) => {
                s.remote.remove(h);
                return;
            }
            (Op::Watch { h, on: true }, Ok(_)) => *h,
            _ => return,
        };
        let Some(rd) = s.handles.get(&on).map(|slot| slot.rd) else { return };
        s.remote.insert(on, rd);
    }

    /// One `Op::Changed` per change on an rd a remote watches, from
    /// this Host's own place in its own ring, and then one `Gone` per
    /// rd `unregister` took out since the last sweep, watched or not.
    /// Everything else the ring holds is somebody's local business and
    /// is passed over.
    ///
    /// A ring that ran past this place is one `Overflow`, which
    /// `Op::Changed` cannot carry: a change is `Modify` or `Attrib`
    /// and no third kind. So every rd a remote watches is told
    /// `Modify` instead and reads the cell again, which is what a
    /// watcher told of a change does anyway; being told nothing is the
    /// one thing a remote that missed something may not be. What went
    /// is not in the ring and is not lost with it.
    fn announce(&self) {
        let said: Vec<(Rd, Kind)> = {
            let mut s = self.state.borrow_mut();
            let (mut said, cursor) = s.owed(self.said.get());
            self.said.set(cursor);
            let gone = core::mem::take(&mut s.gone);
            said.extend(gone.into_iter().map(|rd| (rd, Kind::Gone)));
            said
        };
        for (rd, kind) in said {
            let told = Op::Changed { rd, kind };
            let _ = self.wire.borrow_mut().request(told);
        }
    }

    /// Load a Module: make `/self/<name>`, then `start` it, as the
    /// Kernel loads one under `/module`.
    ///
    /// # Errors
    /// `Invalid` for a malformed or taken name; whatever `start`
    /// returns. A Module that will not start is not loaded: what it
    /// registered before it failed goes out with its home, so the name
    /// is free and the load may be made again.
    pub fn load(
        &self,
        name: &str,
        module: Rc<dyn Module>,
        drivers: Vec<Rc<dyn Driver>>,
    ) -> Result<ModuleId, Error> {
        let now = self.now();
        let starter = module.clone();
        let (id, home) = {
            let mut s = self.state.borrow_mut();
            s.check_name(SELF, Some(name))?;
            let index = s.modules.len();
            let id =
                ModuleId(u32::try_from(index).map_err(|_| Error::NoSpace)?);
            s.drivers.push(drivers);
            let flags = Flags::READ.union(Flags::CONTAINER);
            let home = s.insert(SELF, Some(name), Owner::Home(id), flags, now);
            let name = name.to_string();
            s.modules.push(Some(Loaded { name, module }));
            s.emit(SELF, Kind::Create, Some(home));
            (id, home)
        };
        if let Err(error) = starter.start(&self.calls(id), home) {
            let _ = self.shed(name);
            return Err(error);
        }
        Ok(id)
    }

    /// The Module loaded under `name` is the Client's own: `unload`
    /// refuses it, whoever asks, until [`Self::shed`] takes it out.
    /// `/self/dev` is one, and so is the Plugin holding the region,
    /// which `/self/dev` gives away and nobody else does.
    pub(crate) fn keeps(&self, name: &str) {
        let mut s = self.state.borrow_mut();
        if !s.kept.iter().any(|kept| kept == name) {
            s.kept.push(name.to_string());
        }
    }

    /// Unload the Module named `name`, asked for by name: what
    /// `/self/bin/unload` runs and what anything above the Client may
    /// ask for.
    ///
    /// # Errors
    /// `Denied` for a name the Client is nothing without, which its
    /// own devices, the row the Host serves itself and the Plugin
    /// holding the region are; `Invalid` when no such Module is
    /// loaded.
    pub fn unload(&self, name: &str) -> Result<(), Error> {
        if self.state.borrow().kept.iter().any(|kept| kept == name) {
            return Err(Error::Denied);
        }
        self.shed(name)
    }

    /// The Module named `name` out, kept or not: every handle it opened
    /// and every handle on what it serves closed, then `stop`, then
    /// every rd it serves gone.
    ///
    /// Where the Kernel refuses an open handle a Client must not: the
    /// Module is going and nobody is left to close for it.
    ///
    /// # Errors
    /// `Invalid` when no such Module is loaded.
    pub(crate) fn shed(&self, name: &str) -> Result<(), Error> {
        let (id, loaded) = self.state.borrow_mut().take_module(name)?;
        let mine = self.state.borrow().served_by(id);
        // The borrow is gone before a handle closes: a Driver's own
        // `close` slot may ask this Host anything.
        let held = self.state.borrow().held_by(id, &mine);
        for h in held {
            let _ = self.run(id, Op::Close { h });
        }
        loaded.module.stop();
        let mut s = self.state.borrow_mut();
        for (root, parent) in s.roots_of(&mine) {
            s.unregister(root, parent);
        }
        s.drivers[id.0 as usize] = Vec::new();
        // The Module that name stood for is gone, so nothing is kept
        // under it and a later one may be unloaded by anybody.
        s.kept.retain(|kept| kept != name);
        Ok(())
    }

    /// The Client's own Driver, `ModuleId(0)`'s `DriverId(0)`: what
    /// the Host serves itself, standing under no Module and no home,
    /// so no `unload` reaches it. [`Self::own`] is what registers its
    /// names, `/self/bin`'s `load` and `unload` being the two.
    pub fn serves(&self, driver: Rc<dyn Driver>) {
        self.state.borrow_mut().drivers[0] = alloc::vec![driver];
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

    pub(crate) fn now(&self) -> u64 {
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
    ///
    /// `module` is who is asking, which only `Open` uses: a handle is
    /// remembered as its opener's, so `unload` closes it.
    fn run(&self, module: ModuleId, op: Op) -> Answer {
        let Some(_depth) = self.deeper() else { return Err(Error::Busy) };
        match op {
            Op::Lookup { rd, name } => lookup(self, rd, &name),
            Op::Info { rd } => info(self, rd),
            Op::Parent { rd } => parent(self, rd),
            Op::Children { rd } => children(self, rd),
            Op::Open { rd, rights } => open(self, module, rd, rights),
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
            // What this Client was asked to serve is answered by
            // [`Self::serve`], which is also what announces; neither is
            // asked of anybody here.
            Op::Done { .. } | Op::Changed { .. } => Err(Error::Unsupported),
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
        match self.host.run(self.module, Op::Open { rd, rights })? {
            Out::Handle(handle) => Ok(handle),
            _ => Err(Error::Unsupported),
        }
    }

    fn submit(&self, op: Op) -> Reply<Out> {
        Reply::Ready(self.host.run(self.module, op))
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
        Owner::Driver { module: m, .. } | Owner::Home(m) => m == module,
        Owner::Decl(_) => false,
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
        if let Some(known) = host.state.borrow().fixed.known(rd, name) {
            return Ok(Out::Rd(known));
        }
        if let Some(cell) = published(host, rd, name) {
            return Ok(Out::Rd(cell));
        }
        let out = host.sent(Op::Lookup { rd, name: name.to_string() })?;
        if let Out::Rd(found) = out {
            host.state.borrow_mut().fixed.saw(rd, name, found);
        }
        return Ok(out);
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
/// local op pays a wire message for it; a reader that walked there
/// itself has left the way in [`Fixed`] already.
fn decls_rd(host: &Host) -> Option<Rd> {
    if let Some(rd) = host.state.borrow().fixed.decls {
        return Some(rd);
    }
    let sys = found(host.sent(Op::Lookup { rd: ROOT, name: "sys".into() }))?;
    let rd = found(host.sent(Op::Lookup { rd: sys, name: "decls".into() }))?;
    host.state.borrow_mut().fixed = Fixed { sys: Some(sys), decls: Some(rd) };
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

fn open(host: &Host, module: ModuleId, rd: Rd, rights: Rights) -> Answer {
    if !local(rd.0) {
        let opened = host.wire.borrow_mut().open(rd, rights);
        let h = opened.map_err(|f| refused(&f))?;
        host.state.borrow_mut().openers.insert(h, module);
        return Ok(Out::Handle(h));
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
        Owner::Container | Owner::Home(_) | Owner::Decl(_) => 0,
    };
    let mut s = host.state.borrow_mut();
    let h = Handle(s.next_handle);
    s.next_handle += 1;
    s.handles.insert(h, Slot { rd, rights, cookie, watching: false });
    s.openers.insert(h, module);
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
        Owner::Container | Owner::Home(_) => return Err(Error::Unsupported),
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
    let Owner::Driver { module, driver } = owner else {
        return Err(Error::Unsupported);
    };
    let d = host.driver(module, driver)?;
    let calls = host.calls(module);
    let took = ready(d.write(&calls, slot.cookie, at, &bytes))?;
    Ok(Out::Len(u32::try_from(took).unwrap_or(u32::MAX)))
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
    {
        let s = host.state.borrow();
        let node = s.nodes.get(&slot.rd).ok_or(Error::Gone)?;
        check_call(slot.rights, &node.apis, &s.decls, api, method, &input)?;
    }
    let Owner::Driver { module, driver } = owner else {
        return Err(Error::Unsupported);
    };
    let d = host.driver(module, driver)?;
    let calls = host.calls(module);
    let out = ready(d.call(&calls, slot.cookie, api, method, &input))?;
    Ok(Out::Bytes(out))
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
    host.state.borrow_mut().openers.remove(&h);
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
    // The ops the server asked of this Client are not events: the
    // wire's own list is kept for [`Host::serve`], which is the one
    // thing that runs one.
    Ok(Out::Events { events, cursor, serve: Vec::new() })
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

/// What the link failed with, in the one closed error set: a link that
/// is gone is `Gone`, bytes that are no reply are `Invalid`.
pub const fn refused(fault: &Fault) -> Error {
    match fault {
        Fault::Kernel(error) => *error,
        Fault::Carrier => Error::Gone,
        Fault::Wire => Error::Invalid,
    }
}
