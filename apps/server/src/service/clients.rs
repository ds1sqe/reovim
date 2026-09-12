//! Retained local clients select their own namespace and native loader.

use alloc::{collections::VecDeque, format, string::String, vec, vec::Vec};
use core::task::Context;
use reovim_kernel_interface::{
    Connection, Error, ResourceId,
    provider::{Contribution, Permissions},
};
use reovim_kernel_runtime::Runtime;
use std::io::{self, Read};

use super::{pending::Response, requests::Requests};
use crate::{
    boot::{Modules, Scope},
    loading::Loader,
};

struct Client {
    token: String,
    kernel: Connection,
    loader: Loader,
    requests: Requests,
}

pub(super) struct Clients {
    _assembly: Connection,
    values: Vec<Client>,
    shared: Vec<(&'static str, ResourceId)>,
    next: u64,
    closed: VecDeque<String>,
    exports: Option<reovim_host_reference::export::Directory>,
    modules: Modules,
}

impl Clients {
    pub const fn new(
        owner: Connection,
        exports: Option<reovim_host_reference::export::Directory>,
        shared: Vec<(&'static str, ResourceId)>,
        modules: Modules,
    ) -> Self {
        Self {
            _assembly: owner,
            exports,
            modules,
            values: Vec::new(),
            shared,
            next: 0,
            closed: VecDeque::new(),
        }
    }

    pub fn dispatch(
        &mut self,
        runtime: &mut Runtime,
        line: &str,
    ) -> Result<Response, String> {
        if line == "v1\topen" {
            return self.open(runtime).map_err(|e| format!("{e}"));
        }
        let rest =
            line.strip_prefix("v1\t").ok_or("invalid service profile")?;
        let (token, request) = rest.split_once('\t').ok_or("missing client")?;
        if request == "disconnect" {
            return self.disconnect(runtime, token);
        }
        let client = self
            .values
            .iter_mut()
            .find(|client| client.token == token)
            .ok_or("unavailable client")?;
        client.dispatch(request)
    }

    fn disconnect(
        &mut self,
        runtime: &mut Runtime,
        token: &str,
    ) -> Result<Response, String> {
        if self.closed.iter().any(|closed| closed == token) {
            return Ok(ready(Ok("ok".into())));
        }
        let index = self
            .values
            .iter()
            .position(|client| client.token == token)
            .ok_or("unavailable client")?;
        self.closed.try_reserve(1).map_err(|_| "client capacity")?;
        let client = &self.values[index];
        runtime
            .close_owned_access(client.loader.binding())
            .map_err(|error| format!("{error}"))?;
        if !client.requests.is_empty() || client.loader.is_pending() {
            return Err(
                "client busy; recover and release outstanding requests".into(),
            );
        }
        runtime
            .retire_client(client.loader.binding())
            .map_err(|error| format!("client cleanup: {error}"))?;
        let client = self.values.remove(index);
        if self.closed.len() == 16 {
            self.closed.pop_front();
        }
        self.closed.push_back(client.token);
        Ok(ready(Ok("ok".into())))
    }

    fn open(&mut self, runtime: &mut Runtime) -> io::Result<Response> {
        if self.values.len() == 16 {
            return Err(io::Error::other(Error::PRESSURE));
        }
        self.values
            .try_reserve(1)
            .map_err(|_| io::Error::other(Error::PRESSURE))?;
        let token = token()?;
        if self.values.iter().any(|client| client.token == token)
            || self.closed.iter().any(|closed| closed == &token)
        {
            return Err(io::Error::other(Error::PRESSURE));
        }
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| io::Error::other(Error::PRESSURE))?;
        let output = self
            .exports
            .as_ref()
            .map(|directory| {
                directory.private_provider(&format!("client-{}", self.next))
            })
            .transpose()?;
        let prefix = format!("clients/{}", self.next);
        let log = runtime
            .register(
                &format!("{prefix}/kernel/log"),
                runtime.diagnostic_resource(),
            )
            .map_err(|(error, _)| io::Error::other(error))?;
        let client =
            construct(runtime, &prefix, &self.shared, token.clone(), output);
        let mut client = match client {
            Ok(client) => client,
            Err(error) => {
                runtime
                    .unregister_contributions(&[log])
                    .map_err(io::Error::other)?;
                return Err(io::Error::other(error));
            }
        };
        // The receipt owns startup even if its socket observer disappears.
        let startup = self.modules.prepare(
            runtime,
            client.loader.binding(),
            Scope::Client,
        );
        let response = client
            .requests
            .enqueue("boot", |id| boot_response(startup, token, id));
        self.values.push(client);
        response.map_err(io::Error::other)
    }

    pub fn advance(&mut self, runtime: &mut Runtime, wake: &mut Context<'_>) {
        for client in &mut self.values {
            client.loader.advance(runtime, &client.kernel, wake);
            client.requests.advance(wake);
        }
    }

    pub fn is_pending(&self) -> bool {
        self.values.iter().any(|client| client.loader.is_pending())
    }
}

fn boot_response(
    startup: Result<
        crate::boot::Startup,
        reovim_kernel_interface::RequestError,
    >,
    token: String,
    id: u64,
) -> Response {
    alloc::boxed::Box::pin(async move {
        let result = match startup {
            Ok(startup) => startup.await,
            Err(error) => Err(error),
        };
        result.map_err(|error| {
            format!("boot\t{token}\t{id}\t{}", super::failure(error))
        })?;
        Ok(format!("ok\t{token}\t{id}"))
    })
}

fn construct(
    runtime: &mut Runtime,
    prefix: &str,
    shared: &[(&str, ResourceId)],
    token: String,
    output: Option<reovim_kernel_interface::provider::ResourceProvider>,
) -> Result<Client, Error> {
    let root = runtime.resource(prefix)?;
    let mut grants = vec![(root, Permissions::ALL)];
    grants.extend(shared.iter().map(|(_, id)| (*id, Permissions::ALL)));
    let owner = runtime.bind_at(root, &grants)?;
    for (name, id) in shared {
        runtime.mount(&owner, name, *id)?;
    }
    let kernel = reovim_sdk::local(owner.connection());
    let loader = Loader::new(owner, root, grants);
    let mut values = vec![Contribution::Resource {
        path: format!("{prefix}/kernel/modules/load").into(),
        provider: loader.provider(),
    }];
    if let Some(provider) = output {
        values.push(Contribution::Resource {
            path: format!("{prefix}/host/files").into(),
            provider,
        });
    }
    runtime.register_contributions(values).map_err(|(error, _)| error)?;
    Ok(Client { token, kernel, loader, requests: Requests::default() })
}

pub(super) fn token() -> io::Result<String> {
    let mut bytes = [0_u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(reovim_service::hex(&bytes))
}

impl Client {
    fn dispatch(&mut self, request: &str) -> Result<Response, String> {
        if request == "reserve" {
            return Ok(ready(self.requests.reserve()));
        }
        if request == "requests" {
            return Ok(ready(Ok(self.requests.list())));
        }
        let mut fields = request.splitn(3, '\t');
        let action = fields.next().ok_or("missing request action")?;
        let id: u64 = fields
            .next()
            .ok_or("missing request identity")?
            .parse()
            .map_err(|_| "invalid request identity")?;
        match (action, fields.next()) {
            ("call", Some(input)) => {
                self.requests.call(id, input, &self.kernel)
            }
            ("result", None) => self.requests.result(id),
            ("status", None) => Ok(ready(self.requests.status(id))),
            ("input", None) => Ok(ready(self.requests.input(id))),
            ("release", None) => Ok(ready(self.requests.release(id))),
            _ => Err("invalid request action".into()),
        }
    }
}
fn ready(result: Result<String, String>) -> Response {
    alloc::boxed::Box::pin(core::future::ready(result))
}
