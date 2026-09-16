//! A Buffer's versioned state, as 01-core-runtime.md describes it: a
//! tree of stored states, a head and a published pointer. Nothing here
//! knows an rd or the Kernel.

use alloc::string::String;
use alloc::vec::Vec;

/// One stored state: its parent's revision (0 for the first), the
/// method that made it and the whole draft.
struct State {
    parent: u64,
    label: String,
    bytes: Vec<u8>,
}

/// Revisions are numbered from 1 in the order made; 0 is "none".
pub struct History {
    states: Vec<State>,
    head: u64,
    published: u64,
}

impl History {
    /// The first state, labelled `label`, published.
    pub fn new(label: &str, bytes: Vec<u8>) -> Self {
        let first = State { parent: 0, label: String::from(label), bytes };
        Self { states: alloc::vec![first], head: 1, published: 1 }
    }

    fn state(&self, rev: u64) -> Option<&State> {
        let index = usize::try_from(rev.checked_sub(1)?).ok()?;
        self.states.get(index)
    }

    /// The draft: the state at head.
    pub fn draft(&self) -> &[u8] {
        self.state(self.head).map_or(&[], |s| &s.bytes)
    }

    pub const fn head(&self) -> u64 {
        self.head
    }

    /// `bytes` as a new child of head; head moves there.
    pub fn commit(&mut self, label: &str, bytes: Vec<u8>) {
        let label = String::from(label);
        self.states.push(State { parent: self.head, label, bytes });
        self.head = self.states.len() as u64;
    }

    /// Head is what a `save` just stored.
    pub const fn published(&mut self) {
        self.published = self.head;
    }

    /// Head to its parent; false at the first state.
    pub fn undo(&mut self) -> bool {
        match self.state(self.head).map(|s| s.parent) {
            Some(parent) if parent != 0 => {
                self.head = parent;
                true
            }
            _ => false,
        }
    }

    /// Head to its newest child; false without one.
    pub fn redo(&mut self) -> bool {
        let newest = self
            .states
            .iter()
            .rposition(|s| s.parent == self.head)
            .map(|index| index as u64 + 1);
        match newest {
            Some(rev) => {
                self.head = rev;
                true
            }
            None => false,
        }
    }

    /// Head to `rev`; false for an unknown one.
    pub fn goto(&mut self, rev: u64) -> bool {
        if self.state(rev).is_none() {
            return false;
        }
        self.head = rev;
        true
    }

    /// The declared `Tree` record: the nodes in revision order, then
    /// head and published.
    pub fn tree(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_u32(&mut out, self.states.len());
        for (index, state) in self.states.iter().enumerate() {
            out.extend_from_slice(&(index as u64 + 1).to_le_bytes());
            out.extend_from_slice(&state.parent.to_le_bytes());
            put_u32(&mut out, state.label.len());
            out.extend_from_slice(state.label.as_bytes());
        }
        out.extend_from_slice(&self.head.to_le_bytes());
        out.extend_from_slice(&self.published.to_le_bytes());
        out
    }
}

fn put_u32(out: &mut Vec<u8>, value: usize) {
    let value = u32::try_from(value).unwrap_or(u32::MAX);
    out.extend_from_slice(&value.to_le_bytes());
}
