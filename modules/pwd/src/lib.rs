#![no_std]
#![forbid(unsafe_code)]
//! The pwd Kernel command, invoked through ordinary Resource access.

extern crate alloc;

use alloc::format;
use reovim_call::{cwd, empty};
use reovim_kernel_interface::{Connection, Message, RequestError};

#[allow(clippy::future_not_send)]
/// # Errors
/// Reports invalid arguments or failed granted Resource access.
pub async fn invoke(
    kernel: &Connection,
    input: Message,
) -> Result<Message, RequestError> {
    empty(&input)?;
    Ok(Message::new(
        input.contract,
        format!("/{}\n", cwd(kernel).await?).as_bytes(),
    ))
}

pub const INFO: &[u8] = b"write\tcommand.pwd\t1\targs\noutput\tbytes\n";

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
    reovim_module::Contract::new("command.pwd", 1);
const BYTES: reovim_module::Contract =
    reovim_module::Contract::new("resource.bytes", 1);

reovim_module::rv_module! {
    pub MODULE {
        contract: ("pwd", 1),
        state: Command::new,
        resources: {
            "" => { write CALL => Command::call },
            "info" => { read BYTES => Command::info },
        },
    }
}
