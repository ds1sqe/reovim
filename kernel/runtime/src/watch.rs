//! Accepted Buffer transitions; each reader owns its bounded delivery state.

use alloc::{
    collections::VecDeque,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::{cell::RefCell, task::Waker};
use reovim_kernel_interface::{Error, buffer::Change};

mod read;
pub use read::provider;

const READERS: usize = 32;
const EVENTS: usize = 32;
pub const EVENT_BYTES: usize = 42;

#[derive(Default)]
pub struct Feed {
    readers: Vec<Weak<RefCell<Reader>>>,
}

impl Feed {
    pub fn prepare(&mut self) -> Result<Rc<RefCell<Reader>>, Error> {
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

    pub fn subscribe(&mut self, reader: &Rc<RefCell<Reader>>) {
        self.readers.push(Rc::downgrade(reader));
    }
}

pub struct Reader {
    events: VecDeque<Change>,
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
pub fn changed(feed: &Rc<RefCell<Feed>>, change: Change) {
    notify(feed, Some(change));
}

pub fn end(feed: &Rc<RefCell<Feed>>) {
    notify(feed, None);
}

fn notify(feed: &Rc<RefCell<Feed>>, change: Option<Change>) {
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

fn queue(
    weak: &Weak<RefCell<Reader>>,
    change: Option<Change>,
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

fn close(reader: &Rc<RefCell<Reader>>) {
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
