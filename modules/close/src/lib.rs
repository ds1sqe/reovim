#![no_std]
#![forbid(unsafe_code)]
//! The close Kernel command, invoked through ordinary Resource access.

extern crate alloc;

use reovim_call::{arguments, subject};
use reovim_kernel_interface::{Connection, Error, Message, RequestError};

#[allow(clippy::future_not_send)]
/// # Errors
/// Reports invalid arguments or failed granted Resource access.
pub async fn invoke(
    kernel: &Connection,
    input: Message,
) -> Result<Message, RequestError> {
    let args = arguments(&input.data).map_err(RequestError::Rejected)?;
    let [path] = args.as_slice() else {
        return Err(RequestError::Rejected(Error::INVALID));
    };
    let id = subject(kernel, path, &mut input.resources.to_vec()).await?;
    kernel.close(id).await?;
    Ok(Message::new(input.contract, &[]))
}

pub const INFO: &[u8] = b"write\tcommand.close\t1\targs\noutput\tmessage\n";

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
    reovim_module::Contract::new("command.close", 1).forwarding();
const BYTES: reovim_module::Contract =
    reovim_module::Contract::new("resource.bytes", 1);

reovim_module::rv_module! {
    pub MODULE {
        contract: ("close", 1),
        state: Command::new,
        resources: {
            "" => { write CALL => Command::call },
            "info" => { read BYTES => Command::info },
        },
    }
}
