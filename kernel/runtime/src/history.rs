use alloc::vec::Vec;
use reovim_kernel_interface::{Error, buffer::State};

struct Node {
    state: State,
    parent: Option<usize>,
    preferred: Option<usize>,
}

/// One authority. The caller drops rejected states outside its commit guard.
pub struct History {
    nodes: Vec<Node>,
    current: usize,
    revision: u64,
    limit: usize,
    bytes: u64,
    byte_limit: u64,
}

impl History {
    /// # Errors
    /// Returns the unchanged state on invalid bounds or allocation pressure.
    pub fn new(
        initial: State,
        limit: usize,
        byte_limit: u64,
    ) -> Result<Self, (Error, State)> {
        let mut nodes = Vec::new();
        if limit == 0
            || initial.cost > byte_limit
            || nodes.try_reserve_exact(limit).is_err()
        {
            return Err((Error::PRESSURE, initial));
        }
        let bytes = initial.cost;
        nodes.push(Node { state: initial, parent: None, preferred: None });
        Ok(Self { nodes, current: 0, revision: 0, limit, bytes, byte_limit })
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.nodes[self.current].state.clone()
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn node(&self) -> usize {
        self.current
    }

    #[must_use]
    pub fn children(&self) -> Vec<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                (node.parent == Some(self.current)).then_some(index)
            })
            .collect()
    }

    /// # Errors
    /// Rejects stale state or exhausted bounds without changing either owner.
    pub fn install(
        &mut self,
        expected: u64,
        candidate: State,
    ) -> Result<(), (Error, State)> {
        let error = if expected == self.revision {
            Error::PRESSURE
        } else {
            Error::STALE
        };
        let Some((revision, bytes)) = self.reservation(expected, &candidate)
        else {
            return Err((error, candidate));
        };
        let next = self.nodes.len();
        // Commit cut: reserved storage, scalar changes and ownership moves.
        self.nodes[self.current].preferred = Some(next);
        self.nodes.push(Node {
            state: candidate,
            parent: Some(self.current),
            preferred: None,
        });
        self.current = next;
        self.revision = revision;
        self.bytes = bytes;
        Ok(())
    }

    /// # Errors
    /// Rejects stale currentness or absence of the selected parent/child.
    pub fn restore(
        &mut self,
        expected: u64,
        undo: bool,
        child: Option<usize>,
    ) -> Result<(), Error> {
        if expected != self.revision {
            return Err(Error::STALE);
        }
        let target = self.restore_target(undo, child)?;
        let revision = self.revision.checked_add(1).ok_or(Error::PRESSURE)?;
        // Every node already owns immutable state and its model's code.
        self.nodes[if undo { target } else { self.current }].preferred =
            Some(if undo { self.current } else { target });
        self.current = target;
        self.revision = revision;
        Ok(())
    }

    fn reservation(
        &self,
        expected: u64,
        candidate: &State,
    ) -> Option<(u64, u64)> {
        if expected != self.revision || self.nodes.len() == self.limit {
            return None;
        }
        let revision = self.revision.checked_add(1)?;
        let bytes = self.bytes.checked_add(candidate.cost)?;
        (bytes <= self.byte_limit).then_some((revision, bytes))
    }

    fn restore_target(
        &self,
        undo: bool,
        child: Option<usize>,
    ) -> Result<usize, Error> {
        if undo {
            return self.nodes[self.current].parent.ok_or(Error::UNAVAILABLE);
        }
        let target = child
            .or(self.nodes[self.current].preferred)
            .ok_or(Error::UNAVAILABLE)?;
        if self.nodes.get(target).is_none_or(|n| n.parent != Some(self.current))
        {
            return Err(Error::INVALID);
        }
        Ok(target)
    }
}
