use super::super::{Entry, Registry};
use super::materialize::Work;
use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::cell::{Cell, RefCell};
use reovim_kernel_interface::{Error, ResourceId};

pub(in crate::engine) struct Key {
    pub(super) owner: Weak<Entry>,
    pub(super) bytes: Box<[u8]>,
    pub(super) identity: ResourceId,
    pub(super) reserved: Cell<bool>,
    pub(super) creating: Cell<bool>,
    pub(super) ready: RefCell<Weak<Entry>>,
    pub(super) work: RefCell<Weak<RefCell<Work>>>,
}

pub(super) fn selected(
    registry: &Registry,
    parent: &Rc<Entry>,
    target: &Rc<Entry>,
) -> Result<Option<Box<[u8]>>, Error> {
    let root = parent.namespace_owner.as_ref().and_then(Weak::upgrade);
    let owner = root.as_ref().unwrap_or(parent);
    if Rc::ptr_eq(owner, target) {
        return Ok(None);
    }
    registry
        .keys
        .iter()
        .find(|key| {
            key.identity == target.identity
                && key.owner.ptr_eq(&Rc::downgrade(owner))
                && key
                    .ready
                    .borrow()
                    .upgrade()
                    .is_some_and(|ready| Rc::ptr_eq(&ready, target))
        })
        .map(|key| Some(key.bytes.clone()))
        .ok_or(Error::UNSUPPORTED)
}

pub(super) fn page(
    registry: &mut Registry,
    owner: &Rc<Entry>,
    keys: &[Box<[u8]>],
) -> Result<Vec<ResourceId>, Error> {
    reclaim(registry);
    let root = owner.namespace_owner.as_ref().and_then(Weak::upgrade);
    let owner = root.as_ref().unwrap_or(owner);
    if owner.closed.get() {
        return Err(Error::CLOSED);
    }
    let mut added: Vec<Box<[u8]>> = super::super::reserve(keys.len())?;
    let mut result = super::super::reserve(keys.len())?;
    for key in keys {
        if find(registry, owner, key).is_none()
            && !added.iter().any(|prior| prior == key)
        {
            let mut copy = Vec::new();
            copy.try_reserve_exact(key.len()).map_err(|_| Error::PRESSURE)?;
            copy.extend_from_slice(key);
            added.push(copy.into_boxed_slice());
        }
    }
    // All key/output allocations precede identity reservation. The registry's
    // fixed-capacity key vector shares the ordinary Resource capacity budget.
    let first = registry.reserve_batch(added.len())?;
    for (index, bytes) in added.into_iter().enumerate() {
        registry.keys.push(Rc::new(Key {
            reserved: Cell::new(true),
            creating: Cell::new(false),
            ready: RefCell::new(Weak::new()),
            work: RefCell::new(Weak::new()),
            owner: Rc::downgrade(owner),
            bytes,
            identity: ResourceId {
                epoch: first.epoch,
                serial: first.serial + index as u64,
            },
        }));
    }
    for key in keys {
        result.push(find(registry, owner, key).unwrap());
    }
    Ok(result)
}
fn find(
    registry: &Registry,
    owner: &Rc<Entry>,
    key: &[u8],
) -> Option<ResourceId> {
    let owner = Rc::downgrade(owner);
    registry
        .keys
        .iter()
        .find(|record| {
            record.owner.ptr_eq(&owner) && record.bytes.as_ref() == key
        })
        .map(|record| record.identity)
}

pub(in crate::engine) fn reclaim(registry: &mut Registry) -> bool {
    let before = registry.keys.len();
    let mut credits = 0;
    registry.keys.retain(|key| {
        let keep = live(key);
        if !keep && key.reserved.replace(false) {
            credits += 1;
        }
        keep
    });
    registry.release_batch(credits);
    before != registry.keys.len()
}
fn live(key: &Key) -> bool {
    if key.creating.get() || key.work.borrow().upgrade().is_some() {
        return true;
    }
    key.owner.upgrade().is_some_and(|owner| !owner.closed.get())
        && (key.reserved.get() || key.ready.borrow().upgrade().is_some())
}
pub(in crate::engine) fn clear(registry: &mut Registry) {
    let count = registry.keys.iter().filter(|key| key.reserved.get()).count();
    registry.keys.clear();
    registry.release_batch(count);
}

pub(super) fn claim(
    registry: &mut Registry,
    owner: &Rc<Entry>,
    identity: ResourceId,
    bytes: &[u8],
) -> Result<Rc<Key>, Error> {
    reclaim(registry);
    if find(registry, owner, bytes).is_some() {
        return Err(Error::CONFLICT);
    }
    let mut copy = super::super::reserve(bytes.len())?;
    copy.extend_from_slice(bytes);
    let key = Rc::new(Key {
        owner: Rc::downgrade(owner),
        bytes: copy.into_boxed_slice(),
        identity,
        reserved: Cell::new(true),
        creating: Cell::new(true),
        ready: RefCell::new(Weak::new()),
        work: RefCell::new(Weak::new()),
    });
    registry.keys.push(key.clone());
    Ok(key)
}
