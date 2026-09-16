//! One link's loop: message in, message out. No io, no threads; whoever
//! moves the bytes calls `handle` and sends back what it returns.

use alloc::vec::Vec;

use reovim_kernel_interface::wire::{Message, decode, encode};
use reovim_kernel_interface::{Caller, Error, Out, Rd, Reply};

use crate::kernel::Kernel;

/// What `load <path>` and `unload <name>` do: the connection-level
/// commands of 02-client.md, served by whoever moves the bytes, since
/// only that side can open a file.
pub trait Loader {
    /// Load the Module in the file at `path` into `kernel`.
    ///
    /// # Errors
    /// `Invalid` for no such file or a refused descriptor,
    /// `Unsupported` where nothing can be loaded; whatever
    /// `Kernel::load` returns.
    fn load(&mut self, kernel: &Kernel, path: &str) -> Result<(), Error>;

    /// Unload the Module named `name`.
    ///
    /// # Errors
    /// `Denied` for a Module that is the binary's own; whatever
    /// `Kernel::unload` returns.
    fn unload(&mut self, kernel: &Kernel, name: &str) -> Result<(), Error>;
}

/// A connection that loads nothing: both commands are `Unsupported`.
pub struct NoLoader;

impl Loader for NoLoader {
    fn load(&mut self, _: &Kernel, _: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    fn unload(&mut self, _: &Kernel, _: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}

/// One connection: a caller, and once connected its Session and client
/// rd. The `Connect` sequence is kept so a repeated `Connect` after a
/// lost reply is answered again rather than refused.
pub struct Connection {
    caller: Caller,
    connected: Option<(u64, Rd, Rd)>,
}

impl Connection {
    /// A fresh caller on `kernel`.
    #[must_use]
    pub fn new(kernel: &Kernel) -> Self {
        Self { caller: kernel.caller_new(), connected: None }
    }

    /// The client rd once `Connect` was answered.
    #[must_use]
    pub fn client(&self) -> Option<Rd> {
        self.connected.map(|(_, _, client)| client)
    }

    /// One message in, its reply out. Bytes that are not a message, and a
    /// `Reply` sent this way, answer nothing. `load` and `unload` go to
    /// `loader` under the same reply window as a request.
    pub fn handle(
        &mut self,
        kernel: &Kernel,
        bytes: &[u8],
        loader: &mut dyn Loader,
    ) -> Option<Vec<u8>> {
        let (seq, message) = decode(bytes)?;
        let unit = |r: Result<(), Error>| r.map(|()| Out::Unit).into();
        let reply = match message {
            Message::Connect { session, .. } => {
                self.connect(kernel, seq, session)
            }
            Message::Request(op) => kernel.submit(self.caller, seq, op),
            Message::Reply(_) => return None,
            Message::Load { path } => kernel
                .once(self.caller, seq, || unit(loader.load(kernel, &path))),
            Message::Unload { name } => kernel
                .once(self.caller, seq, || unit(loader.unload(kernel, &name))),
        };
        Some(encode(seq, &Message::Reply(reply)))
    }

    fn connect(
        &mut self,
        kernel: &Kernel,
        seq: u64,
        session: Option<Rd>,
    ) -> Reply<Out> {
        match self.connected {
            Some((at, session, client)) if at == seq => {
                return Reply::ok(Out::Rds(alloc::vec![session, client]));
            }
            Some(_) => return Reply::err(Error::Invalid),
            None => {}
        }
        let session = session.unwrap_or_else(|| kernel.session_new());
        match kernel.client_new(session) {
            Ok(client) => {
                self.connected = Some((seq, session, client));
                Reply::ok(Out::Rds(alloc::vec![session, client]))
            }
            Err(error) => Reply::err(error),
        }
    }
}
