//! The one Kernel: bounded metadata behind a `RefCell`, never borrowed
//! across a vtable call, so a Driver may call back in from any slot.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_kernel_interface::{
    Caller, Decl, Driver, DriverId, Error, Handle, Hash, Kind, Module,
    ModuleId, Op, Out, Rd, Reply, Rights,
};

use crate::calls::ModuleCalls;
use crate::log::Log;
use crate::pending::Pending;
use crate::table::{MODULES, Node, OPEN, Owner, SESSIONS, Table};
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
    /// Global driver indexes, in `DriverId` order.
    pub drivers: Vec<usize>,
    pub caller: Caller,
}

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
}

pub struct State {
    pub table: Table,
    pub decls: BTreeMap<Hash, Decl>,
    pub modules: Vec<Loaded>,
    pub drivers: Vec<Arc<dyn Driver>>,
    pub handles: BTreeMap<Handle, Slot>,
    pub next_handle: u64,
    pub callers: BTreeMap<Caller, CallerState>,
    pub next_caller: u64,
    pub pending: Pending,
    pub events: Ring,
    pub log: Log,
    pub next_session: u64,
}

impl State {
    pub fn emit(&mut self, rd: Rd, kind: Kind, child: Option<Rd>) {
        self.events.push(rd, kind, child);
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

    fn new_caller(&mut self, module: Option<ModuleId>) -> Caller {
        let caller = Caller(self.next_caller);
        self.next_caller += 1;
        let state = CallerState {
            module,
            last_seq: None,
            window: VecDeque::new(),
            watched: BTreeMap::new(),
        };
        self.callers.insert(caller, state);
        caller
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
            next_session: 1,
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
    /// unload comes with milestone 3.
    pub fn load(
        &self,
        name: &str,
        module: &dyn Module,
        drivers: Vec<Arc<dyn Driver>>,
    ) -> Result<ModuleId, Error> {
        let now = self.now();
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
            let loaded =
                Loaded { name: String::from(name), drivers: indexes, caller };
            s.modules.push(loaded);
            (id, home)
        };
        module.start(&self.calls(id), home)?;
        Ok(id)
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

    /// One request. A `seq` seen before is answered from memory and the
    /// op is not run; one older than the window is `Stale`.
    pub fn submit(&self, caller: Caller, seq: u64, op: Op) -> Reply<Out> {
        match self.replay(caller, seq) {
            Ok(Some(reply)) => return reply,
            Ok(None) => {}
            Err(error) => return Reply::err(error),
        }
        let reply = self.run(caller, op);
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
    pub(crate) fn driver(&self, index: usize) -> Arc<dyn Driver> {
        self.state.borrow().drivers[index].clone()
    }

    /// A Module's Driver with its global index.
    pub(crate) fn served(
        &self,
        module: ModuleId,
        driver: DriverId,
    ) -> Result<(usize, Arc<dyn Driver>), Error> {
        let s = self.state.borrow();
        let index = s.driver_index(module, driver)?;
        Ok((index, s.drivers[index].clone()))
    }
}
