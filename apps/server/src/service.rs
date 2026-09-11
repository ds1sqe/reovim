//! Local control service with retained, isolated client namespaces.

mod clients;
mod pending;
mod requests;
mod session;
mod transport;

use alloc::{boxed::Box, format, string::String, vec::Vec};
use clients::Clients;
use pending::{Pending, Response};
use reovim_kernel_interface::{
    Connection, Contract, Message, RequestError, ResourceId, driver::Progress,
};
use reovim_kernel_runtime::{Limits, Runtime};
use reovim_service::{hex, unhex};
use std::{
    io,
    net::TcpListener,
    sync::Arc,
    task::{Context, Wake, Waker},
    thread,
    time::Duration,
};

/// Runs the experimental local control service until the process exits.
/// ```no_run
/// reovim_server::service::serve("127.0.0.1:7878")?;
/// # Ok::<(), std::io::Error>(())
/// ```
/// # Errors
/// Reports bind/startup errors. Individual connection failures are isolated.
pub fn serve(address: &str) -> io::Result<()> {
    let listener = TcpListener::bind(address)?;
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(io::Error::other("the control service requires loopback"));
    }
    let mut runtime =
        Runtime::new(1, Limits::default()).map_err(io::Error::other)?;
    runtime.start().map_err(io::Error::other)?;
    runtime
        .register_factory(
            "modules/catalog/counter",
            Contract::new("counter", 1),
            reovim_module_counter::factory(),
        )
        .map_err(|(error, _, _)| io::Error::other(error))?;
    runtime
        .register_factory(
            "modules/catalog/editing",
            Contract::new("editing", 1),
            reovim_module_editing::factory(),
        )
        .map_err(|(error, _, _)| io::Error::other(error))?;
    let modules = runtime.resource("modules").map_err(io::Error::other)?;
    let models =
        runtime.register_namespace("models").map_err(io::Error::other)?;
    let (owner, session) = session::create(&mut runtime)?;
    let mut clients = Clients::new(
        owner,
        alloc::vec![
            ("modules", modules),
            ("models", models),
            ("session", session),
        ],
    );
    std::println!("listening {}", listener.local_addr()?);
    let commands = transport::start(listener)?;
    let waker = Waker::from(Arc::new(OwnerWake(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut pending = Vec::new();
    loop {
        pending::receive(&commands, &mut runtime, &mut clients, &mut pending)?;
        advance(&mut runtime, &mut clients, &mut pending, &mut context)?;
    }
}

struct OwnerWake(thread::Thread);

impl Wake for OwnerWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn advance(
    runtime: &mut Runtime,
    clients: &mut Clients,
    pending: &mut Vec<Pending>,
    context: &mut Context<'_>,
) -> io::Result<()> {
    for _ in 0..64 {
        Pending::poll(pending, context);
        let progress = runtime.advance(context).map_err(io::Error::other)?;
        clients.advance(runtime, context);
        if progress != Progress::More && !clients.is_pending() {
            // unpark retains a token when a wake races this sleep.
            thread::park();
            return Ok(());
        }
    }
    // Lifecycle cleanup can still require retries without a provider wake.
    // Bound those turns and return to command admission between quanta.
    thread::park_timeout(Duration::from_millis(1));
    Ok(())
}

fn dispatch(line: &str, kernel: &Connection) -> Result<Response, String> {
    let fields: Vec<_> = line.split('\t').collect();
    match fields.as_slice() {
        ["close", path] => {
            let subject = resolve(kernel, path)?;
            kernel.close(subject).into_inner().map_err(failure)?;
            Ok(Box::pin(core::future::ready(Ok("ok".into()))))
        }
        ["ls", path] => {
            let subject = resolve(kernel, path)?;
            let entries =
                kernel.children(subject).into_inner().map_err(failure)?;
            let names: Vec<_> = entries
                .into_iter()
                .map(|e| format!("{}=@{}", e.name, e.resource.0))
                .collect();
            let reply = format!("ok\t{}", names.join("\t"));
            Ok(Box::pin(core::future::ready(Ok(reply))))
        }
        [method, path, contract, version, rest @ ..]
            if matches!(*method, "read" | "write") && rest.len() <= 2 =>
        {
            let mut input = Message::new(
                Contract::new(
                    contract,
                    version.parse().map_err(|_| "invalid contract version")?,
                ),
                &unhex(
                    rest.first()
                        .copied()
                        .filter(|value| *value != "-")
                        .unwrap_or(""),
                )
                .ok_or("invalid hex payload")?,
            );
            if let Some(references) = rest.get(1) {
                input.resources = references
                    .split(',')
                    .map(|reference| resolve(kernel, reference))
                    .collect::<Result<Vec<_>, _>>()?
                    .into();
            }
            call(kernel, method, path, &input)
        }
        _ => Err("expected v1 ls/read/write/close command".into()),
    }
}

fn resolve(kernel: &Connection, path: &str) -> Result<ResourceId, String> {
    if let Some(reference) = path.strip_prefix('@') {
        let (id, suffix) = reference.split_once('/').unwrap_or((reference, ""));
        let id = ResourceId(id.parse().map_err(|_| "invalid resource ID")?);
        if suffix.is_empty() {
            return Ok(id);
        }
        return kernel.resolve_at(id, suffix).into_inner().map_err(failure);
    }
    kernel.resolve(path).into_inner().map_err(failure)
}

fn call(
    kernel: &Connection,
    method: &str,
    path: &str,
    input: &Message,
) -> Result<Response, String> {
    let subject = resolve(kernel, path)?;
    let request = if method == "read" {
        kernel.read(subject, input)
    } else {
        kernel.write(subject, input)
    };
    Ok(Box::pin(async move {
        let reply = request.await.map_err(failure)?;
        Ok(encode(&reply))
    }))
}

fn encode(reply: &Message) -> String {
    let references: Vec<_> =
        reply.resources.iter().map(|id| format!("@{}", id.0)).collect();
    format!(
        "ok\t{}\t{}\t{}\t{}",
        reply.contract.name,
        reply.contract.version,
        hex(&reply.data),
        references.join(",")
    )
}

fn failure(error: RequestError) -> String {
    let (kind, error, recovery) = match error {
        RequestError::Rejected(error) => ("rejected", error, String::new()),
        RequestError::Failed(error) => ("failed", error, String::new()),
        RequestError::Unknown { cause, recovery } => {
            ("unknown", cause, hex(recovery.as_bytes()))
        }
    };
    let id = error.id.map_or_else(String::new, |id| format!("@{}", id.0));
    format!("{kind}\t{}\t{id}\t{recovery}", error.code.0)
}
