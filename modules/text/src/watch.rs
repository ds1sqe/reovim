use alloc::{boxed::Box, rc::Rc};
use core::cell::Cell;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId, buffer,
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};

pub struct Watch {
    pub kernel: Connection,
    pub source: ResourceId,
    pub closed: Cell<bool>,
    pub ended: Cell<bool>,
}

impl Watch {
    pub fn end(&self) -> Result<(), Error> {
        self.ended.set(true);
        self.kernel.close(self.source).into_inner().map_err(crate::view::cause)
    }
    fn close(&self) -> Result<(), Error> {
        self.closed.set(true);
        self.end()
    }

    #[allow(clippy::future_not_send)]
    async fn next(&self, input: Message) -> Result<Message, RequestError> {
        if self.closed.get() {
            return Err(Error::CLOSED.into());
        }
        let wait = match input.data.as_ref() {
            [] => true,
            [0] => false,
            _ => return Err(Error::INVALID.into()),
        };
        if self.ended.get() {
            return Ok(Message::new(input.contract, &[2, 1]));
        }
        let result = buffer::next(&self.kernel, self.source, wait).await;
        if self.closed.get() {
            return Err(Error::CLOSED.into());
        }
        if self.ended.get() {
            return Ok(Message::new(input.contract, &[2, 1]));
        }
        let mut reply = result?;
        reply.contract = input.contract;
        Ok(reply)
    }
}

pub fn provider(watch: Rc<Watch>) -> ResourceProvider {
    let closing = watch.clone();
    ResourceProvider::new(
        watch,
        ResourceOps {
            unload: None,
            read: |watch, _, input| {
                let watch = watch.clone();
                Box::pin(async move { watch.next(input).await })
            },
            write: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED.into()) }),
        },
        Box::new([Surface {
            contract: Contract::new("text.next", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
    .try_owned(move || closing.close())
}
