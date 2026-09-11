use alloc::{boxed::Box, format, rc::Rc, vec::Vec};
use core::cell::{Cell, RefCell};
use reovim_kernel_interface::buffer::ModelProvider;
use reovim_kernel_interface::driver::Lifecycle;
use reovim_kernel_interface::provider::{Permissions, ResourceProvider};
use reovim_kernel_interface::{Error, ResourceId};

use crate::{Editing, Shared};

#[derive(Clone)]
pub enum Kind {
    Session(Rc<RefCell<crate::sessions::Session>>),
    Snapshot(Rc<crate::sessions::Snapshot>),
    ClosedSession { owner: u64 },
    Catalog(Rc<RefCell<crate::catalog::Catalog>>),
    Module(Rc<Cell<Lifecycle>>),
    Directory,
    Unloading,
    Provider(Rc<RefCell<ResourceProvider>>),
    Model(Rc<RefCell<ModelProvider>>),
    Buffer(Rc<RefCell<Editing>>),
    History(Rc<RefCell<Editing>>),
    ClosedBuffer { owner: u64, revision: u64 },
    Work { scope: u64, request: u64, retained: bool },
}

impl Kind {
    pub const fn accepts_children(&self) -> bool {
        matches!(
            self,
            Self::Directory
                | Self::Session(_)
                | Self::Provider(_)
                | Self::Model(_)
        )
    }

    pub fn removable(&self) -> bool {
        match self {
            Self::Provider(provider) => Rc::strong_count(provider) == 1,
            Self::Model(model) => Rc::strong_count(model) == 1,
            _ => false,
        }
    }
}

pub type Group = Rc<RefCell<Vec<ResourceId>>>;

pub struct Ownership {
    pub client: u64,
    pub closed: bool,
}

pub struct Entry {
    pub key: u64,
    pub parent: u64,
    pub name: Box<str>,
    pub kind: Kind,
    pub activation: Option<Rc<Cell<Lifecycle>>>,
    pub receiver: Option<Rc<RefCell<Scope>>>,
    pub group: Option<Group>,
    pub ownership: Option<Ownership>,
}

impl Entry {
    fn matches(&self, parent: u64, name: &str) -> bool {
        self.parent == parent && &*self.name == name && !self.closed()
    }

    pub fn closed(&self) -> bool {
        self.ownership.as_ref().is_some_and(|owner| owner.closed)
    }

    pub fn directory(key: u64, parent: u64, name: &str) -> Self {
        Self {
            key,
            parent,
            name: name.into(),
            kind: Kind::Directory,
            activation: None,
            receiver: None,
            group: None,
            ownership: None,
        }
    }
}

type Names = Vec<(Box<str>, u64)>;

pub struct Scope {
    pub id: u64,
    pub owner: u64,
    pub root: u64,
    pub aliases: Rc<RefCell<Names>>,
    pub generation: u64,
    pub closed: bool,
    pub grants: Vec<(u64, Permissions)>,
    pub names: Vec<(ResourceId, u64)>,
    pub loans: Vec<(u64, u64, Permissions)>,
}

impl Scope {
    pub fn new(id: u64, grants: &[(ResourceId, Permissions)]) -> Self {
        Self {
            id,
            owner: id,
            root: 1,
            aliases: Rc::default(),
            generation: 0,
            closed: false,
            names: Vec::new(),
            loans: Vec::new(),
            grants: grants.iter().map(|(id, rights)| (id.0, *rights)).collect(),
        }
    }

    pub fn key(&self, id: ResourceId) -> Result<u64, Error> {
        self.names
            .iter()
            .find(|(local, _)| *local == id)
            .map(|(_, key)| *key)
            .ok_or(Error::DENIED)
    }

    pub fn child(
        &self,
        shared: &Shared,
        parent: u64,
        name: &str,
    ) -> Result<u64, Error> {
        let aliases = self.aliases.borrow();
        let alias = aliases.iter().find(|(alias, _)| &**alias == name);
        if let Some((_, key)) = alias.filter(|_| parent == self.root) {
            shared.entry_index(*key)?;
            return Ok(*key);
        }
        shared.path(parent, name)
    }

    pub fn children(
        &self,
        shared: &Shared,
        parent: u64,
    ) -> Vec<(Box<str>, u64)> {
        let mut entries: Vec<_> = shared
            .entries
            .iter()
            .filter(|entry| entry.parent == parent && entry.key != parent)
            .map(|entry| (entry.name.clone(), entry.key))
            .collect();
        let aliases = self.aliases.borrow();
        entries.retain(|(name, key)| {
            self.visible(shared, *key)
                && (parent != self.root
                    || !aliases.iter().any(|(alias, _)| alias == name))
        });
        entries.extend(
            aliases
                .iter()
                .filter(|(_, key)| {
                    parent == self.root && self.visible(shared, *key)
                })
                .cloned(),
        );
        entries
    }

    pub fn export(
        &mut self,
        shared: &mut Shared,
        key: u64,
    ) -> Result<ResourceId, Error> {
        let index = shared.entry_index(key)?;
        if let Some((id, _)) =
            self.names.iter().find(|(_, owned)| *owned == key)
        {
            return Ok(*id);
        }
        if self.names.len() == shared.limits.resources {
            return Err(Error::PRESSURE);
        }
        self.names.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        let id = ResourceId(shared.id()?);
        if let Kind::Work { retained, .. } = &mut shared.entries[index].kind {
            *retained = true;
        }
        self.names.push((id, key));
        Ok(id)
    }

    pub fn permissions(
        &self,
        shared: &Shared,
        key: u64,
    ) -> Option<Permissions> {
        let work = shared.entry_index(key).ok().and_then(|index| match shared
            .entries[index]
            .kind
        {
            Kind::Work { scope, .. } => Some(scope),
            _ => None,
        });
        if work == Some(self.id) {
            return Some(Permissions::ALL);
        }
        self.grants
            .iter()
            .copied()
            .chain(self.loans.iter().map(|(_, key, rights)| (*key, *rights)))
            .filter(|(root, _)| {
                key == *root || (work.is_none() && shared.descends(key, *root))
            })
            .map(|(_, permissions)| permissions)
            .reduce(Permissions::union)
    }

    pub fn require_visible(
        &self,
        shared: &Shared,
        key: u64,
    ) -> Result<(), Error> {
        if self.visible(shared, key) { Ok(()) } else { Err(Error::DENIED) }
    }

    pub fn visible(&self, shared: &Shared, key: u64) -> bool {
        let Ok(index) = shared.entry_index(key) else {
            return false;
        };
        if shared.entries[index].closed() {
            return false;
        }
        match &shared.entries[index].kind {
            Kind::Work { .. } => self.permissions(shared, key).is_some(),
            _ => {
                key == self.root
                    || key == 2
                    || self.permissions(shared, key).is_some()
                    || self
                        .grants
                        .iter()
                        .any(|(root, _)| shared.descends(*root, key))
                    || self
                        .loans
                        .iter()
                        .any(|(_, root, _)| shared.descends(*root, key))
            }
        }
    }
}

impl Shared {
    pub fn entry_index(&self, key: u64) -> Result<usize, Error> {
        self.entries
            .iter()
            .position(|entry| entry.key == key)
            .ok_or(Error::UNAVAILABLE)
    }

    pub fn descends(&self, key: u64, root: u64) -> bool {
        core::iter::successors(Some(key), |key| self.parent(*key))
            .take(self.entries.len())
            .any(|key| key == root)
    }

    fn parent(&self, key: u64) -> Option<u64> {
        let entry = &self.entries[self.entry_index(key).ok()?];
        (entry.parent != key).then_some(entry.parent)
    }

    pub fn path(&self, parent: u64, path: &str) -> Result<u64, Error> {
        validate(path, self.limits.path_bytes)?;
        path.split('/').filter(|part| !part.is_empty()).try_fold(
            parent,
            |key, name| {
                self.entries
                    .iter()
                    .find(|entry| entry.matches(key, name))
                    .map(|entry| entry.key)
                    .ok_or(Error::UNAVAILABLE)
            },
        )
    }

    pub fn check_registration(&self, path: &str) -> Result<(), Error> {
        if self.closing {
            return Err(Error::CLOSED);
        }
        validate(path, self.limits.path_bytes)?;
        if path.is_empty() || self.path(1, path).is_ok() {
            return Err(Error::INVALID);
        }
        let needed = path.split('/').count();
        if self.entries.len().saturating_add(needed) > self.limits.resources {
            return Err(Error::PRESSURE);
        }
        // Reserve enough identities before taking ownership of the provider.
        if !self.ids.can_take(needed) {
            return Err(Error::PRESSURE);
        }
        self.check_parents(path)
    }

    fn check_parents(&self, path: &str) -> Result<(), Error> {
        let Some((parent, _)) = path.rsplit_once('/') else {
            return Ok(());
        };
        parent
            .split('/')
            .try_fold(Some(1), |key, name| self.existing_directory(key, name))
            .map(|_| ())
    }

    fn existing_directory(
        &self,
        parent: Option<u64>,
        name: &str,
    ) -> Result<Option<u64>, Error> {
        let Some(key) = parent.and_then(|key| self.path(key, name).ok()) else {
            return Ok(None);
        };
        if !self.entries[self.entry_index(key)?].kind.accepts_children() {
            return Err(Error::INVALID);
        }
        Ok(Some(key))
    }

    pub fn insert(&mut self, path: &str, kind: Kind) -> u64 {
        self.insert_at(1, path, kind)
    }

    pub fn insert_at(&mut self, root: u64, path: &str, kind: Kind) -> u64 {
        let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
        let parent = parent
            .split('/')
            .filter(|name| !name.is_empty())
            .fold(root, |parent, name| self.directory(parent, name));
        if let Ok(key) = self.path(parent, name) {
            // A later same-batch contribution replaces only its implicit
            // directory. Keep the identity already used by its children.
            let index = self.entry_index(key).expect("existing entry");
            debug_assert!(matches!(self.entries[index].kind, Kind::Directory));
            self.entries[index].kind = kind;
            key
        } else {
            self.insert_entry(parent, name, kind)
        }
    }

    fn directory(&mut self, parent: u64, name: &str) -> u64 {
        self.path(parent, name).unwrap_or_else(|_| {
            self.insert_entry(parent, name, Kind::Directory)
        })
    }

    fn insert_entry(&mut self, parent: u64, name: &str, kind: Kind) -> u64 {
        let key = self.ids.take().expect("registration reserved identities");
        self.entries.push(Entry {
            key,
            parent,
            name: name.into(),
            kind,
            activation: None,
            receiver: None,
            group: None,
            ownership: None,
        });
        key
    }

    pub fn work_path(&self, scope: u64, request: u64) -> Box<str> {
        format!("requests/{}-{scope}-{request}", self.identity).into()
    }
}

pub fn validate(path: &str, limit: usize) -> Result<(), Error> {
    if path.len() > limit {
        return Err(Error::PRESSURE);
    }
    if !path.is_empty()
        && path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(Error::INVALID);
    }
    Ok(())
}
