use alloc::boxed::Box;
use reovim_kernel_interface::{
    Contract, Error, Message, ResourceId, access::Method, provider::Permissions,
};

use crate::Runtime;

impl Runtime {
    pub(crate) fn begin_module_unload(
        &mut self,
        id: ResourceId,
        input: &Message,
    ) -> Result<(), Error> {
        if !input.data.is_empty() {
            return Err(Error::INVALID);
        }
        let result = self.close_module(id);
        let instance = self
            .modules
            .iter()
            .find(|m| m.id == id)
            .ok_or(Error::UNAVAILABLE)?;
        if result == Err(Error::BUSY) && instance.closing {
            return Ok(());
        }
        result
    }

    pub(crate) fn module_unload_result(
        &self,
        id: ResourceId,
        input: &Message,
    ) -> Result<Option<Message>, Error> {
        let instance = self
            .modules
            .iter()
            .find(|m| m.id == id)
            .ok_or(Error::UNAVAILABLE)?;
        if instance.module.is_some() || instance.factory.is_some() {
            return Ok(None);
        }
        if let Some(error) = instance.cleanup_error {
            return Err(error);
        }
        Ok(Some(Message::new(input.contract.clone(), &[])))
    }
}

pub fn surface(
    method: Method,
    contract: &Contract,
) -> Result<Box<[Permissions]>, Error> {
    if contract.version != 1 {
        return Err(Error::UNSUPPORTED);
    }
    match (method, contract.name.as_ref()) {
        (Method::Read, "kernel.module.status" | "kernel.module.ready")
        | (Method::Write, "kernel.module.unload") => Ok(Box::new([])),
        _ => Err(Error::UNSUPPORTED),
    }
}

pub fn during_close(method: Method, contract: &Contract) -> bool {
    contract.version == 1
        && matches!(
            (method, contract.name.as_ref()),
            (Method::Read, "kernel.module.status")
                | (Method::Write, "kernel.module.unload")
        )
}
