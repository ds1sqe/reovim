//! The one Kernel: bounded metadata behind a `RefCell`, never borrowed
//! across a vtable call, so a Driver may call back in from any slot.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_kernel_interface::{
    Caller, Decl, Driver, DriverId, Error, Flags, Handle, Hash, Kind, Module,
    ModuleId, Op, Out, Rd, Reply, Rights,
};

use crate::calls::ModuleCalls;
use crate::log::Log;
use crate::pending::Pending;
use crate::table::{
    CONTAINER, LOG, MODULES, Node, OPEN, Owner, SESSIONS, Table,
};
use crate::watch::Ring;

/// Sizes of the bounded Kernel state.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Events kept for `Since`.
    pub watch_ring: usize,
    /// Outstanding Pending replies per handle; past it `Busy`.
    pub pending_per_handle: usize,
    /// Replies remembered per caller for a repeated sequence.
    pub reply_window: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { watch_ring: 1024, pending_per_handle: 16, reply_window: 64 }
    }
}

pub struct Loaded {
    pub name: String,
    /// The Module while loaded; `None` after `unload`.
    pub module: Option<Rc<dyn Module>>,
    /// Global driver indexes, in `DriverId` order.
    pub drivers: Vec<usize>,
    pub caller: Caller,
}

/// What an unloaded Module's Driver slots hold: every op `Unsupported`.
struct Unloaded;

impl Driver for Unloaded {}

/// One open handle.
#[derive(Clone)]
pub struct Slot {
    pub caller: Caller,
    pub rd: Rd,
    pub rights: Rights,
    /// The serving Module and global driver index; `None` for the Kernel.
    pub served: Option<(ModuleId, usize)>,
    /// The Driver's own handle value.
    pub cookie: u64,
    pub pending: usize,
    pub watching: bool,
}

pub struct CallerState {
    pub module: Option<ModuleId>,
    pub last_seq: Option<u64>,
    pub window: VecDeque<(u64, Reply<Out>)>,
    pub watched: BTreeMap<Rd, usize>,
    /// What a connection presented; empty for a Module or an unnamed
    /// link. A named caller survives its link for the next `Connect`.
    pub credential: Vec<u8>,
    /// The Session and client rd of a connection.
    pub client: Option<(Rd, Rd)>,
    /// A link is on it now.
    pub live: bool,
}

pub struct State {
    pub table: Table,
    pub decls: BTreeMap<Hash, Decl>,
    pub modules: Vec<Loaded>,
    pub drivers: Vec<Rc<dyn Driver>>,
    pub handles: BTreeMap<Handle, Slot>,
    pub next_handle: u64,
    pub callers: BTreeMap<Caller, CallerState>,
    pub next_caller: u64,
    pub pending: Pending,
    pub events: Ring,
    pub log: Log,
    /// The bytes of every `Node::Cell`.
    pub cells: BTreeMap<Rd, Vec<u8>>,
    pub next_session: u64,
    pub next_client: u64,
}

impl State {
    pub fn emit(&mut self, rd: Rd, kind: Kind, child: Option<Rd>) {
        self.events.push(rd, kind, child);
    }

    /// One `/sys/log` entry; its watchers get `Modify`.
    pub fn log(&mut self, now: u64, mark: &str, bytes: Vec<u8>) {
        self.log.append(now, mark, 0, bytes);
        self.emit(LOG, Kind::Modify, None);
    }

    pub fn driver_index(
        &self,
        module: ModuleId,
        driver: DriverId,
    ) -> Result<usize, Error> {
        let loaded =
            self.modules.get(module.0 as usize).ok_or(Error::Invalid)?;
        loaded.drivers.get(driver.0 as usize).copied().ok_or(Error::Invalid)
    }

    pub fn watched(&self, caller: Caller) -> BTreeSet<Rd> {
        self.callers
            .get(&caller)
            .map(|c| c.watched.keys().copied().collect())
            .unwrap_or_default()
    }

    pub fn mark(&self, caller: Caller) -> &str {
        self.callers
            .get(&caller)
            .and_then(|c| c.module)
            .and_then(|m| self.modules.get(m.0 as usize))
            .map_or("caller", |m| m.name.as_str())
    }

    /// The loaded Module named `name`, taken out of its slot once no
    /// handle it serves or holds is open.
    fn take_module(
        &mut self,
        name: &str,
    ) -> Result<(usize, Rc<dyn Module>), Error> {
        let index = self
            .modules
            .iter()
            .position(|m| m.name == name && m.module.is_some())
            .ok_or(Error::Invalid)?;
        let id = ModuleId(u32::try_from(index).map_err(|_| Error::Invalid)?);
        let caller = self.modules[index].caller;
        let held = |slot: &Slot| {
            slot.caller == caller || slot.served.is_some_and(|(m, _)| m == id)
        };
        if self.handles.values().any(held) {
            return Err(Error::Busy);
        }
        let module = self.modules[index].module.take().ok_or(Error::Invalid)?;
        Ok((index, module))
    }

    /// Every rd the Module at `index` serves, its home included, gone
    /// with the events of `unregister`; its Drivers and caller dropped.
    fn remove_module(&mut self, index: usize) {
        let id = ModuleId(u32::try_from(index).unwrap_or(u32::MAX));
        let served = |owner: Owner| match owner {
            Owner::Driver { module, .. }
            | Owner::Kernel(Node::Home(module)) => module == id,
            Owner::Kernel(_) => false,
        };
        let mine: Vec<Rd> = self
            .table
            .all()
            .filter(|(_, e)| served(e.owner))
            .map(|(rd, _)| rd)
            .collect();
        let parent_of = |rd: Rd| self.table.get(rd).ok().and_then(|e| e.parent);
        let roots: Vec<(Rd, Rd)> = mine
            .iter()
            .filter_map(|rd| parent_of(*rd).map(|p| (*rd, p)))
            .filter(|(_, p)| !mine.contains(p))
            .collect();
        for (root, parent) in roots {
            self.unregister(root, parent);
        }
        for i in core::mem::take(&mut self.modules[index].drivers) {
            self.drivers[i] = Rc::new(Unloaded);
        }
        let caller = self.modules[index].caller;
        self.callers.remove(&caller);
    }

    /// `rd` and its subtree gone, `Delete` on `parent`.
    pub fn unregister(&mut self, rd: Rd, parent: Rd) {
        for gone in self.table.subtree(rd) {
            self.emit(gone, Kind::Gone, None);
            self.table.remove(gone);
        }
        self.emit(parent, Kind::Delete, Some(rd));
    }

    fn new_caller(&mut self, module: Option<ModuleId>) -> Caller {
        let caller = Caller(self.next_caller);
        self.next_caller += 1;
        let state = CallerState {
            module,
            last_seq: None,
            window: VecDeque::new(),
            watched: BTreeMap::new(),
            credential: Vec::new(),
            client: None,
            live: false,
        };
        self.callers.insert(caller, state);
        caller
    }

    /// The caller's link ended: `None` when it is kept for its next
    /// `Connect`, else the handles to close before it goes.
    fn ending(&mut self, caller: Caller) -> Option<Vec<Handle>> {
        let state = self.callers.get_mut(&caller)?;
        state.live = false;
        if !state.credential.is_empty() {
            return None;
        }
        let mine = self.handles.iter().filter(|(_, s)| s.caller == caller);
        Some(mine.map(|(h, _)| *h).collect())
    }

    /// The caller `credential` named in `session` before, taken over
    /// by a new link; `Busy` while its old link is still on it.
    fn adopt(
        &mut self,
        session: Rd,
        credential: &[u8],
    ) -> Result<Option<(Caller, Rd)>, Error> {
        if credential.is_empty() {
            return Ok(None);
        }
        let named = |c: &CallerState| {
            c.credential == credential
                && c.client.is_some_and(|(at, _)| at == session)
        };
        let Some((caller, state)) =
            self.callers.iter_mut().find(|(_, c)| named(c))
        else {
            return Ok(None);
        };
        if state.live {
            return Err(Error::Busy);
        }
        state.live = true;
        Ok(state.client.map(|(_, client)| (*caller, client)))
    }
}

/// The Kernel. See the crate docs.
pub struct Kernel {
    clock: fn() -> u64,
    limits: Limits,
    pub(crate) state: RefCell<State>,
}

impl Kernel {
    /// `clock` is the host's monotonic tick; the Kernel stamps times and
    /// log entries with it.
    #[must_use]
    pub fn new(clock: fn() -> u64, limits: Limits) -> Self {
        let state = State {
            table: Table::new(clock()),
            decls: BTreeMap::new(),
            modules: Vec::new(),
            drivers: Vec::new(),
            handles: BTreeMap::new(),
            next_handle: 1,
            callers: BTreeMap::new(),
            next_caller: 1,
            pending: Pending::new(),
            events: Ring::new(limits.watch_ring),
            log: Log::new(),
            cells: BTreeMap::new(),
            next_session: 1,
            next_client: 1,
        };
        Self { clock, limits, state: RefCell::new(state) }
    }

    pub(crate) fn now(&self) -> u64 {
        (self.clock)()
    }

    pub(crate) const fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Load a Module: make `/module/<name>`, then `start` it.
    ///
    /// # Errors
    /// `Invalid` for a malformed or taken name; whatever `start` returns.
    /// A failed `start` leaves the Module loaded with what it registered;
    /// `unload` takes it out.
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
            s.table.check_name(MODULES, Some(name))?;
            let id = ModuleId(
                u32::try_from(s.modules.len()).map_err(|_| Error::NoSpace)?,
            );
            let first = s.drivers.len();
            s.drivers.extend(drivers);
            let indexes = (first..s.drivers.len()).collect();
            let caller = s.new_caller(Some(id));
            let owner = Owner::Kernel(Node::Home(id));
            let home = s.table.insert(MODULES, Some(name), owner, OPEN, now);
            s.emit(MODULES, Kind::Create, Some(home));
            let loaded = Loaded {
                name: String::from(name),
                module: Some(module),
                drivers: indexes,
                caller,
            };
            s.modules.push(loaded);
            let line = alloc::format!("load module={name}");
            s.log(now, "kernel", line.into_bytes());
            (id, home)
        };
        starter.start(&self.calls(id), home)?;
        Ok(id)
    }

    /// Unload the Module named `name`: `stop`, then every rd it serves
    /// is gone, its Drivers are dropped and its id answers `Invalid`.
    /// Logged under `kernel`.
    ///
    /// # Errors
    /// `Invalid` when no such Module is loaded; `Busy` while a handle on
    /// an rd it serves, or one it holds itself, is open. Nothing is
    /// revoked.
    pub fn unload(&self, name: &str) -> Result<(), Error> {
        let now = self.now();
        let (index, module) = self.state.borrow_mut().take_module(name)?;
        module.stop();
        let mut s = self.state.borrow_mut();
        s.remove_module(index);
        let line = alloc::format!("unload module={name}");
        s.log(now, "kernel", line.into_bytes());
        Ok(())
    }

    /// The seven calls as Module `module` makes them.
    #[must_use]
    pub const fn calls(&self, module: ModuleId) -> ModuleCalls<'_> {
        ModuleCalls::new(self, module)
    }

    /// A new connection-level caller.
    pub fn caller_new(&self) -> Caller {
        self.state.borrow_mut().new_caller(None)
    }

    /// A new `/session/<id>`: a Kernel Container any Module may register
    /// under.
    pub fn session_new(&self) -> Rd {
        let now = self.now();
        let mut s = self.state.borrow_mut();
        let id = s.next_session;
        s.next_session += 1;
        let name = alloc::format!("{id}");
        let owner = Owner::Kernel(Node::Container);
        let rd = s.table.insert(SESSIONS, Some(&name), owner, OPEN, now);
        s.emit(SESSIONS, Kind::Create, Some(rd));
        rd
    }

    /// `Connect`: the caller and client rd for `credential` in
    /// `session`. An empty credential is a new client rd
    /// `/session/<id>/client/<cid>` every time; a named one is made
    /// once and taken over by every later link. Logged under `kernel`.
    ///
    /// # Errors
    /// `Invalid` unless `session` came from `session_new`; `Busy`
    /// while the caller that credential names still has its link.
    pub fn connect(
        &self,
        session: Rd,
        credential: &[u8],
    ) -> Result<(Caller, Rd), Error> {
        let now = self.now();
        let mut s = self.state.borrow_mut();
        let entry = s.table.get(session)?;
        if entry.parent != Some(SESSIONS) {
            return Err(Error::Invalid);
        }
        if let Some(known) = s.adopt(session, credential)? {
            return Ok(known);
        }
        let owner = Owner::Kernel(Node::Container);
        let clients = s.table.child(session, "client").unwrap_or_else(|| {
            let name = Some("client");
            let rd = s.table.insert(session, name, owner, CONTAINER, now);
            s.emit(session, Kind::Create, Some(rd));
            rd
        });
        let cid = s.next_client;
        s.next_client += 1;
        let name = alloc::format!("{cid}");
        let rd = s.table.insert(clients, Some(&name), owner, CONTAINER, now);
        s.emit(clients, Kind::Create, Some(rd));
        let cell = Owner::Kernel(Node::Cell);
        let flags = Flags::READ.union(Flags::WRITE);
        let capability =
            s.table.insert(rd, Some("capability"), cell, flags, now);
        s.cells.insert(capability, Vec::new());
        s.emit(rd, Kind::Create, Some(capability));
        let line =
            alloc::format!("connect session={} client={}", session.0, rd.0);
        s.log(now, "kernel", line.into_bytes());
        let caller = s.new_caller(None);
        if let Some(state) = s.callers.get_mut(&caller) {
            state.credential = credential.to_vec();
            state.client = Some((session, rd));
            state.live = true;
        }
        Ok((caller, rd))
    }

    /// The caller's link ended. A named caller waits for its next
    /// `Connect` with everything it holds; any other is over: its
    /// handles closed, its watches and reply window dropped. The client
    /// rd stays.
    pub fn caller_end(&self, caller: Caller) {
        let Some(handles) = self.state.borrow_mut().ending(caller) else {
            return;
        };
        for h in handles {
            let _ = self.run(caller, Op::Close { h });
        }
        self.state.borrow_mut().callers.remove(&caller);
    }

    /// One request. A `seq` seen before is answered from memory and the
    /// op is not run; one older than the window is `Stale`.
    pub fn submit(&self, caller: Caller, seq: u64, op: Op) -> Reply<Out> {
        self.once(caller, seq, || self.run(caller, op))
    }

    /// `run` under the reply window: a `seq` seen before is answered
    /// from memory and `run` is not called.
    pub fn once(
        &self,
        caller: Caller,
        seq: u64,
        run: impl FnOnce() -> Reply<Out>,
    ) -> Reply<Out> {
        match self.replay(caller, seq) {
            Ok(Some(reply)) => return reply,
            Ok(None) => {}
            Err(error) => return Reply::err(error),
        }
        let reply = run();
        self.remember(caller, seq, reply.clone());
        reply
    }

    fn replay(
        &self,
        caller: Caller,
        seq: u64,
    ) -> Result<Option<Reply<Out>>, Error> {
        let s = self.state.borrow();
        let state = s.callers.get(&caller).ok_or(Error::Invalid)?;
        let stored = state.window.iter().find(|(at, _)| *at == seq);
        if let Some((_, reply)) = stored {
            return Ok(Some(reply.clone()));
        }
        if state.last_seq.is_some_and(|last| seq <= last) {
            return Err(Error::Stale);
        }
        Ok(None)
    }

    fn remember(&self, caller: Caller, seq: u64, reply: Reply<Out>) {
        let mut s = self.state.borrow_mut();
        let Some(state) = s.callers.get_mut(&caller) else { return };
        state.last_seq = Some(seq);
        state.window.push_back((seq, reply));
        while state.window.len() > self.limits.reply_window {
            state.window.pop_front();
        }
    }

    /// The Driver at a global index, cloned out so no borrow is held
    /// while it runs.
    pub(crate) fn driver(&self, index: usize) -> Rc<dyn Driver> {
        self.state.borrow().drivers[index].clone()
    }

    /// A Module's Driver with its global index.
    pub(crate) fn served(
        &self,
        module: ModuleId,
        driver: DriverId,
    ) -> Result<(usize, Rc<dyn Driver>), Error> {
        let s = self.state.borrow();
        let index = s.driver_index(module, driver)?;
        Ok((index, s.drivers[index].clone()))
    }
}
