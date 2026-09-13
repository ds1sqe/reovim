use super::*;
use reovim_kernel_interface::{
    access::{Access, Method, Reply},
    namespace::ResourceEntry,
};

pub fn once(kernel: Connection) -> Connection {
    Connection::new(Rc::new(Fault { kernel, fail: Cell::new(true) }))
}

struct Fault {
    kernel: Connection,
    fail: Cell<bool>,
}

impl Access for Fault {
    fn capture(
        &self,
        id: ResourceId,
        rights: Permissions,
    ) -> Result<Connection, Error> {
        self.kernel.capture(id, rights)
    }
    fn root(&self) -> Result<ResourceId, Error> {
        self.kernel.root().into_inner().map_err(|error| code(&error))
    }
    fn resolve_at(
        &self,
        parent: ResourceId,
        path: &str,
    ) -> Result<ResourceId, Error> {
        self.kernel
            .resolve_at(parent, path)
            .into_inner()
            .map_err(|error| code(&error))
    }
    fn children(
        &self,
        parent: ResourceId,
    ) -> Result<Vec<ResourceEntry>, Error> {
        self.kernel.children(parent).into_inner().map_err(|error| code(&error))
    }
    fn close(&self, id: ResourceId) -> Result<(), Error> {
        self.kernel.close(id).into_inner().map_err(|error| code(&error))
    }
    fn reconnect(&self) -> Result<Connection, Error> {
        self.kernel.reconnect()
    }
    fn request(
        &self,
        method: Method,
        subject: ResourceId,
        input: &Message,
    ) -> Reply {
        if input.contract.name.as_ref() == "kernel.request.result"
            && self.fail.replace(false)
        {
            return Box::pin(core::future::ready(Err(Error::PRESSURE.into())));
        }
        match method {
            Method::Read => self.kernel.read(subject, input),
            Method::Write => self.kernel.write(subject, input),
        }
    }
}
