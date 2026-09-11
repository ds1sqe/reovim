#![no_std]
#![forbid(unsafe_code)]
//! Private environment values in an independently loaded module instance.

extern crate alloc;
mod watch;

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    cell::RefCell,
    future::{Ready, ready},
};
use reovim_module::{
    Connection, Contract, Error, Message, Permissions, RequestError,
    ResourceId, rv_module,
};

const BYTES: Contract = Contract::new("resource.bytes", 1);
const NAMES: Contract = Contract::new("env.names", 1);
const OBSERVE: Contract =
    Contract::new("env.observe", 1).output(&[Permissions::READ]);
const REMOVE: Contract = Contract::new("env.remove", 1);
const CREATE: Contract =
    Contract::new("env.create", 1).output(&[Permissions::ALL]);
type Value = Rc<RefCell<Vec<u8>>>;
type Values = Vec<(Box<str>, Value)>;

struct Environment {
    kernel: Connection,
    values: RefCell<Values>,
    feed: Rc<RefCell<watch::Feed>>,
}
impl Environment {
    fn new(kernel: Connection) -> Self {
        Self {
            kernel,
            feed: Rc::default(),
            values: RefCell::new(alloc::vec![(
                "CWD".into(),
                Rc::new(RefCell::new(Vec::new()))
            )]),
        }
    }

    fn names(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, Error>> {
        let mut names: Vec<_> =
            self.values.borrow().iter().map(|(name, _)| name.clone()).collect();
        names.sort_unstable();
        ready(info(input, names.join("\0").as_bytes()))
    }

    fn create(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, RequestError>> {
        ready(self.create_value(input))
    }

    fn create_value(&self, input: Message) -> Result<Message, RequestError> {
        let split = input
            .data
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(Error::INVALID)?;
        let name = core::str::from_utf8(&input.data[..split])
            .map_err(|_| Error::INVALID)?;
        if name.is_empty() || name.contains('/') || matches!(name, "." | "..") {
            return Err(Error::INVALID.into());
        }
        self.feed.borrow().prepare_change()?;
        let parent = self.kernel.resolve("env").into_inner()?;
        let mut values = self.values.borrow_mut();
        if values.iter().any(|(key, _)| &**key == name) {
            return Err(Error::INVALID.into());
        }
        values.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        let name: Box<str> = name.into();
        let value = Rc::new(RefCell::new(input.data[split + 1..].to_vec()));
        let mut reply = Message::reference(input.contract, ResourceId(0));
        let resource = self
            .kernel
            .publish(
                parent,
                &name,
                value_provider(value.clone(), self.feed.clone()),
            )
            .map_err(|(error, _)| error)?;
        values.push((name, value));
        drop(values);
        watch::changed(&self.feed);
        reply.resources[0] = resource;
        Ok(reply)
    }

    fn remove(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, RequestError>> {
        ready(self.remove_value(input))
    }

    fn remove_value(&self, input: Message) -> Result<Message, RequestError> {
        let name =
            core::str::from_utf8(&input.data).map_err(|_| Error::INVALID)?;
        self.feed.borrow().prepare_change()?;
        let parent = self.kernel.resolve("env").into_inner()?;
        let mut values = self.values.borrow_mut();
        let index = values
            .iter()
            .position(|(key, _)| &**key == name)
            .ok_or(Error::UNAVAILABLE)?;
        let resource = self.kernel.resolve_at(parent, name).into_inner()?;
        let reply = Message::new(input.contract, &[]);
        self.kernel.withdraw(resource)?;
        values.remove(index);
        drop(values);
        watch::changed(&self.feed);
        Ok(reply)
    }

    #[allow(clippy::unused_self)]
    fn remove_info(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, Error>> {
        ready(info(input, b"write\tenv.remove\t1\tutf8\noutput\tbytes\n"))
    }

    #[allow(clippy::unused_self)]
    fn create_info(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, Error>> {
        ready(info(input, b"write\tenv.create\t1\targs\noutput\tmessage\n"))
    }

    fn cwd_read(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, Error>> {
        ready(self.read("CWD", input))
    }

    fn cwd_write(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, Error>> {
        ready(self.write("CWD", input))
    }

    fn read(&self, key: &str, input: Message) -> Result<Message, Error> {
        if !input.data.is_empty() {
            return Err(Error::INVALID);
        }
        let values = self.values.borrow();
        let value = &values
            .iter()
            .find(|(name, _)| &**name == key)
            .ok_or(Error::UNAVAILABLE)?
            .1;
        Ok(Message::new(input.contract, &value.borrow()))
    }

    fn write(&self, key: &str, input: Message) -> Result<Message, Error> {
        let values = self.values.borrow();
        let value = values
            .iter()
            .find(|(name, _)| &**name == key)
            .ok_or(Error::UNAVAILABLE)?
            .1
            .clone();
        drop(values);
        replace(&value, &self.feed, input)
    }

    #[allow(clippy::unused_self)] // Uniform Resource callback signature.
    fn value_info(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, Error>> {
        ready(info(input, b"write\tresource.bytes\t1\tutf8\noutput\tbytes\n"))
    }

    #[allow(clippy::unused_self)] // Uniform Resource callback signature.
    fn module_info(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, Error>> {
        ready(info(
            input,
            b"Environment values use resource.bytes. Initial key: CWD.\n",
        ))
    }
}
fn info(input: Message, data: &[u8]) -> Result<Message, Error> {
    if !input.data.is_empty() {
        return Err(Error::INVALID);
    }
    Ok(Message::new(input.contract, data))
}

rv_module! {
    pub MODULE {
        state: Environment::new,
        resources: {
            "env" => { read NAMES => Environment::names },
            "module/env/watch" => { read OBSERVE => Environment::observe },
            "module/env/create" => { write CREATE => Environment::create },
            "module/env/create/info" => {
                read BYTES => Environment::create_info
            },
            "module/env/remove" => { write REMOVE => Environment::remove },
            "module/env/remove/info" => {
                read BYTES => Environment::remove_info
            },
            "env/CWD" => { read BYTES => Environment::cwd_read,
                           write BYTES => Environment::cwd_write },
            "env/CWD/info" => { read BYTES => Environment::value_info },
            "module/env/info" => { read BYTES => Environment::module_info },
        },
    }
}

fn value_provider(
    value: Value,
    feed: Rc<RefCell<watch::Feed>>,
) -> reovim_kernel_interface::provider::ResourceProvider {
    use reovim_kernel_interface::{
        Contract,
        provider::{ResourceOps, ResourceProvider, Surface},
    };
    ResourceProvider::new(
        (value, feed),
        ResourceOps {
            unload: None,
            read: |(value, _), _, input| {
                Box::pin(ready(info(input, &value.borrow())))
            },
            write: |(value, feed), _, input| {
                Box::pin(ready(replace(value, feed, input)))
            },
        },
        Box::new([Surface {
            contract: Contract::new("resource.bytes", 1),
            methods: Permissions::ALL,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
}

fn replace(
    value: &Value,
    feed: &Rc<RefCell<watch::Feed>>,
    input: Message,
) -> Result<Message, Error> {
    feed.borrow().prepare_change()?;
    let reply = Message::new(input.contract, &[]);
    *value.borrow_mut() = input.data.into_vec();
    watch::changed(feed);
    Ok(reply)
}

impl Environment {
    fn observe(
        &self,
        _: ResourceId,
        input: Message,
    ) -> Ready<Result<Message, RequestError>> {
        ready(self.subscribe(input))
    }
    fn subscribe(&self, input: Message) -> Result<Message, RequestError> {
        if !input.data.is_empty() {
            return Err(Error::INVALID.into());
        }
        let parent = self.kernel.resolve("module/env/watch").into_inner()?;
        let mut feed = self.feed.borrow_mut();
        feed.prepare_reader()?;
        let reader = watch::Reader::new(feed.revision);
        let data = snapshot(&self.values.borrow(), feed.revision)?;
        let mut reply = Message::reference(input.contract, ResourceId(0));
        reply.data = data.into_boxed_slice();
        let name = alloc::format!("reader-{}", feed.serial);
        let id = self
            .kernel
            .publish(parent, &name, watch::provider(reader.clone()))
            .map_err(|(error, _)| error)?;
        feed.subscribe(&reader);
        reply.resources[0] = id;
        Ok(reply)
    }
}
fn snapshot(values: &Values, revision: u64) -> Result<Vec<u8>, Error> {
    let mut ordered: Vec<_> = values.iter().collect();
    ordered.sort_unstable_by_key(|(name, _)| name.as_ref());
    let mut data = revision.to_le_bytes().to_vec();
    for (name, value) in ordered {
        append(&mut data, name.as_bytes())?;
        append(&mut data, &value.borrow())?;
    }
    Ok(data)
}
fn append(data: &mut Vec<u8>, value: &[u8]) -> Result<(), Error> {
    const SNAPSHOT_BYTES: usize = 16 * 1024;
    let available =
        SNAPSHOT_BYTES.checked_sub(data.len() + 4).ok_or(Error::PRESSURE)?;
    if value.len() > available {
        return Err(Error::PRESSURE);
    }
    let len = u32::try_from(value.len()).map_err(|_| Error::PRESSURE)?;
    data.try_reserve(4 + value.len()).map_err(|_| Error::PRESSURE)?;
    data.extend_from_slice(&len.to_le_bytes());
    data.extend_from_slice(value);
    Ok(())
}
