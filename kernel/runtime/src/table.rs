//! The rd table: every Resource the Kernel knows, with its cached shape.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_kernel_interface::{
    Caller, DriverId, Error, Flags, Hash, Info, ModuleId, Rd,
};

/// Fixed ids, allocated in this order.
///
/// `/` 0, `/sys` 1, `/sys/decls` 2, `/sys/log` 3, `/module` 4,
/// `/session` 5, `/var` 6, `/sys/loglevel` 7, `/sys/limit` 8 with its
/// five cells 9 to 13, `/sys/core` 14, `/bin` 15, `/sys/info` 16 with
/// its five cells 17 to 21. These rows are made afresh at every boot,
/// so one of them moves when the tree it names moves; an id a run
/// hands out never does, being taken one at a time from `FIRST` and
/// never given twice. Ids below `FIRST` are reserved for the Kernel;
/// everything registered later starts there.
pub const ROOT: Rd = Rd(0);
pub const SYS: Rd = Rd(1);
pub const DECLS: Rd = Rd(2);
pub const LOG: Rd = Rd(3);
pub const MODULES: Rd = Rd(4);
pub const SESSIONS: Rd = Rd(5);
pub const LOGLEVEL: Rd = Rd(7);
pub const LIMITS: Rd = Rd(8);
pub const CORE: Rd = Rd(14);
pub const INFO: Rd = Rd(16);
pub const MEM: Rd = Rd(17);
const FIRST: u64 = 100;

/// A Kernel Container.
pub const CONTAINER: Flags = Flags::READ.union(Flags::CONTAINER);
/// A Kernel Container any Module may register under.
pub const OPEN: Flags = CONTAINER.union(Flags::WRITE);

/// A Kernel cell a caller may read and write.
const RW: Flags = Flags::READ.union(Flags::WRITE);

/// A cell the Kernel reads and serves on the spot; a core is one
/// moment, so nothing may be written to either.
const R: Flags = Flags::READ;

const FIXED: [(Rd, &str, Node, Flags); 16] = [
    (ROOT, "sys", Node::Container, CONTAINER),
    (SYS, "decls", Node::Container, CONTAINER),
    (SYS, "log", Node::Log, RW),
    (ROOT, "module", Node::Container, CONTAINER),
    (ROOT, "session", Node::Container, CONTAINER),
    (ROOT, "var", Node::Container, OPEN),
    (SYS, "loglevel", Node::Level, RW),
    (SYS, "limit", Node::Container, CONTAINER),
    (LIMITS, "watch_ring", Node::Limit(Param::WatchRing), RW),
    (LIMITS, "pending_per_handle", Node::Limit(Param::Pending), RW),
    (LIMITS, "reply_window", Node::Limit(Param::ReplyWindow), RW),
    (LIMITS, "log_ring", Node::Limit(Param::LogRing), RW),
    (LIMITS, "reentry", Node::Limit(Param::Reentry), RW),
    (SYS, "core", Node::Core, R),
    (ROOT, "bin", Node::Container, OPEN),
    (SYS, "info", Node::Container, CONTAINER),
];

/// An rd whose ops are never traced: the log and the level a follower
/// reads, which a line about would feed, and the two the Kernel makes
/// at the read, which a trace would make a reading of itself.
#[must_use]
pub fn untraced(rd: Rd) -> bool {
    [LOG, LOGLEVEL, MEM, CORE].contains(&rd)
}

/// A node the Kernel makes at the read out of what it already holds.
/// Reading one changes nothing, so it leaves no atime behind either.
#[must_use]
pub const fn momentary(node: Node) -> bool {
    matches!(node, Node::Info(_) | Node::Core | Node::Heard(_))
}

/// One field of `Limits`, the cell under `/sys/limit` that holds it.
/// The cells are the Kernel's parameters, as `/proc/sys/kernel/*` are
/// Linux's: one value per file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Param {
    WatchRing,
    Pending,
    ReplyWindow,
    LogRing,
    Reentry,
}

impl Param {
    /// The five in the order `/sys/limit` holds them.
    pub const ALL: [Self; 5] = [
        Self::WatchRing,
        Self::Pending,
        Self::ReplyWindow,
        Self::LogRing,
        Self::Reentry,
    ];

    /// The cell's name, which is the field's.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::WatchRing => "watch_ring",
            Self::Pending => "pending_per_handle",
            Self::ReplyWindow => "reply_window",
            Self::LogRing => "log_ring",
            Self::Reentry => "reentry",
        }
    }
}

/// One thing the Kernel knows about itself, the cell under `/sys/info`
/// that holds it. `/sys/limit` runs the other way: a parameter is
/// written and applied where it is used, a fact is made at the read out
/// of state the Kernel already holds and is kept nowhere.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fact {
    Mem,
    Time,
    Uptime,
    Version,
    Watchers,
}

impl Fact {
    /// The five in the order `/sys/info` holds them.
    pub const ALL: [Self; 5] =
        [Self::Mem, Self::Time, Self::Uptime, Self::Version, Self::Watchers];

    /// The cell's name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Mem => "mem",
            Self::Time => "time",
            Self::Uptime => "uptime",
            Self::Version => "version",
            Self::Watchers => "watchers",
        }
    }
}

/// Who serves an rd.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Owner {
    Kernel(Node),
    Driver { module: ModuleId, driver: DriverId },
}

/// What a Kernel-served rd is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Node {
    /// WRITE in its flags lets any Module register below.
    Container,
    /// `/module/<name>`; only that Module registers below.
    Home(ModuleId),
    /// `/sys/decls/<hash>`; reads give the canonical bytes.
    Decl(Hash),
    Log,
    /// `/sys/loglevel`; the level the log keeps up to, by name.
    Level,
    /// `/sys/limit/<name>`; one field of `Limits` in decimal ASCII.
    Limit(Param),
    /// `/sys/info/<name>`; one fact, made at the read.
    Info(Fact),
    /// `/sys/core`; everything the `State` holds, as text.
    Core,
    /// `/session/<id>/client/<cid>/heard`; the clock at the moment that
    /// caller's last message arrived.
    Heard(Caller),
    /// Bytes the Kernel stores and serves, never parses:
    /// `/session/<id>/client/<cid>/capability`.
    Cell,
}

/// One row of the table.
#[derive(Clone, Debug)]
pub struct Entry {
    pub owner: Owner,
    pub parent: Option<Rd>,
    pub name: Option<String>,
    pub flags: Flags,
    pub ctime: u64,
    pub mtime: u64,
    pub atime: u64,
    pub apis: Vec<Hash>,
    pub children: Vec<Rd>,
    /// `lookup("")` was asked once.
    pub listed: bool,
}

impl Entry {
    fn new(
        owner: Owner,
        parent: Option<Rd>,
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
}

pub struct Table {
    rds: BTreeMap<Rd, Entry>,
    next: u64,
}

impl Table {
    /// The fixed tree.
    pub fn new(now: u64) -> Self {
        let root = Owner::Kernel(Node::Container);
        let root = Entry::new(root, None, None, CONTAINER, now);
        let mut table = Self { rds: BTreeMap::from([(ROOT, root)]), next: 1 };
        for (parent, name, node, flags) in FIXED {
            table.insert(parent, Some(name), Owner::Kernel(node), flags, now);
        }
        // The cells of `/sys/info`, so one list says which facts there
        // are, what each is named and the order the Container holds
        // them in.
        for fact in Fact::ALL {
            let owner = Owner::Kernel(Node::Info(fact));
            table.insert(INFO, Some(fact.name()), owner, R, now);
        }
        table.next = FIRST;
        table
    }

    /// A new rd under `parent`; the caller has checked parent and name.
    pub fn insert(
        &mut self,
        parent: Rd,
        name: Option<&str>,
        owner: Owner,
        flags: Flags,
        now: u64,
    ) -> Rd {
        let rd = Rd(self.next);
        self.next += 1;
        self.rds.insert(rd, Entry::new(owner, Some(parent), name, flags, now));
        if let Some(parent) = self.rds.get_mut(&parent) {
            parent.children.push(rd);
        }
        rd
    }

    pub fn get(&self, rd: Rd) -> Result<&Entry, Error> {
        self.rds.get(&rd).ok_or(Error::Invalid)
    }

    pub fn get_mut(&mut self, rd: Rd) -> Result<&mut Entry, Error> {
        self.rds.get_mut(&rd).ok_or(Error::Invalid)
    }

    /// Every rd with its entry.
    pub fn all(&self) -> impl Iterator<Item = (Rd, &Entry)> {
        self.rds.iter().map(|(rd, entry)| (*rd, entry))
    }

    pub fn exists(&self, rd: Rd) -> bool {
        self.rds.contains_key(&rd)
    }

    /// The child of `parent` named `name`, from the cache.
    pub fn child(&self, parent: Rd, name: &str) -> Option<Rd> {
        let named = |rd: &Rd| self.rds[rd].name.as_deref() == Some(name);
        self.rds.get(&parent)?.children.iter().copied().find(named)
    }

    /// `Invalid` when the name is malformed or taken under `parent`.
    pub fn check_name(
        &self,
        parent: Rd,
        name: Option<&str>,
    ) -> Result<(), Error> {
        let Some(name) = name else { return Ok(()) };
        let malformed = name.is_empty() || name.contains(['/', '\0']);
        if malformed || self.child(parent, name).is_some() {
            return Err(Error::Invalid);
        }
        Ok(())
    }

    /// `rd` and everything below it, deepest first, `rd` last.
    pub fn subtree(&self, rd: Rd) -> Vec<Rd> {
        let mut order = alloc::vec![rd];
        let mut at = 0;
        while let Some(next) = order.get(at).copied() {
            order.extend_from_slice(&self.rds[&next].children);
            at += 1;
        }
        order.reverse();
        order
    }

    /// Remove one rd and detach it from its parent.
    pub fn remove(&mut self, rd: Rd) -> Option<Entry> {
        let entry = self.rds.remove(&rd)?;
        if let Some(parent) = entry.parent.and_then(|p| self.rds.get_mut(&p)) {
            parent.children.retain(|c| *c != rd);
        }
        Some(entry)
    }

    pub fn info(&self, rd: Rd) -> Result<Info, Error> {
        let entry = self.get(rd)?;
        Ok(Info {
            name: entry.name.clone(),
            flags: entry.flags,
            ctime: entry.ctime,
            mtime: entry.mtime,
            atime: entry.atime,
            apis: entry.apis.clone(),
        })
    }
}
