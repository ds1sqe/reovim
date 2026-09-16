//! A Buffer's versioned state, as 01-core-runtime.md describes it: a
//! tree of stored states, a head and a published pointer. Nothing here
//! knows an rd or the Kernel.

use alloc::string::String;
use alloc::vec::Vec;

use crate::history::{Node, Tree};

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

    /// `bytes` in place of head's; false at the first state, which
    /// `open` made.
    pub fn amend(&mut self, bytes: Vec<u8>) -> bool {
        let head = usize::try_from(self.head).unwrap_or(0);
        match (head, self.states.get_mut(head.wrapping_sub(1))) {
            (0 | 1, _) | (_, None) => false,
            (_, Some(state)) => {
                state.bytes = bytes;
                true
            }
        }
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

    /// The declared `Tree`: the nodes in revision order, then head
    /// and published.
    pub fn tree(&self) -> Tree {
        let node = |(index, state): (usize, &State)| Node {
            id: index as u64 + 1,
            parent: state.parent,
            label: state.label.clone(),
        };
        let nodes = self.states.iter().enumerate().map(node).collect();
        Tree { nodes, head: self.head, published: self.published }
    }
}
