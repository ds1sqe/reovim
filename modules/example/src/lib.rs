#![no_std]
#![forbid(unsafe_code)]
//! A module with its own command and an ordinary statistics Resource.

extern crate alloc;

use alloc::{format, vec::Vec};
use core::{
    cell::{Cell, RefCell},
    future::{Ready, poll_fn, ready},
    task::{Context, Poll, Waker},
};
use reovim_module::{
    Connection, Contract, Error, Message, ResourceId, rv_module,
};

const GREET: Contract = Contract::new("example.greet", 1);
const BYTES: Contract = Contract::new("resource.bytes", 1);
const STATS: Contract = Contract::new("example.calls", 1);
const WAIT: Contract = Contract::new("example.wait", 1);
const SIGNAL: Contract = Contract::new("example.signal", 1);

struct Example {
    calls: Cell<u64>,
    signalled: Cell<bool>,
    waiters: RefCell<Vec<Waker>>,
}

impl Example {
    fn new(_: Connection) -> Self {
        Self {
            calls: Cell::new(0),
            signalled: Cell::new(false),
            waiters: RefCell::new(Vec::new()),
        }
    }

    #[allow(clippy::future_not_send)]
    async fn wait(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Result<Message, Error> {
        if !input.data.is_empty() {
            return Err(Error::INVALID);
        }
        poll_fn(|context| self.poll_wait(context)).await?;
        Ok(Message::new(input.contract, b"released"))
    }

    fn poll_wait(&self, context: &Context<'_>) -> Poll<Result<(), Error>> {
        if self.signalled.get() {
            return Poll::Ready(Ok(()));
        }
        let mut waiters = self.waiters.borrow_mut();
        if waiters.iter().any(|wake| wake.will_wake(context.waker())) {
            return Poll::Pending;
        }
        if waiters.len() == 32 {
            return Poll::Ready(Err(Error::PRESSURE));
        }
        waiters.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        waiters.push(context.waker().clone());
        Poll::Pending
    }

    fn signal(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, Error>> {
        let result = Self::read_value(input, b"signalled");
        if result.is_err() {
            return ready(result);
        }
        self.signalled.set(true);
        let waiters = core::mem::take(&mut *self.waiters.borrow_mut());
        for waiter in waiters {
            waiter.wake();
        }
        ready(result)
    }

    fn greet(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, Error>> {
        ready(self.greeting(input))
    }

    fn greeting(&self, input: Message) -> Result<Message, Error> {
        let name =
            core::str::from_utf8(&input.data).map_err(|_| Error::INVALID)?;
        let name = if name.is_empty() { "world" } else { name };
        let next = self.calls.get().checked_add(1).ok_or(Error::PRESSURE)?;
        let reply = Message::new(
            input.contract,
            format!("Hello, {name}!\n").as_bytes(),
        );
        self.calls.set(next);
        Ok(reply)
    }

    #[allow(clippy::unused_self)]
    fn info(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, Error>> {
        ready(Self::read_value(
            input,
            b"write\texample.greet\t1\tutf8\noutput\tbytes\n",
        ))
    }

    fn statistics(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, Error>> {
        ready(Self::read_value(input, &self.calls.get().to_le_bytes()))
    }

    fn read_value(input: Message, data: &[u8]) -> Result<Message, Error> {
        if !input.data.is_empty() {
            return Err(Error::INVALID);
        }
        Ok(Message::new(input.contract, data))
    }
}

rv_module! {
    pub MODULE {
        state: Example::new,
        resources: {
            "" => { write GREET => Example::greet },
            "info" => { read BYTES => Example::info },
            "statistics" => { read STATS => Example::statistics },
            "wait" => { write WAIT => Example::wait },
            "signal" => { write SIGNAL => Example::signal },
        },
    }
}
