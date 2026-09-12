#![no_std]
#![forbid(unsafe_code)]
//! Client-owned Text interpretation over public source/Buffer contracts.

extern crate alloc;
pub mod api;
mod index;
mod scan;
mod snapshot;
mod view;
mod watch;

use alloc::{rc::Rc, vec::Vec};
use core::cell::{Cell, RefCell};
use reovim_kernel_interface::provider::ResourceProvider;
use reovim_module::{
    Connection, Contract, Error, Message, Permissions, RequestError, ResourceId,
};

const ATTACH: Contract = Contract::new("text.attach", 1)
    .input(&[Permissions::ALL])
    .output(&[Permissions::ALL]);
const READ_ONLY: Contract = Contract::new("text.attach.read", 1)
    .input(&[Permissions::READ])
    .output(&[Permissions::READ]);

pub(crate) struct Factory {
    kernel: Connection,
    serial: Cell<u64>,
}
impl Factory {
    fn publish(
        &self,
        parent: ResourceId,
        prefix: &str,
        provider: ResourceProvider,
    ) -> Result<ResourceId, Error> {
        let serial = self.serial.get().checked_add(1).ok_or(Error::PRESSURE)?;
        self.serial.set(serial);
        let name = alloc::format!("{prefix}-{serial}");
        self.kernel.publish(parent, &name, provider).map_err(|(error, _)| error)
    }
}

struct Module(Rc<Factory>);
impl Module {
    fn new(kernel: Connection) -> Self {
        Self(Rc::new(Factory { kernel, serial: Cell::new(0) }))
    }

    #[allow(clippy::future_not_send)]
    async fn attach(
        &self,
        subject: ResourceId,
        input: Message,
    ) -> Result<Message, RequestError> {
        if !input.data.is_empty() {
            return Err(Error::INVALID.into());
        }
        let writable = input.contract.name.as_ref() == "text.attach";
        let rights =
            if writable { Permissions::ALL } else { Permissions::READ };
        let buffer = input.resources[0];
        let kernel = self.0.kernel.capture(buffer, rights)?;
        let descriptor = kernel
            .read(
                buffer,
                &Message::new(
                    reovim_kernel_interface::Contract::new("source.bytes", 1),
                    &[],
                ),
            )
            .await?;
        let observed =
            reovim_kernel_interface::buffer::observation(descriptor)?;
        reovim_module_source::api::length(&observed)?;
        let view = Rc::new(view::View {
            factory: self.0.clone(),
            parent: subject,
            buffer,
            writable,
            kernel: RefCell::new(Some(kernel)),
            closed: Cell::new(false),
            active: Cell::new(0),
            snapshots: RefCell::new(Vec::new()),
            watches: RefCell::new(Vec::new()),
            pending: RefCell::new(Vec::new()),
            index_hint: RefCell::new(None),
        });
        let id = self.0.publish(subject, "view", view::provider(view))?;
        Ok(Message::reference(input.contract, id))
    }
}

reovim_module::rv_module! {
    pub TEXT {
        contract: ("text", 1),
        state: Module::new,
        resources: { "" => { write ATTACH => Module::attach,
            write READ_ONLY => Module::attach } },
    }
}
