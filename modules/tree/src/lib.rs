#![no_std]
#![forbid(unsafe_code)]
//! Passive, bounded Resource hierarchy inspection through granted access.

extern crate alloc;
mod walk;

use alloc::{format, string::String};
use core::future::{Ready, ready};
use reovim_module::{
    Connection, Contract, Error, Message, RequestError, ResourceId, rv_module,
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
        let path = reovim_call::optional(&input.data, ".")
            .map_err(RequestError::Rejected)?;
        let label = label(&self.0, &path).await?;
        let mut references = input.resources.to_vec();
        let root =
            reovim_call::subject(&self.0, &label, &mut references).await?;
        if !references.is_empty() {
            return Err(RequestError::Rejected(Error::INVALID));
        }
        let tree = walk::tree(&self.0, root, &label).await?;
        Ok(Message::new(input.contract, tree.as_bytes()))
    }

    #[allow(clippy::unused_self)]
    fn info(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, Error>> {
        ready(Ok(Message::new(
            input.contract,
            concat!(
                "write\tcommand.tree\t1\targs\noutput\tbytes\n",
                "usage\ttree [PATH]; omitted PATH means CWD; / means root\n",
            )
            .as_bytes(),
        )))
    }
}

#[allow(clippy::future_not_send)]
async fn label(
    kernel: &Connection,
    path: &str,
) -> Result<String, RequestError> {
    if path.starts_with('@') {
        return Ok(path.into());
    }
    let cwd = if path.starts_with('/') {
        String::new()
    } else {
        reovim_call::cwd(kernel).await?
    };
    Ok(format!("/{}", reovim_call::path(&cwd, path)))
}

const CALL: Contract = Contract::new("command.tree", 1).forwarding();
const BYTES: Contract = Contract::new("resource.bytes", 1);
rv_module! {
    pub MODULE {
        contract: ("tree", 1),
        state: Command::new,
        resources: {
            "" => { write CALL => Command::call },
            "info" => { read BYTES => Command::info },
        },
    }
}
