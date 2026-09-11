#![no_std]
#![forbid(unsafe_code)]
//! The help Kernel command, invoked through ordinary Resource access.

extern crate alloc;

use alloc::{string::String, vec::Vec};
use reovim_call::{optional, subject};
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError,
};

#[allow(clippy::future_not_send)]
/// # Errors
/// Reports invalid arguments or failed granted Resource access.
pub async fn invoke(
    kernel: &Connection,
    input: Message,
) -> Result<Message, RequestError> {
    let path = optional(&input.data, ".").map_err(RequestError::Rejected)?;
    let id = subject(kernel, &path, &mut Vec::new()).await?;
    let entries = kernel.children(id).await?;
    let mut data = String::new();
    for entry in entries {
        let Ok(info) = kernel.resolve_at(entry.resource, "info").await else {
            continue;
        };
        let read = kernel
            .read(info, &Message::new(Contract::new("resource.bytes", 1), &[]))
            .await;
        let Ok(reply) = read else { continue };
        let text = core::str::from_utf8(&reply.data)
            .map_err(|_| RequestError::Failed(Error::INVALID))?;
        data.push_str(&entry.name);
        data.push('\n');
        data.push_str(text);
    }
    Ok(Message::new(input.contract, data.as_bytes()))
}

pub const INFO: &[u8] = b"write\tcommand.help\t1\targs\noutput\tbytes\n";

struct Command(Connection);
impl Command {
    const fn new(kernel: Connection) -> Self {
        Self(kernel)
    }

    #[allow(clippy::future_not_send)]
    async fn call(
        &self,
        _: reovim_module::ResourceId,
        input: Message,
    ) -> Result<Message, RequestError> {
        invoke(&self.0, input).await
    }

    #[allow(clippy::unused_self)] // Uniform Resource handler signature.
    fn info(
        &self,
        _: reovim_module::ResourceId,
        input: Message,
    ) -> core::future::Ready<Result<Message, reovim_module::Error>> {
        core::future::ready(Ok(Message::new(input.contract, INFO)))
    }
}

const CALL: reovim_module::Contract =
    reovim_module::Contract::new("command.help", 1);
const BYTES: reovim_module::Contract =
    reovim_module::Contract::new("resource.bytes", 1);

reovim_module::rv_module! {
    pub MODULE {
        contract: ("help", 1),
        state: Command::new,
        resources: {
            "" => { write CALL => Command::call },
            "info" => { read BYTES => Command::info },
        },
    }
}
