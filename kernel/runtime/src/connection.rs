//! One link's loop: message in, message out. No io, no threads; whoever
//! moves the bytes calls `handle` and sends back what it returns, and
//! `end` when the link is gone.

use alloc::rc::Rc;
use alloc::vec::Vec;

use reovim_kernel_interface::wire::{Message, REPLY, decode, encode, sequence};
use reovim_kernel_interface::{
    Caller, Error, Kind, ModuleId, Op, Out, Outcome, Rd, Reply, Token,
};

use crate::kernel::Kernel;
use crate::log::WARN;
use crate::proxy::Proxy;

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

/// One link's end of the wire.
///
/// Once `Connect` was answered it holds that link's caller, Session and
/// client rd, and the Driver that stands for its Client. The `Connect`
/// sequence is kept so a repeated `Connect` after a lost reply is
/// answered again rather than refused.
#[derive(Default)]
pub struct Connection {
    caller: Option<Caller>,
    connected: Option<(u64, Rd, Rd)>,
    proxy: Option<(Rc<Proxy>, ModuleId)>,
}

impl Connection {
    /// A link that has not connected yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { caller: None, connected: None, proxy: None }
    }

    /// The client rd once `Connect` was answered.
    #[must_use]
    pub fn client(&self) -> Option<Rd> {
        self.connected.map(|(_, _, client)| client)
    }

    /// One message in, its reply out. A `Reply` sent this way answers
    /// nothing; anything before `Connect` is `Invalid`. `load` and
    /// `unload` go to `loader` under the same reply window as a
    /// request. A frame `decode` refuses is `undecodable`'s.
    ///
    /// Three ops are the link's own and never reach dispatch:
    /// `Op::Done`, which finishes what this Client was asked to serve,
    /// `Op::Changed`, which announces an rd it serves, and `Op::Since`,
    /// whose answer carries what it is asked next. All run under the
    /// same window as the rest, so a sequence sent twice is answered
    /// from memory and moves `heard` all the same.
    pub fn handle(
        &mut self,
        kernel: &Kernel,
        bytes: &[u8],
        loader: &mut dyn Loader,
    ) -> Option<Vec<u8>> {
        let Some((seq, message)) = decode(bytes) else {
            return self.undecodable(kernel, bytes);
        };
        let unit = |r: Result<(), Error>| r.map(|()| Out::Unit).into();
        let reply = match (message, self.caller) {
            (Message::Connect { credential, session }, _) => {
                self.connect(kernel, seq, &credential, session)
            }
            (Message::Reply(_), _) => return None,
            (_, None) => Reply::err(Error::Invalid),
            (Message::Request(Op::Done { token, outcome }), Some(caller)) => {
                kernel
                    .once(caller, seq, || self.answered(kernel, token, outcome))
            }
            (Message::Request(Op::Changed { rd, kind }), Some(caller)) => {
                kernel.once(caller, seq, || self.changed(kernel, rd, kind))
            }
            (Message::Request(Op::Since { cursor }), Some(caller)) => kernel
                .once(caller, seq, || {
                    self.serving(
                        kernel,
                        kernel.run(caller, Op::Since { cursor }),
                    )
                }),
            (Message::Request(op), Some(caller)) => {
                kernel.submit(caller, seq, op)
            }
            (Message::Load { path }, Some(caller)) => {
                kernel.once(caller, seq, || unit(loader.load(kernel, &path)))
            }
            (Message::Unload { name }, Some(caller)) => {
                kernel.once(caller, seq, || unit(loader.unload(kernel, &name)))
            }
        };
        Some(encode(seq, &Message::Reply(reply)))
    }

    /// A frame `decode` refused: an `Endpoint` would offer it again
    /// for ever otherwise, no sequence ever retiring it.
    ///
    /// A sequence and a tag that is not `Reply`'s answers `Invalid`
    /// there, the same path as any call once connected. A torn
    /// `Reply` answers nothing — two ends answering replies would
    /// echo — nor does a frame with no sequence to address the
    /// answer at.
    fn undecodable(&self, kernel: &Kernel, bytes: &[u8]) -> Option<Vec<u8>> {
        let Some((seq, tag)) = sequence(bytes) else {
            return unanswered(
                kernel,
                format_args!("no sequence, len={}", bytes.len()),
            );
        };
        if tag == REPLY {
            return unanswered(kernel, format_args!("torn Reply, seq={seq}"));
        }
        let reply = self.caller.map_or_else(
            || Reply::err(Error::Invalid),
            |caller| kernel.once(caller, seq, || Reply::err(Error::Invalid)),
        );
        Some(encode(seq, &Message::Reply(reply)))
    }

    /// The link is gone; what the caller keeps is the Kernel's rule.
    ///
    /// The Driver that stood for its Client goes first: nobody is left
    /// to answer what it was asked, so its rds are `Gone` and every
    /// token on them `Cancelled` before the caller's own handles
    /// close.
    pub fn end(&mut self, kernel: &Kernel) {
        if let Some((_, id)) = self.proxy.take() {
            kernel.ended(id);
        }
        if let Some(caller) = self.caller.take() {
            kernel.caller_end(caller);
        }
        self.connected = None;
    }

    /// The Client's answer to an op it was asked to serve. Its rds and
    /// handles are its own; what the mirror makes of them is the
    /// proxy's, and the caller waiting at the Kernel is answered from
    /// there.
    fn answered(
        &self,
        kernel: &Kernel,
        token: Token,
        outcome: Outcome,
    ) -> Reply<Out> {
        let Some((proxy, id)) = &self.proxy else {
            return Reply::err(Error::Invalid);
        };
        proxy.answered(kernel, *id, token, outcome).map(|()| Out::Unit).into()
    }

    /// A change the Client announced on one of its own rds. It is
    /// always `Unit`: what the mirror can place becomes an event,
    /// retires the row for a `Gone` or unmarks its listing for a
    /// `Create`, and what it cannot is one `debug` line at the proxy,
    /// a Client being believed about nothing.
    fn changed(&self, kernel: &Kernel, rd: Rd, kind: Kind) -> Reply<Out> {
        if let Some((proxy, id)) = &self.proxy {
            proxy.changed(kernel, *id, rd, kind);
        }
        Reply::ok(Out::Unit)
    }

    /// The ops this Client is asked to serve, spliced into the answer
    /// to its `Op::Since`. It runs inside `Kernel::once`, so the reply
    /// window remembers the list with the reply: a lost reply asked
    /// again carries the same ops, and a sequence sent twice takes
    /// none out of the queue.
    fn serving(&self, kernel: &Kernel, reply: Reply<Out>) -> Reply<Out> {
        let Some((proxy, _)) = &self.proxy else { return reply };
        match reply {
            Reply::Ready(Ok(Out::Events { events, cursor, .. })) => {
                let serve = proxy.take_out(kernel);
                Reply::ok(Out::Events { events, cursor, serve })
            }
            other => other,
        }
    }

    fn connect(
        &mut self,
        kernel: &Kernel,
        seq: u64,
        credential: &[u8],
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
        match kernel.connect(session, credential) {
            Ok((caller, client)) => {
                self.caller = Some(caller);
                self.connected = Some((seq, session, client));
                self.proxy = Proxy::attach(kernel, client);
                Reply::ok(Out::Rds(alloc::vec![session, client]))
            }
            Err(error) => Reply::err(error),
        }
    }
}

/// One `warn` line for a frame `undecodable` answers nothing to.
fn unanswered(
    kernel: &Kernel,
    why: core::fmt::Arguments<'_>,
) -> Option<Vec<u8>> {
    kernel.say(WARN, || alloc::format!("undecodable frame, {why}"));
    None
}
