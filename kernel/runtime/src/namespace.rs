use alloc::vec::Vec;
use reovim_kernel_interface::interaction::root::RootFailure;
use reovim_kernel_interface::namespace::ResourceEntry;
use reovim_kernel_interface::resource::ResourceId;

use crate::RuntimeError;

// Names belong to this single-source reference profile, not the common SDK.
struct Entry {
    parent: usize,
    name: &'static str,
}

const ENTRIES: [Entry; 4] = [
    Entry { parent: 0, name: "" },
    Entry { parent: 0, name: "source" },
    Entry { parent: 0, name: "buffer" },
    Entry { parent: 2, name: "history" },
];

const MAX_PATH_BYTES: usize = 256;

pub struct Namespace {
    ids: [ResourceId; 4],
}

impl Namespace {
    pub(crate) fn new(client: usize) -> Result<Self, RuntimeError> {
        let base = u64::try_from(client)
            .ok()
            .and_then(|n| n.checked_mul(4))
            .filter(|n| n.checked_add(4).is_some())
            .ok_or(RuntimeError::Capacity)?;
        Ok(Self { ids: [1, 2, 3, 4].map(|offset| ResourceId(base + offset)) })
    }

    pub(crate) const fn root(&self) -> ResourceId {
        self.ids[0]
    }

    pub(crate) const fn source(&self) -> ResourceId {
        self.ids[1]
    }

    pub(crate) const fn buffer(&self) -> ResourceId {
        self.ids[2]
    }

    pub(crate) fn resolve_at(
        &self,
        parent: ResourceId,
        path: &str,
        published: bool,
    ) -> Result<ResourceId, RootFailure> {
        let mut index = self.index(parent, published)?;
        if path.len() > MAX_PATH_BYTES {
            return Err(RootFailure::Pressure);
        }
        if path.is_empty() {
            return Ok(parent);
        }
        if path.split('/').any(|part| matches!(part, "" | "." | "..")) {
            return Err(RootFailure::InvalidRequest);
        }
        for name in path.split('/') {
            index = self.child(index, name, published)?;
        }
        Ok(self.ids[index])
    }

    pub(crate) fn children(
        &self,
        parent: ResourceId,
        published: bool,
    ) -> Result<Vec<ResourceEntry>, RootFailure> {
        let parent = self.index(parent, published)?;
        if parent == 1 || parent == 3 {
            return Err(RootFailure::Unsupported);
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(ENTRIES.len())
            .map_err(|_| RootFailure::Pressure)?;
        let children = ENTRIES.iter().enumerate().filter(|(index, entry)| {
            *index != parent
                && entry.parent == parent
                && visible(*index, published)
        });
        for (index, entry) in children {
            entries.push(ResourceEntry {
                name: entry.name.into(),
                resource: self.ids[index],
            });
        }
        entries.sort_unstable_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    fn index(
        &self,
        resource: ResourceId,
        published: bool,
    ) -> Result<usize, RootFailure> {
        let index = self
            .ids
            .iter()
            .position(|id| *id == resource)
            .ok_or(RootFailure::Denied)?;
        if !visible(index, published) {
            return Err(RootFailure::Unavailable);
        }
        Ok(index)
    }

    fn child(
        &self,
        parent: usize,
        name: &str,
        published: bool,
    ) -> Result<usize, RootFailure> {
        if parent == 1 || parent == 3 {
            return Err(RootFailure::Unsupported);
        }
        let index = ENTRIES
            .iter()
            .position(|entry| entry.parent == parent && entry.name == name)
            .ok_or(RootFailure::Unavailable)?;
        self.index(self.ids[index], published)
    }
}

const fn visible(index: usize, published: bool) -> bool {
    index < 2 || published
}
