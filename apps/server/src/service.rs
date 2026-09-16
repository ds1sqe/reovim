//! Hosted framing and driving over retained Caller endpoints.

#[path = "service/transport.rs"]
mod transport;

use crate::{clients::Clients, loading::Loader};
use alloc::{string::String, sync::Arc};
use reovim_kernel_interface::{Error, driver::Progress};
use reovim_kernel_runtime::Runtime;
use reovim_service::protocol::{Frame, Reply};
use std::{
    io,
    net::TcpListener,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Wake, Waker},
    thread,
    time::Duration,
};

/// Serves already authorized Callers and drives configured loaders and Runtime.
///
/// Composition owns boot, credentials and grants. Stop ends socket admission
/// and joins transport workers; it does not discard Caller or loader custody.
/// The caller retains those owners to drain or retire them after this returns.
/// # Errors
/// Reports listener/driver failure; malformed frames and lost socket observers
/// remain local to their connections. Oversized result delivery closes its
/// socket without inventing an operation error or releasing the receipt.
pub fn serve(
    listener: TcpListener,
    runtime: &mut Runtime,
    clients: &Clients,
    loaders: &mut [Loader],
    stop: &AtomicBool,
) -> io::Result<()> {
    serve_mode(listener, runtime, clients, loaders, stop, false)
}

/// Serves until termination is requested and Runtime cleanup actually finishes.
///
/// For example, a process keeps original result/reconcile/release access alive
/// while closing; Busy and Incomplete do not terminate its listener.
/// # Errors
/// Reports transport or driver failure without claiming completed cleanup.
pub fn serve_closing(
    listener: TcpListener,
    runtime: &mut Runtime,
    clients: &Clients,
    loaders: &mut [Loader],
    stop: &AtomicBool,
) -> io::Result<()> {
    serve_mode(listener, runtime, clients, loaders, stop, true)
}

fn serve_mode(
    listener: TcpListener,
    runtime: &mut Runtime,
    clients: &Clients,
    loaders: &mut [Loader],
    stop: &AtomicBool,
    close_runtime: bool,
) -> io::Result<()> {
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the control service requires loopback",
        ));
    }
    let mut transport = transport::Transport::start(listener)?;
    let waker = Waker::from(Arc::new(OwnerWake(thread::current())));
    let mut context = Context::from_waker(&waker);
    let result = drive(
        &transport,
        runtime,
        clients,
        loaders,
        stop,
        close_runtime,
        &mut context,
    );
    let shutdown = transport.stop();
    result.and(shutdown)
}

fn drive(
    transport: &transport::Transport,
    runtime: &mut Runtime,
    clients: &Clients,
    loaders: &mut [Loader],
    stop: &AtomicBool,
    close_runtime: bool,
    context: &mut Context<'_>,
) -> io::Result<()> {
    loop {
        if stopping(runtime, loaders, stop, close_runtime)? {
            return Ok(());
        }
        let mut more = dispatch(transport, clients)? == transport::CAPACITY;
        for loader in &mut *loaders {
            more |= loader.advance(runtime, context) == Progress::More;
        }
        more |= runtime.advance(context).map_err(io::Error::other)?
            == Progress::More;
        if !more {
            // Wakes retain a token; the timeout also observes an external stop.
            thread::park_timeout(Duration::from_millis(100));
        }
    }
}

fn stopping(
    runtime: &mut Runtime,
    loaders: &[Loader],
    stop: &AtomicBool,
    close_runtime: bool,
) -> io::Result<bool> {
    if !stop.load(Ordering::Acquire) {
        return Ok(false);
    }
    if !close_runtime {
        return Ok(true);
    }
    for loader in loaders {
        loader.close();
    }
    match runtime.close() {
        Ok(()) => Ok(true),
        Err(Error::BUSY | Error::INCOMPLETE) => Ok(false),
        Err(error) => Err(io::Error::other(error)),
    }
}

fn dispatch(
    transport: &transport::Transport,
    clients: &Clients,
) -> io::Result<usize> {
    let mut received = 0;
    while received < transport::CAPACITY {
        let Some(command) = transport.next()? else { break };
        // Execute even after socket loss. Caller owns the admitted action.
        if let Ok(reply) = response(clients, &command.line) {
            let _ = command.reply.send(reply);
        }
        received += 1;
    }
    Ok(received)
}

fn response(clients: &Clients, line: &str) -> Result<String, Error> {
    let reply = match Frame::decode(line) {
        Ok(frame) => clients.dispatch(frame),
        Err(error) => Reply::Error(error),
    };
    reply.encode()
}

struct OwnerWake(thread::Thread);
impl Wake for OwnerWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

#[cfg(test)]
#[path = "service/tests.rs"]
mod tests;
