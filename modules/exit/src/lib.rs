#![no_std]
#![forbid(unsafe_code)]
//! The exit Kernel command, invoked through ordinary Resource access.

extern crate alloc;

use reovim_call::empty;
use reovim_kernel_interface::{Message, RequestError};

/// # Errors
/// Reports invalid arguments or failed granted Resource access.
pub fn invoke(input: Message) -> Result<Message, RequestError> {
    empty(&input)?;
    Ok(Message::new(input.contract, &[]))
}

pub const INFO: &[u8] = b"write\tcommand.exit\t1\targs\noutput\tclose\n";

struct Command;
impl Command {
    fn new(_: reovim_module::Connection) -> Self {
        Self
    }

    #[allow(clippy::unused_self)] // Uniform Resource handler signature.
    fn call(
        &self,
        _: reovim_module::ResourceId,
        input: Message,
    ) -> core::future::Ready<Result<Message, RequestError>> {
        core::future::ready(invoke(input))
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
    reovim_module::Contract::new("command.exit", 1);
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
