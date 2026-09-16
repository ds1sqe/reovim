use alloc::{
    rc::{Rc, Weak},
    vec::Vec,
};
use core::cell::RefCell;
use reovim_kernel_interface::{Error, ResourceId, declaration::Declaration};

use super::{Entry, reserve};

pub(super) struct Registry {
    epoch: [u8; 16],
    next: u64,
    limit: usize,
    reserved: usize,
    pub entries: Vec<Rc<Entry>>,
    pub keys: Vec<Rc<super::namespace::intern::Key>>,
    pub staged: Vec<Weak<Entry>>,
    pub opening: super::opening::Pool,
    pub closed: bool,
}

impl Registry {
    pub fn new(
        epoch: [u8; 16],
        limit: usize,
        opened: usize,
    ) -> Result<Self, Error> {
        Ok(Self {
            epoch,
            next: 1,
            limit,
            reserved: 0,
            entries: reserve(limit)?,
            keys: reserve(limit)?,
            staged: reserve(limit)?,
            opening: super::opening::Pool::new(opened)?,
            closed: false,
        })
    }

    pub fn reserve(&mut self) -> Result<ResourceId, Error> {
        self.reserve_batch(1)
    }

    pub fn reserve_batch(&mut self, count: usize) -> Result<ResourceId, Error> {
        if self.closed {
            return Err(Error::CLOSED);
        }
        if count > self.limit - self.entries.len() - self.reserved {
            return Err(Error::PRESSURE);
        }
        let end = self
            .next
            .checked_add(u64::try_from(count).map_err(|_| Error::PRESSURE)?)
            .ok_or(Error::PRESSURE)?;
        let identity = ResourceId { epoch: self.epoch, serial: self.next };
        self.next = end;
        self.reserved += count;
        Ok(identity)
    }

    pub const fn release_batch(&mut self, count: usize) {
        self.reserved -= count;
    }

    pub const fn release(&mut self) {
        self.reserved -= 1;
    }

    pub fn install(&mut self, entry: Rc<Entry>) {
        self.release();
        self.entries.push(entry);
    }

    pub fn validate(
        &self,
        path: &[u8],
        interfaces: &[Declaration],
    ) -> Result<(), Error> {
        if self.closed {
            return Err(Error::CLOSED);
        }
        for entry in self.staged.iter().filter_map(Weak::upgrade) {
            compatible(&entry, path, interfaces)?;
        }
        for entry in &self.entries {
            compatible(entry, path, interfaces)?;
        }
        Ok(())
    }
}

pub(super) fn compatible(
    entry: &Entry,
    path: &[u8],
    interfaces: &[Declaration],
) -> Result<(), Error> {
    if !path.is_empty() && entry.path.as_ref() == path {
        return Err(Error::CONFLICT);
    }
    for declaration in interfaces {
        let previous = entry
            .implementation
            .interfaces
            .iter()
            .find(|other| other.api() == declaration.api());
        if previous.is_some_and(|other| other != declaration) {
            return Err(Error::CONFLICT);
        }
    }
    Ok(())
}

pub(super) fn reclaim(registry: &Rc<RefCell<Registry>>) -> Result<bool, Error> {
    let mut progress = super::opening::reclaim(registry)?;
    progress |= super::namespace::intern::reclaim(&mut registry.borrow_mut());
    let count = registry.borrow().entries.len();
    for index in (0..count).rev() {
        progress |= reclaim_one(registry, index)?;
    }
    Ok(progress)
}

fn reclaim_one(
    registry: &Rc<RefCell<Registry>>,
    index: usize,
) -> Result<bool, Error> {
    let entries = registry.borrow();
    let entry = &entries.entries[index];
    if !entry.path.is_empty() || Rc::strong_count(entry) != 1 {
        return Ok(false);
    }
    let entry = entry.clone();
    drop(entries);
    match entry.implementation.provider.borrow_mut().stop() {
        Ok(()) => {}
        Err(Error::BUSY) => return Ok(false),
        Err(error) => return Err(error),
    }
    registry.borrow_mut().entries.remove(index);
    // Provider destruction runs after removing the registry owner and guard.
    drop(entry);
    Ok(true)
}

pub(super) fn close(registry: &Rc<RefCell<Registry>>) -> Result<(), Error> {
    loop {
        let entry = registry.borrow().entries.last().cloned();
        let Some(entry) = entry else {
            super::namespace::intern::clear(&mut registry.borrow_mut());
            return Ok(());
        };
        entry.implementation.provider.borrow_mut().stop()?;
        registry.borrow_mut().entries.pop();
    }
}
