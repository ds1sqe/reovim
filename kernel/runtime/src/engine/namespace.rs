pub(super) mod acquire;
pub(super) mod create;
pub(super) mod intern;
pub(super) mod link;
pub(super) mod listing;
mod materialize;
pub(super) mod mutation;
mod traversal;

// Read-only Kernel publication directories with recorded parent identities.

use super::{Entry, Runtime, access::Scope, codec};
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use reovim_kernel_interface::{
    Error, Outcome, ResourceId, Value,
    provider::{ResourceOps, ResourceProvider},
    resource::AccessId,
};

impl Runtime {
    /// Installs a Kernel-owned publication directory, e.g. `/loaded`.
    /// Entries returns one complete copied page or Pressure; cursor is zero.
    /// No child creation or backend I/O is implied.
    /// # Errors
    /// Invalid/conflicting paths and allocation pressure reject installation.
    pub fn register_namespace(
        &mut self,
        path: &[u8],
    ) -> Result<ResourceId, Error> {
        let provider = ResourceProvider::new(
            (),
            ResourceOps {
                open: None,
                namespace: None,
                io: None,
                stop: |()| Ok(()),
            },
            Box::new([]),
            Rc::new(()),
        )
        .map_err(|(error, ())| error)?;
        self.register_entry(path, provider, None, true)
            .map_err(|(error, _)| error)
    }
}

pub(super) fn parent(entries: &[Rc<Entry>], path: &[u8]) -> Option<ResourceId> {
    let slash = path.iter().rposition(|byte| *byte == b'/')?;
    let parent = if slash == 0 { &b"/"[..] } else { &path[..slash] };
    entries
        .iter()
        .find(|entry| {
            entry.path.as_ref() == parent
                && entry.directory
                && !entry.closed.get()
                && entry.path.as_ref() != path
        })
        .map(|entry| entry.identity)
}

pub(super) fn flags(scope: &Scope, entry: &Entry) -> Result<u32, Error> {
    if !entry.directory {
        return Ok(0);
    }
    let registry = scope.registry.upgrade().ok_or(Error::CLOSED)?;
    let registry = registry.borrow();
    let populated =
        registry.entries.iter().any(|child| visible(child, entry.identity));
    Ok(if populated { 1 } else { 3 })
}

pub(super) fn entries(
    scope: &Scope,
    subject: AccessId,
    input: &[u8],
) -> Result<Outcome, Error> {
    let handle = scope.live(subject)?;
    if !handle.entry.directory {
        return Err(Error::UNSUPPORTED);
    }
    if handle.opened.borrow().held & 1 == 0 {
        return Err(Error::DENIED);
    }
    if input.len() != 16 {
        return Err(Error::INVALID);
    }
    let cursor = codec::u64_value(&input[..8])?;
    if cursor != 0 {
        return Err(Error::INVALID);
    }
    let maximum = usize::try_from(codec::u64_value(&input[8..])?)
        .map_err(|_| Error::PRESSURE)?;
    let budget = maximum.min(scope.limits.values.bytes);
    if budget < 12 || scope.limits.values.elements < 3 {
        return Err(Error::PRESSURE);
    }
    let registry = scope.registry.upgrade().ok_or(Error::CLOSED)?;
    let registry = registry.borrow();
    let children = registry
        .entries
        .iter()
        .filter(|entry| visible(entry, handle.entry.identity));
    let mut size = 12_usize;
    let mut count = 0_u32;
    for entry in children.clone() {
        let name = entry.path.rsplit(|byte| *byte == b'/').next().unwrap();
        size = size.checked_add(28 + name.len()).ok_or(Error::PRESSURE)?;
        count = count.checked_add(1).ok_or(Error::PRESSURE)?;
    }
    if size > budget || 3 + 3 * count as usize > scope.limits.values.elements {
        return Err(Error::PRESSURE);
    }
    let mut output = Vec::new();
    output.try_reserve_exact(size).map_err(|_| Error::PRESSURE)?;
    output.extend_from_slice(&count.to_le_bytes());
    for entry in children {
        let name = entry.path.rsplit(|byte| *byte == b'/').next().unwrap();
        output.extend_from_slice(
            &u32::try_from(name.len())
                .map_err(|_| Error::PRESSURE)?
                .to_le_bytes(),
        );
        output.extend_from_slice(name);
        output.extend_from_slice(&entry.identity.epoch);
        output.extend_from_slice(&entry.identity.serial.to_le_bytes());
    }
    output.extend_from_slice(&0_u64.to_le_bytes());
    Ok(Outcome::Ok(Value {
        data: output.into_boxed_slice(),
        references: Box::new([]),
    }))
}

fn visible(entry: &Entry, parent: ResourceId) -> bool {
    entry.parent == Some(parent) && !entry.closed.get()
}
