//! One bounded, numbered ring of events; `since` reads it per watcher.

use alloc::collections::{BTreeSet, VecDeque};
use alloc::vec::Vec;

use reovim_kernel_interface::{Event, Kind, Rd};

use crate::table::ROOT;

pub struct Ring {
    events: VecDeque<Event>,
    next_seq: u64,
    cap: usize,
}

impl Ring {
    pub const fn new(cap: usize) -> Self {
        Self { events: VecDeque::new(), next_seq: 1, cap }
    }

    pub fn push(&mut self, rd: Rd, kind: Kind, child: Option<Rd>) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.events.push_back(Event { seq, rd, kind, child });
        while self.events.len() > self.cap {
            self.events.pop_front();
        }
    }

    /// Position of the newest event; the cursor after reading all.
    pub const fn head(&self) -> u64 {
        self.next_seq - 1
    }

    /// Events after `cursor` on `watched`, and the new cursor. A cursor
    /// the ring no longer covers yields one `Overflow` on `ROOT`.
    pub fn since(
        &self,
        cursor: u64,
        watched: &BTreeSet<Rd>,
    ) -> (Vec<Event>, u64) {
        let oldest = self.events.front().map_or(self.next_seq, |e| e.seq);
        if cursor + 1 < oldest {
            let event = Event {
                seq: self.head(),
                rd: ROOT,
                kind: Kind::Overflow,
                child: None,
            };
            return (alloc::vec![event], self.head());
        }
        let events = self
            .events
            .iter()
            .filter(|e| e.seq > cursor && watched.contains(&e.rd))
            .copied()
            .collect();
        (events, self.head())
    }
}
