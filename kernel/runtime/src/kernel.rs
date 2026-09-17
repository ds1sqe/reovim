//! The one Kernel: bounded metadata behind a `RefCell`, never borrowed
//! across a vtable call, so a Driver may call back in from any slot.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::fmt::Write as _;

use reovim_kernel_interface::{
    Caller, Decl, Driver, DriverId, Error, Flags, Handle, Hash, Kind, Module,
    ModuleId, Op, Out, Rd, Reply, Rights,
};

use crate::calls::ModuleCalls;
use crate::log::{self, DEBUG, ERR, INFO, Log, NOTICE};
use crate::pending::{Pending, State as Waiting};
use crate::table::{
    CONTAINER, LOG, LOGLEVEL, MODULES, Node, OPEN, Owner, Param, SESSIONS,
    Table,
};
use crate::watch::Ring;

/// What the host's allocator counts of the process heap: the Kernel
/// owns no allocator, so it takes this reading the way it takes time.
/// `/sys/mem` is one of these, and a trace line says what its op cost.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Heap {
    /// Bytes handed out and not given back.
    pub live: u64,
    /// The largest `live` ever reached.
    pub peak: u64,
    /// Allocations made since the process began.
    pub allocs: u64,
}

/// Sizes of the bounded Kernel state. Each is one cell under
/// `/sys/limit`, read and written there and applied as it is written;
/// this is the one place each number is kept.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Events kept for `Since`.
    pub watch_ring: usize,
    /// Outstanding Pending replies per handle; past it `Busy`.
    pub pending_per_handle: usize,
    /// Replies remembered per caller for a repeated sequence.
    pub reply_window: usize,
    /// Entries kept in `/sys/log`.
    pub log_ring: usize,
    /// How deep one op may stand inside another before the Kernel
    /// answers `Busy`; a Driver served from inside its own op is the
    /// chain this bounds.
    pub reentry: u8,
}

impl Limits {
    /// The field `param` names, as its cell reads it.
    #[must_use]
    pub fn get(&self, param: Param) -> u64 {
        let size = |n: usize| u64::try_from(n).unwrap_or(u64::MAX);
        match param {
            Param::WatchRing => size(self.watch_ring),
            Param::Pending => size(self.pending_per_handle),
            Param::ReplyWindow => size(self.reply_window),
            Param::LogRing => size(self.log_ring),
            Param::Reentry => u64::from(self.reentry),
        }
    }

    /// The field `param` names set to `value`, or `Invalid` when the
    /// Kernel cannot hold that number. Every one of the five is a size
    /// the Kernel needs, so `0` is refused for all of them: at `0` a
    /// ring keeps nothing, a cap refuses everything, and `reentry` 0
    /// would refuse every op before it began.
    pub(crate) fn set(
        &mut self,
        param: Param,
        value: u64,
    ) -> Result<(), Error> {
        if value == 0 {
            return Err(Error::Invalid);
        }
        let size = || usize::try_from(value).map_err(|_| Error::Invalid);
        match param {
            Param::WatchRing => self.watch_ring = size()?,
            Param::Pending => self.pending_per_handle = size()?,
            Param::ReplyWindow => self.reply_window = size()?,
            Param::LogRing => self.log_ring = size()?,
            Param::Reentry => {
                self.reentry =
                    u8::try_from(value).map_err(|_| Error::Invalid)?;
            }
        }
        Ok(())
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            watch_ring: 1024,
            pending_per_handle: 16,
            reply_window: 64,
            log_ring: 4096,
            reentry: 8,
        }
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
    /// The Session and client rd of a connection. A Session keeps no
    /// list of its own: the callers it has are the callers this field
    /// names it in, and it ends when none is left.
    pub client: Option<(Rd, Rd)>,
    /// A link is on it now.
    pub live: bool,
}

pub struct State {
    /// The host's monotonic tick, so an entry stamps itself.
    clock: fn() -> u64,
    /// The host's count of the process heap; `/sys/mem` is this read.
    heap: fn() -> Heap,
    /// The five parameters, the one place each is kept. The cells
    /// under `/sys/limit` read and write them here.
    pub limits: Limits,
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
    /// The core each open handle on `/sys/core` serves. A core is one
    /// moment, so the text is made when the handle opens and read from
    /// there: a reader walking it in chunks is never torn by what the
    /// Kernel did between two of them.
    pub cores: BTreeMap<Handle, Vec<u8>>,
    pub next_session: u64,
    pub next_client: u64,
}

impl State {
    pub fn now(&self) -> u64 {
        (self.clock)()
    }

    /// What the host's allocator counts now.
    pub fn heap(&self) -> Heap {
        (self.heap)()
    }

    /// One event on `rd`, and at `debug` one line saying so. The log's
    /// own rds are left out: the line an event on them would write
    /// emits an event of its own, which would write the next.
    pub fn emit(&mut self, rd: Rd, kind: Kind, child: Option<Rd>) {
        self.events.push(rd, kind, child, self.limits.watch_ring);
        if rd == LOG || rd == LOGLEVEL {
            return;
        }
        let seq = self.events.head();
        self.say(DEBUG, || {
            let child = child
                .map_or(String::new(), |c| alloc::format!(" child={}", c.0));
            alloc::format!(
                "event rd={} kind={} seq={seq}{child}",
                rd.0,
                kind_name(kind)
            )
        });
    }

    /// One `/sys/log` entry at `level`, kept only when the level keeps
    /// it; its watchers get `Modify`.
    pub fn log(&mut self, mark: &str, level: u8, bytes: Vec<u8>) {
        if !self.log.keeps(level) {
            return;
        }
        let now = self.now();
        self.log.append(now, mark, level, bytes, self.limits.log_ring);
        self.emit(LOG, Kind::Modify, None);
    }

    /// One `kernel` entry, built only when the level keeps it: a line
    /// the level turns off is never formatted.
    pub fn say(&mut self, level: u8, line: impl FnOnce() -> String) {
        if self.log.keeps(level) {
            self.log("kernel", level, line().into_bytes());
        }
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
        self.forget(caller);
    }

    /// `rd` and its subtree gone, `Delete` on `parent`. The bytes of a
    /// cell go with it; an rd id is never given out twice, so what is
    /// left behind here is left forever.
    pub fn unregister(&mut self, rd: Rd, parent: Rd) {
        for gone in self.table.subtree(rd) {
            self.emit(gone, Kind::Gone, None);
            self.cells.remove(&gone);
            self.table.remove(gone);
        }
        self.emit(parent, Kind::Delete, Some(rd));
    }

    /// The caller is gone: its client rd goes with it, and the Session
    /// it was in ends once no caller is left in it. A named caller is
    /// never forgotten this way, so a Session it is in stays for the
    /// `Connect` that adopts it.
    fn forget(&mut self, caller: Caller) {
        let client = self.callers.remove(&caller).and_then(|c| c.client);
        let Some((session, rd)) = client else { return };
        if let Some(parent) = self.table.get(rd).ok().and_then(|e| e.parent) {
            self.unregister(rd, parent);
        }
        let inside =
            |c: &CallerState| c.client.is_some_and(|(at, _)| at == session);
        if self.callers.values().any(inside) {
            return;
        }
        self.session_end(session);
    }

    /// The Session is over: its subtree is unregistered deepest first,
    /// every Module's per-session state with it, and `/session` sees
    /// the `Delete`.
    fn session_end(&mut self, session: Rd) {
        if self.table.exists(session) {
            self.unregister(session, SESSIONS);
        }
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

/// The kinds in the Kernel's own words, as a traced event says them.
const fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Modify => "modify",
        Kind::Attrib => "attrib",
        Kind::Create => "create",
        Kind::Delete => "delete",
        Kind::Move => "move",
        Kind::Gone => "gone",
        Kind::Overflow => "overflow",
    }
}

/// How deep `run` stands while one call is in it. It falls on drop,
/// so an early return, an error or a panic in a slot leaves the count
/// where it found it.
pub struct Depth<'k>(&'k Cell<u8>);

impl Drop for Depth<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get().saturating_sub(1));
    }
}

/// The Kernel. See the crate docs.
pub struct Kernel {
    clock: fn() -> u64,
    heap: fn() -> Heap,
    /// Ops standing inside ops. The Kernel is single-threaded, so the
    /// re-entrant chain is the only chain this can be counting.
    depth: Cell<u8>,
    pub(crate) state: RefCell<State>,
}

impl Kernel {
    /// `clock` is the host's monotonic tick, which stamps times and log
    /// entries; `heap` is the host's count of the process heap, which
    /// `/sys/mem` and the `heap=` on a trace line read. `limits` are
    /// the parameters the Kernel starts with; `/sys/limit` holds them
    /// from then on.
    #[must_use]
    pub fn new(clock: fn() -> u64, heap: fn() -> Heap, limits: Limits) -> Self {
        let state = State {
            clock,
            heap,
            limits,
            table: Table::new(clock()),
            decls: BTreeMap::new(),
            modules: Vec::new(),
            drivers: Vec::new(),
            handles: BTreeMap::new(),
            next_handle: 1,
            callers: BTreeMap::new(),
            next_caller: 1,
            pending: Pending::new(),
            events: Ring::new(),
            log: Log::new(),
            cells: BTreeMap::new(),
            cores: BTreeMap::new(),
            next_session: 1,
            next_client: 1,
        };
        let depth = Cell::new(0);
        Self { clock, heap, depth, state: RefCell::new(state) }
    }

    pub(crate) fn now(&self) -> u64 {
        (self.clock)()
    }

    /// What the host's allocator counts now.
    pub(crate) fn heap(&self) -> Heap {
        (self.heap)()
    }

    /// Everything the Kernel holds, as the text `/sys/core` serves, or
    /// `None` while the `State` is borrowed. A Kernel that cannot take
    /// its own lock says nothing, which is what a panic under the
    /// borrow leaves; the caller is a panic hook, and a hook that
    /// panicked itself would take the process down twice.
    #[must_use]
    pub fn core(&self) -> Option<Vec<u8>> {
        self.state.try_borrow().ok().map(|s| core(&s))
    }

    /// One `kernel` entry, built only when the level keeps it.
    pub(crate) fn say(&self, level: u8, line: impl FnOnce() -> String) {
        self.state.borrow_mut().say(level, line);
    }

    /// Whether an entry at `level` would be kept.
    pub(crate) fn logs(&self, level: u8) -> bool {
        self.state.borrow().log.keeps(level)
    }

    /// The parameters as they stand now, which a write to
    /// `/sys/limit/<name>` may have moved since boot.
    pub(crate) fn limits(&self) -> Limits {
        self.state.borrow().limits
    }

    /// One step deeper into `run`, or `None` at `Limits::reentry`:
    /// the guard is what holds the step, and dropping it takes it back.
    pub(crate) fn deeper(&self) -> Option<Depth<'_>> {
        let depth = self.depth.get();
        if depth >= self.limits().reentry {
            return None;
        }
        self.depth.set(depth + 1);
        Some(Depth(&self.depth))
    }

    /// How deep `run` stands now.
    pub(crate) const fn depth(&self) -> u8 {
        self.depth.get()
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
        let (id, home) = match self.enter(now, name, module, drivers) {
            Ok(both) => both,
            Err(error) => return Err(self.refused(name, error)),
        };
        starter
            .start(&self.calls(id), home)
            .map_err(|error| self.refused(name, error))?;
        Ok(id)
    }

    /// The Module in the table with its home, before `start` runs.
    fn enter(
        &self,
        now: u64,
        name: &str,
        module: Rc<dyn Module>,
        drivers: Vec<Rc<dyn Driver>>,
    ) -> Result<(ModuleId, Rd), Error> {
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
        s.say(INFO, || alloc::format!("load module={name}"));
        Ok((id, home))
    }

    /// A load the Kernel refused, or a Module whose `start` failed:
    /// one `err` entry, and the error back to the caller.
    fn refused(&self, name: &str, error: Error) -> Error {
        self.say(ERR, || {
            alloc::format!("load module={name} refused={error:?}")
        });
        error
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
        let (index, module) = self.state.borrow_mut().take_module(name)?;
        module.stop();
        self.state.borrow_mut().remove_module(index);
        self.say(INFO, || alloc::format!("unload module={name}"));
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
        s.say(INFO, || {
            alloc::format!("connect session={} client={}", session.0, rd.0)
        });
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
    /// handles closed, its watches and reply window dropped, its client
    /// rd unregistered.
    ///
    /// The Session goes with its last caller: once none is left in it,
    /// and so none that could be adopted, the Kernel unregisters it
    /// with everything under it and `/session` sees the `Delete`.
    pub fn caller_end(&self, caller: Caller) {
        self.disconnected(caller);
        let Some(handles) = self.state.borrow_mut().ending(caller) else {
            return;
        };
        if !handles.is_empty() {
            let (caller, open) = (caller.0, handles.len());
            let line =
                || alloc::format!("caller end caller={caller} handles={open}");
            self.say(NOTICE, line);
        }
        for h in handles {
            let _ = self.run(caller, Op::Close { h });
        }
        self.state.borrow_mut().forget(caller);
    }

    /// The counterpart of `connect`: the link on this caller's client
    /// rd is over. A named caller keeps the rd for its next `Connect`
    /// and says so all the same.
    fn disconnected(&self, caller: Caller) {
        let state = self.state.borrow();
        let client = state.callers.get(&caller).and_then(|c| c.client);
        drop(state);
        let Some((session, rd)) = client else { return };
        let (session, client) = (session.0, rd.0);
        let line =
            || alloc::format!("disconnect session={session} client={client}");
        self.say(INFO, line);
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
        let window = s.limits.reply_window;
        let Some(state) = s.callers.get_mut(&caller) else { return };
        state.last_seq = Some(seq);
        state.window.push_back((seq, reply));
        while state.window.len() > window {
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

/// Everything the `State` holds, as one text: `key=value` lines in the
/// log's own form, one line per thing, each beginning with the word of
/// its section. The sections come in one order — `limit`, `module`,
/// `rd`, `cell`, `decl`, `caller`, `handle`, `pending`, `event`, `log`,
/// `mem` — so two cores of the same Kernel differ only where it did.
/// `mem` is last, being the one line that moves as the core is made.
///
/// Nothing here is a Module's: the design gives a Module its own
/// memory, and a Module that wants its state in a core offers a Read
/// rd, which the tree already holds.
pub fn core(s: &State) -> Vec<u8> {
    let mut out = String::new();
    limits(s, &mut out);
    modules(s, &mut out);
    rds(s, &mut out);
    cells(s, &mut out);
    decls(s, &mut out);
    callers(s, &mut out);
    handles(s, &mut out);
    pendings(s, &mut out);
    events(s, &mut out);
    entries(s, &mut out);
    out.push_str(&mem(s.heap()));
    out.into_bytes()
}

/// The `/sys/mem` line: what the host's allocator counts.
pub fn mem(heap: Heap) -> String {
    let Heap { live, peak, allocs } = heap;
    alloc::format!("live={live} peak={peak} allocs={allocs}")
}

/// A number that may be missing, as the core writes one: `-`.
fn or_dash(value: Option<u64>) -> String {
    value.map_or_else(|| String::from("-"), |n| alloc::format!("{n}"))
}

/// `yes` or `no`, the core's word for a flag.
const fn yes(set: bool) -> &'static str {
    if set { "yes" } else { "no" }
}

/// The rights in a flags or rights word, as a listing writes them.
const fn rw(read: bool, write: bool) -> &'static str {
    match (read, write) {
        (true, true) => "rw",
        (true, false) => "r",
        (false, true) => "w",
        (false, false) => "-",
    }
}

fn limits(s: &State, out: &mut String) {
    for param in Param::ALL {
        let _ = writeln!(out, "limit {}={}", param.name(), s.limits.get(param));
    }
    let _ = writeln!(out, "limit loglevel={}", log::name(s.log.level()));
}

fn modules(s: &State, out: &mut String) {
    for (id, loaded) in s.modules.iter().enumerate() {
        let _ = writeln!(
            out,
            "module id={id} name={} loaded={} caller={}",
            loaded.name,
            yes(loaded.module.is_some()),
            loaded.caller.0
        );
    }
}

fn rds(s: &State, out: &mut String) {
    for (rd, entry) in s.table.all() {
        let (node, module) = owner(entry.owner);
        let flags = rw(
            entry.flags.contains(Flags::READ),
            entry.flags.contains(Flags::WRITE),
        );
        let _ = writeln!(
            out,
            "rd rd={} parent={} name={} node={node} flags={flags} \
             ctime={} mtime={} atime={} module={module} apis={}",
            rd.0,
            or_dash(entry.parent.map(|p| p.0)),
            entry.name.as_deref().unwrap_or("-"),
            entry.ctime,
            entry.mtime,
            entry.atime,
            hashes(&entry.apis),
        );
    }
}

/// The apis an rd offers, by hash, or `-` when it offers none.
fn hashes(apis: &[Hash]) -> String {
    if apis.is_empty() {
        return String::from("-");
    }
    let mut out = String::new();
    for (at, hash) in apis.iter().enumerate() {
        let comma = if at == 0 { "" } else { "," };
        let _ = write!(out, "{comma}{:016x}", hash.0);
    }
    out
}

/// What an rd is and whose it is, in the Kernel's own words.
fn owner(owner: Owner) -> (&'static str, String) {
    let module = |id: ModuleId| alloc::format!("{}", id.0);
    match owner {
        Owner::Driver { module: id, .. } => ("driver", module(id)),
        Owner::Kernel(Node::Home(id)) => ("home", module(id)),
        Owner::Kernel(node) => (kind_of(node), String::from("-")),
    }
}

const fn kind_of(node: Node) -> &'static str {
    match node {
        Node::Container => "container",
        Node::Home(_) => "home",
        Node::Decl(_) => "decl",
        Node::Log => "log",
        Node::Level => "level",
        Node::Limit(_) => "limit",
        Node::Mem => "mem",
        Node::Core => "core",
        Node::Cell => "cell",
    }
}

fn cells(s: &State, out: &mut String) {
    for (rd, bytes) in &s.cells {
        let mut hex = String::new();
        for byte in bytes {
            let _ = write!(hex, "{byte:02x}");
        }
        let _ =
            writeln!(out, "cell rd={} len={} bytes={hex}", rd.0, bytes.len());
    }
}

fn decls(s: &State, out: &mut String) {
    for (hash, decl) in &s.decls {
        let _ = writeln!(
            out,
            "decl hash={:016x} api={} ver={}",
            hash.0, decl.api, decl.ver
        );
    }
}

fn callers(s: &State, out: &mut String) {
    for (caller, state) in &s.callers {
        let _ = writeln!(
            out,
            "caller id={} named={} live={} session={} client={} module={}",
            caller.0,
            yes(!state.credential.is_empty()),
            yes(state.live),
            or_dash(state.client.map(|(at, _)| at.0)),
            or_dash(state.client.map(|(_, rd)| rd.0)),
            or_dash(state.module.map(|m| u64::from(m.0))),
        );
    }
}

fn handles(s: &State, out: &mut String) {
    for (h, slot) in &s.handles {
        let rights = rw(
            slot.rights.contains(Rights::READ),
            slot.rights.contains(Rights::WRITE),
        );
        let _ = writeln!(
            out,
            "handle caller={} h={} rd={} rights={rights} watch={}",
            slot.caller.0,
            h.0,
            slot.rd.0,
            if slot.watching { "on" } else { "off" }
        );
    }
}

fn pendings(s: &State, out: &mut String) {
    for (token, wait) in s.pending.all() {
        let caller = s.handles.get(&wait.handle).map(|slot| slot.caller.0);
        let state = match wait.state {
            Waiting::Waiting => "waiting",
            Waiting::Done(_) => "done",
            Waiting::Cancelled => "cancelled",
        };
        let _ = writeln!(
            out,
            "pending token={} caller={} handle={} state={state}",
            token.0,
            or_dash(caller),
            wait.handle.0
        );
    }
}

fn events(s: &State, out: &mut String) {
    for event in s.events.events() {
        let _ = writeln!(
            out,
            "event seq={} rd={} kind={} child={}",
            event.seq,
            event.rd.0,
            kind_name(event.kind),
            or_dash(event.child.map(|c| c.0))
        );
    }
}

fn entries(s: &State, out: &mut String) {
    for entry in s.log.entries() {
        let _ = writeln!(
            out,
            "log seq={} time={} mark={} level={} text={}",
            entry.seq,
            entry.time,
            entry.mark,
            entry.level,
            escaped(&entry.bytes)
        );
    }
}

/// A log entry's bytes on one line: a newline, a backslash and anything
/// outside printable ASCII are escaped, so one entry is one line
/// however it was written and grep finds it whole.
fn escaped(bytes: &[u8]) -> String {
    let mut out = String::new();
    for byte in bytes {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            0x20..=0x7e => out.push(char::from(*byte)),
            other => {
                let _ = write!(out, "\\x{other:02x}");
            }
        }
    }
    out
}
