//! Accepted state transitions; each reader owns its bounded delivery state.

use alloc::{
    collections::VecDeque,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::{cell::RefCell, task::Waker};
use reovim_kernel_interface::{Error, buffer::Change};

mod read;
pub use read::{mapped, provider};

const READERS: usize = 32;
const EVENTS: usize = 32;
pub const EVENT_BYTES: usize = 42;

pub struct Feed<T = Change> {
    readers: Vec<Weak<RefCell<Reader<T>>>>,
}
impl<T> Default for Feed<T> {
    fn default() -> Self {
        Self { readers: Vec::new() }
    }
}

impl<T> Feed<T> {
    pub fn prepare(&mut self) -> Result<Rc<RefCell<Reader<T>>>, Error> {
        self.readers.retain(|weak| {
            weak.upgrade()
                .is_some_and(|reader| reader.borrow().status != Status::Closed)
        });
        if self.readers.len() == READERS {
            return Err(Error::PRESSURE);
        }
        self.readers.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        let mut events = VecDeque::new();
        events.try_reserve_exact(EVENTS).map_err(|_| Error::PRESSURE)?;
        Ok(Rc::new(RefCell::new(Reader {
            events,
            gap: false,
            status: Status::Live,
            reading: false,
            wake: None,
        })))
    }

    pub fn subscribe(&mut self, reader: &Rc<RefCell<Reader<T>>>) {
        self.readers.push(Rc::downgrade(reader));
    }
}

pub struct Reader<T = Change> {
    events: VecDeque<T>,
    gap: bool,
    status: Status,
    reading: bool,
    wake: Option<Waker>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Status {
    Live,
    Ended,
    Closed,
}

/// Queue only after installation, then invoke wakes outside every state guard.
pub fn changed<T: Copy>(feed: &Rc<RefCell<Feed<T>>>, change: T) {
    notify(feed, Some(change));
}

pub fn end<T: Copy>(feed: &Rc<RefCell<Feed<T>>>) {
    notify(feed, None);
}

fn notify<T: Copy>(feed: &Rc<RefCell<Feed<T>>>, change: Option<T>) {
    let mut wakes: [Option<Waker>; READERS] = core::array::from_fn(|_| None);
    {
        let feed = feed.borrow();
        for (weak, wake) in feed.readers.iter().zip(&mut wakes) {
            *wake = queue(weak, change);
        }
    }
    for wake in wakes.into_iter().flatten() {
        wake.wake();
    }
}

fn queue<T: Copy>(
    weak: &Weak<RefCell<Reader<T>>>,
    change: Option<T>,
) -> Option<Waker> {
    let reader = weak.upgrade()?;
    let mut reader = reader.borrow_mut();
    if reader.status == Status::Closed {
        return None;
    }
    if change.is_none() {
        reader.status = Status::Ended;
    }
    if let Some(change) = change.filter(|_| !reader.gap) {
        if reader.events.len() == EVENTS {
            reader.events.clear();
            reader.gap = true;
        } else {
            reader.events.push_back(change);
        }
    }
    reader.wake.take()
}

pub fn close<T>(reader: &Rc<RefCell<Reader<T>>>) {
    let wake = {
        let mut reader = reader.borrow_mut();
        reader.status = Status::Closed;
        reader.events.clear();
        reader.wake.take()
    };
    if let Some(wake) = wake {
        wake.wake();
    }
}
