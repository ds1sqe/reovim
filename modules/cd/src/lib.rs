#![no_std]
#![forbid(unsafe_code)]
//! The cd Kernel command, invoked through ordinary Resource access.

extern crate alloc;

use alloc::string::String;
use reovim_call::{cwd, optional};
use reovim_kernel_interface::{Connection, Contract, Message, RequestError};

#[allow(clippy::future_not_send)]
/// # Errors
/// Reports invalid arguments or failed granted Resource access.
pub async fn invoke(
    kernel: &Connection,
    input: Message,
) -> Result<Message, RequestError> {
    let target = optional(&input.data, "/").map_err(RequestError::Rejected)?;
    let current = if target.starts_with('/') {
        String::new()
    } else {
        cwd(kernel).await?
    };
    let path = reovim_call::path(&current, &target);
    let id = kernel.resolve(&path).await?;
    kernel.children(id).await?;
    let value = kernel.resolve("env/CWD").await?;
    kernel
        .write(
            value,
            &Message::new(Contract::new("resource.bytes", 1), path.as_bytes()),
        )
        .await?;
    Ok(Message::new(input.contract, &[]))
}

pub const INFO: &[u8] = b"write\tcommand.cd\t1\targs\noutput\tbytes\n";

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
    reovim_module::Contract::new("command.cd", 1);
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
