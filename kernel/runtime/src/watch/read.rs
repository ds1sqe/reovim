use super::{Reader, Status, close};
use alloc::{boxed::Box, rc::Rc};
use core::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Contract, Error, Message,
    buffer::{Change, Transition},
    provider::{
        Permissions, ProviderReply, ResourceOps, ResourceProvider, Surface,
    },
};

struct Read {
    reader: Rc<RefCell<Reader>>,
    contract: Contract,
    wait: bool,
}

impl Future for Read {
    type Output = Result<Message, Error>;
    fn poll(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let mut reader = self.reader.borrow_mut();
        if reader.status == Status::Closed {
            return Poll::Ready(Err(Error::CLOSED));
        }
        if let Some(change) = reader.events.pop_front() {
            return Poll::Ready(Ok(encode(&self.contract, change)));
        }
        let bytes = if reader.status == Status::Ended {
            &[2, u8::from(reader.gap)][..]
        } else if reader.gap {
            &[1]
        } else if !self.wait {
            &[3]
        } else {
            reader.wake = Some(context.waker().clone());
            return Poll::Pending;
        };
        Poll::Ready(Ok(Message::new(self.contract.clone(), bytes)))
    }
}

impl Drop for Read {
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
            contract: Contract::new("kernel.buffer.next", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
    .owned(move || close(&closing))
}

fn next(reader: &Rc<RefCell<Reader>>, input: Message) -> ProviderReply {
    let wait = match input.data.as_ref() {
        [] => true,
        [0] => false,
        _ => return Box::pin(async { Err(Error::INVALID) }),
    };
    let mut state = reader.borrow_mut();
    if state.reading {
        return Box::pin(async { Err(Error::BUSY) });
    }
    state.reading = true;
    Box::pin(Read { reader: reader.clone(), contract: input.contract, wait })
}

fn encode(contract: &Contract, change: Change) -> Message {
    let mut data = [0; super::EVENT_BYTES];
    data[1] = match change.transition {
        Transition::Edit => 0,
        Transition::Undo => 1,
        Transition::Redo => 2,
    };
    let fields = [
        change.request,
        change.before,
        change.after,
        change.from_node,
        change.to_node,
    ];
    for (slot, value) in data[2..].as_chunks_mut::<8>().0.iter_mut().zip(fields)
    {
        slot.copy_from_slice(&value.to_le_bytes());
    }
    Message::new(contract.clone(), &data)
}
