#![no_std]
#![forbid(unsafe_code)]
//! The cat Kernel command, invoked through ordinary Resource access.

extern crate alloc;

use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError,
};

pub const INFO: &[u8] = concat!(
    "write\tcommand.cat\t1\tutf8\n",
    "input\tResource with read access\n",
    "Reads resource.bytes by default; data may specify NAME\\0VERSION.\n",
    "Returns the finite value and its borrowed source reference.\n",
)
.as_bytes();

#[allow(clippy::future_not_send)] // Imported access uses its owning executor.
/// # Errors
/// Reports invalid input or failed granted Resource reads.
pub async fn invoke(
    kernel: &Connection,
    input: Message,
) -> Result<Message, RequestError> {
    let [source] = input.resources.as_ref() else {
        return Err(RequestError::Rejected(Error::INVALID));
    };
    let contract =
        read_contract(&input.data).map_err(RequestError::Rejected)?;
    let reply = kernel.read(*source, &Message::new(contract, &[])).await?;
    if !reply.resources.is_empty() {
        return Err(RequestError::Failed(Error::UNSUPPORTED));
    }
    Ok(Message {
        contract: input.contract,
        data: reply.data,
        resources: input.resources,
    })
}

fn read_contract(data: &[u8]) -> Result<Contract, Error> {
    if data.is_empty() {
        return Ok(Contract::new("resource.bytes", 1));
    }
    let text = core::str::from_utf8(data).map_err(|_| Error::INVALID)?;
    let (name, version) = text.split_once('\0').ok_or(Error::INVALID)?;
    Ok(Contract::new(name, version.parse().map_err(|_| Error::INVALID)?))
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
    reovim_module::Contract::new("command.cat", 1)
        .input(&[reovim_module::Permissions::READ])
        .output(&[reovim_module::Permissions::READ]);
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
