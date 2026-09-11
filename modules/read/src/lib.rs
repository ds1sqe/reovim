#![no_std]
#![forbid(unsafe_code)]
//! The read Kernel command, invoked through ordinary Resource access.

extern crate alloc;

use reovim_kernel_interface::{
    Connection, Message, RequestError, access::Method,
};

/// # Errors
/// Reports contract, reference or operation failures from the Resource.
#[allow(clippy::future_not_send)]
pub async fn invoke(
    kernel: &Connection,
    input: Message,
) -> Result<Message, RequestError> {
    reovim_call::invoke(kernel, Method::Read, input).await
}

pub const INFO: &[u8] = b"write\tcommand.read\t1\targs\noutput\tmessage\n";

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
    reovim_module::Contract::new("command.read", 1).forwarding();
const BYTES: reovim_module::Contract =
    reovim_module::Contract::new("resource.bytes", 1);

reovim_module::rv_module! {
    pub MODULE {
        state: Command::new,
        resources: {
            "" => { write CALL => Command::call },
            "info" => { read BYTES => Command::info },
        },
    }
}
