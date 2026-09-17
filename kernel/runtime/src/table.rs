//! The rd table: every Resource the Kernel knows, with its cached shape.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_kernel_interface::{
    DriverId, Error, Flags, Hash, Info, ModuleId, Rd,
};

/// Fixed ids, allocated in this order.
///
/// `/` 0, `/sys` 1, `/sys/decls` 2, `/sys/log` 3, `/module` 4,
/// `/session` 5, `/var` 6, `/sys/loglevel` 7. Ids below `FIRST` are
/// reserved for the Kernel; everything registered later starts there.
pub const ROOT: Rd = Rd(0);
pub const SYS: Rd = Rd(1);
pub const DECLS: Rd = Rd(2);
pub const LOG: Rd = Rd(3);
pub const MODULES: Rd = Rd(4);
pub const SESSIONS: Rd = Rd(5);
pub const LOGLEVEL: Rd = Rd(7);
const FIRST: u64 = 100;

/// A Kernel Container.
pub const CONTAINER: Flags = Flags::READ.union(Flags::CONTAINER);
/// A Kernel Container any Module may register under.
pub const OPEN: Flags = CONTAINER.union(Flags::WRITE);

/// A Kernel cell a caller may read and write.
const RW: Flags = Flags::READ.union(Flags::WRITE);

const FIXED: [(Rd, &str, Node, Flags); 7] = [
    (ROOT, "sys", Node::Container, CONTAINER),
    (SYS, "decls", Node::Container, CONTAINER),
    (SYS, "log", Node::Log, RW),
    (ROOT, "module", Node::Container, CONTAINER),
    (ROOT, "session", Node::Container, CONTAINER),
    (ROOT, "var", Node::Container, OPEN),
    (SYS, "loglevel", Node::Level, RW),
];

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
