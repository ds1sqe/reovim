#![no_std]
#![forbid(unsafe_code)]
//! The dmesg Kernel command, invoked through ordinary Resource access.

extern crate alloc;

use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError,
};

pub const INFO: &[u8] = concat!(
    "write\tcommand.dmesg\t1\tutf8\n",
    "output\tbytes\n",
    "usage\tdmesg [AFTER_SEQUENCE]\n",
    "Reads the existing bounded Kernel diagnostic stream.\n",
)
.as_bytes();

#[allow(clippy::future_not_send)] // Imported access uses its owning executor.
/// # Errors
/// Reports invalid input or failed granted Resource reads.
pub async fn invoke(
    kernel: &Connection,
    input: Message,
) -> Result<Message, RequestError> {
    if !input.resources.is_empty() {
        return Err(RequestError::Rejected(Error::INVALID));
    }
    let text = core::str::from_utf8(&input.data)
        .map_err(|_| RequestError::Rejected(Error::INVALID))?;
    let after: u64 = if text.is_empty() {
        0
    } else {
        text.parse().map_err(|_| RequestError::Rejected(Error::INVALID))?
    };
    let log = kernel.resolve("kernel/log").await?;
    let query = Message::new(
        Contract::new("kernel.diagnostics", 1),
        &after.to_le_bytes(),
    );
    let reply = kernel.read(log, &query).await?;
    Ok(Message::new(input.contract, &reply.data))
}

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
    reovim_module::Contract::new("command.dmesg", 1);
const BYTES: reovim_module::Contract =
    reovim_module::Contract::new("resource.bytes", 1);

reovim_module::rv_module! {
    pub MODULE {
        contract: ("dmesg", 1),
        state: Command::new,
        resources: {
            "" => { write CALL => Command::call },
            "info" => { read BYTES => Command::info },
        },
    }
}
