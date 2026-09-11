//! Bounded invalidation readers; snapshots and registration share one callback.

use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use reovim_kernel_interface::{
    Contract, Error, Message,
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};

const READERS: usize = 32;
#[derive(Default)]
pub struct Feed {
    pub revision: u64,
    pub serial: u64,
    readers: Vec<Weak<RefCell<Reader>>>,
}
impl Feed {
    pub fn prepare_change(&self) -> Result<(), Error> {
        self.revision.checked_add(1).ok_or(Error::PRESSURE).map(|_| ())
    }
    pub fn prepare_reader(&mut self) -> Result<(), Error> {
        self.readers.retain(|reader| {
            reader.upgrade().is_some_and(|r| !r.borrow().closed)
        });
        if self.readers.len() == READERS || self.serial == u64::MAX {
            return Err(Error::PRESSURE);
        }
        self.readers.try_reserve(1).map_err(|_| Error::PRESSURE)
    }
    pub fn subscribe(&mut self, reader: &Rc<RefCell<Reader>>) {
        self.serial += 1;
        self.readers.push(Rc::downgrade(reader));
    }
}

pub fn changed(feed: &Rc<RefCell<Feed>>) {
    let mut wakes: [Option<Waker>; READERS] = core::array::from_fn(|_| None);
    {
        let mut feed = feed.borrow_mut();
        feed.revision += 1; // Reserved before the value mutation.
        for (reader, wake) in feed.readers.iter().zip(&mut wakes) {
            *wake = notify(reader, feed.revision);
        }
    }
    for wake in wakes.into_iter().flatten() {
        wake.wake();
    }
}
fn notify(reader: &Weak<RefCell<Reader>>, revision: u64) -> Option<Waker> {
    let reader = reader.upgrade()?;
    let mut reader = reader.borrow_mut();
    reader.latest = revision;
    reader.wake.take()
}

pub struct Reader {
    cursor: u64,
    latest: u64,
    gap: bool,
    closed: bool,
    reading: bool,
    wake: Option<Waker>,
}
impl Reader {
    pub fn new(revision: u64) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            cursor: revision,
            latest: revision,
            gap: false,
            closed: false,
            reading: false,
            wake: None,
        }))
    }
}
struct Next {
    reader: Rc<RefCell<Reader>>,
    contract: Contract,
}
impl Future for Next {
    type Output = Result<Message, Error>;
    fn poll(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let mut reader = self.reader.borrow_mut();
        if reader.closed {
            return Poll::Ready(Err(Error::CLOSED));
        }
        if reader.gap {
            return Poll::Ready(Err(Error::STALE));
        }
        if reader.latest == reader.cursor {
            reader.wake = Some(context.waker().clone());
            return Poll::Pending;
        }
        let gap = reader.latest - reader.cursor > 1;
        let mut data = [0; 9];
        data[0] = u8::from(gap);
        data[1..].copy_from_slice(&reader.latest.to_le_bytes());
        let reply = Message::new(self.contract.clone(), &data);
        reader.gap = gap;
        reader.cursor = reader.latest;
        Poll::Ready(Ok(reply))
    }
}
impl Drop for Next {
    fn drop(&mut self) {
        let mut reader = self.reader.borrow_mut();
        reader.reading = false;
        reader.wake = None;
    }
}

pub fn provider(reader: Rc<RefCell<Reader>>) -> ResourceProvider {
    let closing = reader.clone();
    ResourceProvider::new(
        reader,
        ResourceOps {
            unload: None,
            read: |reader, _, input| next(reader, input),
            write: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
        },
        Box::new([Surface {
            contract: Contract::new("env.next", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
    .owned(move || close(&closing))
}
fn next(
    reader: &Rc<RefCell<Reader>>,
    input: Message,
) -> reovim_kernel_interface::provider::ProviderReply {
    let mut state = reader.borrow_mut();
    if !input.data.is_empty() || state.reading {
        let error = if state.reading { Error::BUSY } else { Error::INVALID };
        return Box::pin(async move { Err(error) });
    }
    state.reading = true;
    Box::pin(Next { reader: reader.clone(), contract: input.contract })
}
fn close(reader: &Rc<RefCell<Reader>>) {
    let wake = {
        let mut reader = reader.borrow_mut();
        reader.closed = true;
        reader.wake.take()
    };
    if let Some(wake) = wake {
        wake.wake();
    }
}
