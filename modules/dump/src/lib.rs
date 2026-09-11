#![no_std]
#![forbid(unsafe_code)]
//! Finite byte export through borrowed source and Host directory Resources.

extern crate alloc;
mod capture;
mod output;
#[cfg(test)]
mod tests;

use core::future::{Ready, ready};
use reovim_module::{
    Connection, Contract, Error, Message, Permissions, RequestError,
    ResourceId, rv_module,
};

struct Command(Connection);
impl Command {
    const fn new(kernel: Connection) -> Self {
        Self(kernel)
    }
    #[allow(clippy::future_not_send)]
    async fn call(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Result<Message, RequestError> {
        let name =
            core::str::from_utf8(&input.data).map_err(|_| Error::INVALID)?;
        if name.is_empty()
            || name.contains(['/', '\0'])
            || matches!(name, "." | "..")
            || input.resources.len() != 2
        {
            return Err(Error::INVALID.into());
        }
        let mut nodes = capture::tree(&self.0, input.resources[0]).await?;
        let report =
            output::export(&self.0, input.resources[1], name, &mut nodes)
                .await?;
        Ok(Message::new(input.contract, report.as_bytes()))
    }
    #[allow(clippy::unused_self)]
    fn info(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, Error>> {
        ready(Ok(Message::new(
            input.contract,
            b"write\tcommand.dump\t1\tutf8\noutput\tbytes\n",
        )))
    }
}
const CALL: Contract = Contract::new("command.dump", 1)
    .input(&[Permissions::READ, Permissions::WRITE]);
const BYTES: Contract = Contract::new("resource.bytes", 1);
rv_module! {
    pub MODULE {
        contract: ("dump", 1),
        state: Command::new,
        resources: {
            "" => { write CALL => Command::call },
            "info" => { read BYTES => Command::info },
        },
    }
}
