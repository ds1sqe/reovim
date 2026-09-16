use crate::requests::Client;
use reovim_kernel_interface::{
    Value,
    caller::Operation,
    declaration::{Declaration, Operation as Declared},
    resource::AccessId,
};
use reovim_service::protocol::{Action, Reply};
use std::io;

mod decode;
use decode::Reader;

pub struct Shell {
    client: Client,
}
impl Shell {
    pub const fn new(client: Client) -> Self {
        Self { client }
    }
    pub fn execute(&mut self, fields: &[&str]) -> io::Result<bool> {
        match fields {
            [] => {}
            ["exit"] => return Ok(false),
            ["retry"] => report(&self.client.retry()?)?,
            ["receipts"] => {
                println!("{:?}", self.client.receipts().collect::<Vec<_>>());
            }
            ["result", id] => self.control(Action::Result(number(id)?))?,
            ["adopt", id] => self.control(Action::Adopt(number(id)?))?,
            ["release", id] => self.control(Action::Release(number(id)?))?,
            ["cancel", id] => self.control(Action::Cancel(number(id)?))?,
            ["reconcile", id] => {
                self.control(Action::Reconcile(number(id)?))?;
            }
            ["close", id] => self.close(AccessId(number(id)?))?,
            ["ls", path] => self.list(path)?,
            ["inspect", path] => self.inspect(path)?,
            ["invoke", path, method, hex] => self.invoke(path, method, hex)?,
            _ => {
                return Err(io::Error::other(concat!(
                    "expected ls/inspect PATH; invoke PATH METHOD HEX; ",
                    "receipts/retry; result/adopt/release/cancel/",
                    "reconcile ID; close ACCESS; exit"
                )));
            }
        }
        Ok(true)
    }
    fn control(&mut self, action: Action) -> io::Result<()> {
        report(&self.client.exchange(action)?)
    }
    fn open(&mut self, path: &str) -> io::Result<AccessId> {
        let bytes = if let Some(identity) = path.strip_prefix('@') {
            let id = reovim_service::unhex(identity)
                .ok_or_else(|| io::Error::other("invalid identity"))?;
            let mut bytes = 1_u32.to_le_bytes().to_vec();
            bytes.extend(id);
            bytes
        } else {
            let mut bytes = 0_u32.to_le_bytes().to_vec();
            bytes.extend_from_slice(
                &u32::try_from(path.len())
                    .map_err(io::Error::other)?
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(path.as_bytes());
            bytes
        };
        let value = self.client.call(
            Operation::Acquire,
            AccessId(0),
            0,
            Value::copy(&bytes, &[]).map_err(io::Error::other)?,
        )?;
        reference(&value)
    }
    fn close(&mut self, access: AccessId) -> io::Result<()> {
        self.client.call(Operation::Close, access, 0, Value::unit())?;
        Ok(())
    }
    fn access(&mut self, access: AccessId, rights: u32) -> io::Result<()> {
        self.client.call(
            Operation::SetAccess,
            access,
            0,
            Value::copy(&rights.to_le_bytes(), &[])
                .map_err(io::Error::other)?,
        )?;
        Ok(())
    }
    fn interfaces(&mut self, access: AccessId) -> io::Result<Vec<Declaration>> {
        interfaces(&mut self.client, access)
    }
    fn inspect(&mut self, path: &str) -> io::Result<()> {
        let opened = self.open(path)?;
        let result = self.inspect_opened(opened);
        self.finish(opened, result)
    }
    fn inspect_opened(&mut self, opened: AccessId) -> io::Result<()> {
        for api in self.interfaces(opened)? {
            describe(&api);
        }
        Ok(())
    }
    fn list(&mut self, path: &str) -> io::Result<()> {
        let opened = self.open(path)?;
        let result = self
            .access(opened, 1)
            .and_then(|()| list(&mut self.client, opened));
        self.finish(opened, result)
    }
    fn invoke(&mut self, path: &str, name: &str, hex: &str) -> io::Result<()> {
        let data = if hex == "-" {
            Vec::new()
        } else {
            reovim_service::unhex(hex)
                .ok_or_else(|| io::Error::other("invalid input hex"))?
        };
        let opened = self.open(path)?;
        let result = self.invoke_opened(opened, name, &data);
        self.finish(opened, result)
    }
    fn invoke_opened(
        &mut self,
        opened: AccessId,
        name: &str,
        data: &[u8],
    ) -> io::Result<()> {
        let (api, method) = select(self.interfaces(opened)?, name)?;
        let method = &api.methods()[method];
        let mut encoded = u32::try_from(api.bytes().len())
            .map_err(io::Error::other)?
            .to_le_bytes()
            .to_vec();
        encoded.extend_from_slice(api.bytes());
        let bound = self.client.call(
            Operation::Bind,
            opened,
            0,
            Value::copy(&encoded, &[]).map_err(io::Error::other)?,
        )?;
        self.access(opened, method.access())?;
        let (operation, command) = match method.operation() {
            Declared::Exec => (Operation::Exec, 0),
            Declared::Ioctl(command) => (Operation::Ioctl, command),
        };
        let value = self.client.call(
            operation,
            reference(&bound)?,
            command,
            Value::copy(data, &[]).map_err(io::Error::other)?,
        )?;
        println!(
            "data {} refs {:?}",
            reovim_service::hex(&value.data),
            value.references
        );
        Ok(())
    }
    fn finish(
        &mut self,
        opened: AccessId,
        result: io::Result<()>,
    ) -> io::Result<()> {
        result.and_then(|()| self.close(opened)).map_err(|error| {
            io::Error::other(format!(
                "{error}; opened access {}: recover receipt before closing",
                opened.0
            ))
        })
    }
}
fn number(text: &str) -> io::Result<u64> {
    text.parse().map_err(io::Error::other)
}
fn reference(value: &Value) -> io::Result<AccessId> {
    value
        .references
        .first()
        .copied()
        .ok_or_else(|| io::Error::other("missing returned access"))
}
fn select(
    apis: Vec<Declaration>,
    name: &str,
) -> io::Result<(Declaration, usize)> {
    let mut selected = None;
    for api in apis {
        let Some(index) =
            api.methods().iter().position(|method| method.name() == name)
        else {
            continue;
        };
        if selected.is_some() {
            return Err(io::Error::other("ambiguous method name"));
        }
        selected = Some((api, index));
    }
    selected.ok_or_else(|| io::Error::other("method unavailable"))
}
fn describe(api: &Declaration) {
    for method in api.methods() {
        println!(
            "api {} method {} {:?} input {:?} output {:?}",
            reovim_service::hex(api.api()),
            method.name(),
            method.operation(),
            method.input(),
            method.output()
        );
    }
}
fn interfaces(
    client: &mut Client,
    access: AccessId,
) -> io::Result<Vec<Declaration>> {
    let mut cursor = 0;
    let mut apis = Vec::new();
    for _ in 0..256 {
        let value =
            client.call(Operation::Interfaces, access, 0, page(cursor)?)?;
        let mut reader = Reader::new(&value.data);
        let count = usize::try_from(reader.u32()?).map_err(io::Error::other)?;
        if count > 256 - apis.len() {
            return Err(io::Error::other("more than 256 interfaces"));
        }
        for _ in 0..count {
            apis.push(
                Declaration::decode(reader.bytes()?)
                    .map_err(io::Error::other)?,
            );
        }
        cursor = reader.u64()?;
        reader.end()?;
        if cursor == 0 {
            return Ok(apis);
        }
    }
    Err(io::Error::other("interface listing exceeds 256 pages"))
}
fn list(client: &mut Client, access: AccessId) -> io::Result<()> {
    let mut cursor = 0;
    for _ in 0..256 {
        let value =
            client.call(Operation::Entries, access, 0, page(cursor)?)?;
        let mut reader = Reader::new(&value.data);
        for _ in 0..reader.u32()? {
            let name = reader.bytes()?;
            let identity = reader.take(24)?;
            println!(
                "name {} identity @{}",
                reovim_service::hex(name),
                reovim_service::hex(identity)
            );
        }
        cursor = reader.u64()?;
        reader.end()?;
        if cursor == 0 {
            return Ok(());
        }
    }
    Err(io::Error::other("namespace listing exceeds 256 pages"))
}
fn page(cursor: u64) -> io::Result<Value> {
    let mut bytes = cursor.to_le_bytes().to_vec();
    bytes.extend_from_slice(&65536_u64.to_le_bytes());
    Value::copy(&bytes, &[]).map_err(io::Error::other)
}

fn report(reply: &Reply) -> io::Result<()> {
    if let Reply::Error(error) = reply {
        return Err(io::Error::other(format!("{error:?}")));
    }
    println!("{}", reply.encode().map_err(io::Error::other)?);
    Ok(())
}
