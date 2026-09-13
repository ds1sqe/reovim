//! Fault witness: retain each real stage, then lose its first result retrieval.
use reovim_kernel_interface::{
    Connection, Error, Message, RequestError, ResourceId,
    access::{Access, Method, Reply},
    namespace::ResourceEntry,
    provider::Permissions,
};
use reovim_sdk::module::Definition;
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
struct State {
    capture: Cell<bool>,
    initialize: Cell<bool>,
    retrieval: Cell<bool>,
    fail_initialization: Cell<bool>,
}
struct Fault {
    kernel: Connection,
    state: Rc<State>,
}

static MODULE: Definition = Definition {
    identity: reovim_sdk::module::Identity::new("open", 1),
    dependencies: reovim_module_open::MODULE.dependencies,
    exports: reovim_module_open::MODULE.exports,
    construct: |kernel| {
        (reovim_module_open::MODULE.construct)(Connection::new(Rc::new(
            Fault { kernel, state: Rc::default() },
        )))
    },
};
reovim_module::rv_module_entry!(MODULE);

impl Fault {
    fn lose(&self, method: Method, input: &Message) -> bool {
        if method != Method::Write {
            return false;
        }
        match input.contract.name.as_ref() {
            "host.files.select" => !self.state.capture.replace(true),
            "kernel.buffer.open" => !self.state.initialize.replace(true),
            _ => false,
        }
    }

    fn retained(&self, subject: ResourceId, input: &Message) -> Reply {
        let result = self.admit(subject, input);
        Box::pin(core::future::ready(result))
    }

    fn admit(
        &self,
        subject: ResourceId,
        input: &Message,
    ) -> Result<Message, RequestError> {
        let root = self.kernel.resolve("requests").into_inner()?;
        let before = self.kernel.children(root).into_inner()?;
        if input.contract.name.as_ref() == "host.files.select" {
            self.state
                .fail_initialization
                .set(input.data.as_ref() == b"fail.md");
        }
        let mut input = input.clone();
        if input.contract.name.as_ref() == "kernel.buffer.open"
            && self.state.fail_initialization.get()
        {
            input.data = Box::new([1]);
        }
        let waiting = self.kernel.write(subject, &input);
        let after = self.kernel.children(root).into_inner()?;
        let work = after
            .iter()
            .find(|entry| {
                !before.iter().any(|old| old.resource == entry.resource)
            })
            .ok_or(Error::PROVIDER)?;
        drop(waiting);
        self.state.retrieval.set(true);
        Err(RequestError::Unknown {
            cause: Error::PRESSURE,
            recovery: format!("requests/{}", work.name).into(),
        })
    }
}

impl Access for Fault {
    fn capture(
        &self,
        id: ResourceId,
        rights: Permissions,
    ) -> Result<Connection, Error> {
        Ok(Connection::new(Rc::new(Self {
            kernel: self.kernel.capture(id, rights)?,
            state: self.state.clone(),
        })))
    }
    fn root(&self) -> Result<ResourceId, Error> {
        self.kernel.root().into_inner().map_err(code)
    }
    fn resolve_at(
        &self,
        root: ResourceId,
        path: &str,
    ) -> Result<ResourceId, Error> {
        self.kernel.resolve_at(root, path).into_inner().map_err(code)
    }
    fn children(&self, id: ResourceId) -> Result<Vec<ResourceEntry>, Error> {
        self.kernel.children(id).into_inner().map_err(code)
    }
    fn close(&self, id: ResourceId) -> Result<(), Error> {
        self.kernel.close(id).into_inner().map_err(code)
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
        if self.lose(method, input) {
            return self.retained(subject, input);
        }
        if input.contract.name.as_ref() == "kernel.request.result"
            && self.state.retrieval.replace(false)
        {
            return Box::pin(core::future::ready(Err(Error::PRESSURE.into())));
        }
        match method {
            Method::Read => self.kernel.read(subject, input),
            Method::Write => self.kernel.write(subject, input),
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn code(error: RequestError) -> Error {
    match error {
        RequestError::Rejected(error) | RequestError::Failed(error) => error,
        RequestError::Unknown { cause, .. } => cause,
    }
}
