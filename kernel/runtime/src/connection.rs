//! One link's loop: message in, message out. No io, no threads; whoever
//! moves the bytes calls `handle` and sends back what it returns.

use alloc::vec::Vec;

use reovim_kernel_interface::wire::{Message, decode, encode};
use reovim_kernel_interface::{Caller, Error, Out, Rd, Reply};

use crate::kernel::Kernel;

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
    /// `Reply` sent this way, answer nothing.
    pub fn handle(&mut self, kernel: &Kernel, bytes: &[u8]) -> Option<Vec<u8>> {
        let (seq, message) = decode(bytes)?;
        let reply = match message {
            Message::Connect { session, .. } => {
                self.connect(kernel, seq, session)
            }
            Message::Request(op) => kernel.submit(self.caller, seq, op),
            Message::Reply(_) => return None,
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
