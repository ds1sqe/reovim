use alloc::{boxed::Box, format, rc::Rc, vec::Vec};
use core::cell::RefCell;
use reovim_kernel_interface::buffer::ModelProvider;
use reovim_kernel_interface::provider::{Permissions, ResourceProvider};
use reovim_kernel_interface::{Error, ResourceId};

use crate::{Editing, Shared};

#[derive(Clone)]
pub enum Kind {
    Directory,
    Unloading,
    Provider(Rc<RefCell<ResourceProvider>>),
    Model(Rc<RefCell<ModelProvider>>),
    Buffer(Rc<RefCell<Editing>>),
    History(Rc<RefCell<Editing>>),
    ClosedBuffer { owner: u64, revision: u64 },
    Work { scope: u64, request: u64 },
}

impl Kind {
    pub fn removable(&self) -> bool {
        match self {
            Self::Provider(provider) => Rc::strong_count(provider) == 1,
            Self::Model(model) => Rc::strong_count(model) == 1,
            _ => false,
        }
    }
}

pub struct Entry {
    pub key: u64,
    pub parent: u64,
    pub name: Box<str>,
    pub kind: Kind,
}

impl Entry {
    pub fn directory(key: u64, parent: u64, name: &str) -> Self {
        Self { key, parent, name: name.into(), kind: Kind::Directory }
    }
}

pub struct Scope {
    pub id: u64,
    pub generation: u64,
    pub grants: Vec<(u64, Permissions)>,
    pub names: Vec<(ResourceId, u64)>,
}

impl Scope {
    pub fn new(id: u64, grants: &[(ResourceId, Permissions)]) -> Self {
        Self {
            id,
            generation: 0,
            names: Vec::new(),
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

    pub fn export(
        &mut self,
        shared: &mut Shared,
        key: u64,
    ) -> Result<ResourceId, Error> {
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
        self.names.push((id, key));
        Ok(id)
    }

    pub fn permissions(
        &self,
        shared: &Shared,
        key: u64,
    ) -> Option<Permissions> {
        self.grants
            .iter()
            .filter(|(root, _)| shared.descends(key, *root))
            .map(|(_, permissions)| *permissions)
            .reduce(|a, b| Permissions {
                read: a.read || b.read,
                write: a.write || b.write,
            })
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
        match &shared.entries[index].kind {
            Kind::Work { scope, .. } => *scope == self.id,
            Kind::Directory => {
                key == 1
                    || key == 2
                    || self
                        .grants
                        .iter()
                        .any(|(root, _)| shared.descends(*root, key))
                    || self.permissions(shared, key).is_some()
            }
            _ => self.permissions(shared, key).is_some(),
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
                    .find(|entry| entry.parent == key && &*entry.name == name)
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
        if !matches!(self.entries[self.entry_index(key)?].kind, Kind::Directory)
        {
            return Err(Error::INVALID);
        }
        Ok(Some(key))
    }

    pub fn insert(&mut self, path: &str, kind: Kind) -> u64 {
        let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
        let parent = parent
            .split('/')
            .filter(|name| !name.is_empty())
            .fold(1, |parent, name| self.directory(parent, name));
        self.insert_entry(parent, name, kind)
    }

    fn directory(&mut self, parent: u64, name: &str) -> u64 {
        self.path(parent, name).unwrap_or_else(|_| {
            self.insert_entry(parent, name, Kind::Directory)
        })
    }

    fn insert_entry(&mut self, parent: u64, name: &str, kind: Kind) -> u64 {
        let key = self.ids.take().expect("registration reserved identities");
        self.entries.push(Entry { key, parent, name: name.into(), kind });
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
