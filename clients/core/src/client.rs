//! The tree over a `Carrier`: the ops, and the helpers above them;
//! and the Client, which is a `Tree` and the watch cursor it drains.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_kernel_interface::wire::{Endpoint, Message};
use reovim_kernel_interface::{
    Decl, Error, Event, Handle, Hash, Info, Op, Out, Rd, Reply, Rights, Token,
};

use reovim_sdk::walk::Step;

use crate::Fault;

/// Known to every Client; everything else is `lookup`.
pub const ROOT: Rd = Rd(0);
/// Bytes asked per `Read` in `read_all`.
const CHUNK: u32 = 64 * 1024;

/// Moves one message out and brings the next reply in. The app gives a
/// link; a test or the integrated form an in-process `Connection`.
pub trait Carrier {
    /// # Errors
    /// `Carrier` when the other end is gone.
    fn exchange(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Fault>;

    /// A fresh link to the same end, after `Carrier`. None by default.
    ///
    /// # Errors
    /// `Carrier` when there is no way back.
    fn reconnect(&mut self) -> Result<(), Fault> {
        Err(Fault::Carrier)
    }

    /// Between two `Op::Ask` on one token: the asker gives the machine
    /// back. The default is nothing; a link that shares a thread with
    /// whoever would answer must not spin, and must not touch its own
    /// Client, whose wire is in use while it waits.
    fn pause(&mut self) {}
}

/// The tree a Client reaches: its credential, its Session and its
/// client rd, with the seven ops and the helpers above them.
///
/// This is what a `Plugin` is handed: everything the tree offers, and
/// nothing of the Client's own, the watch cursor above all, which one
/// reader drains for every Plugin at once.
pub struct Tree {
    carrier: Box<dyn Carrier>,
    endpoint: Endpoint,
    credential: Vec<u8>,
    session: Rd,
    cid: Rd,
    /// Opened here and not closed here: a mirror of the handles the
    /// Kernel holds for this caller, so whoever replaces a Plugin can
    /// close what it left behind.
    handles: Vec<Handle>,
}

/// One connected Client: a `Tree`, the watch cursor it drains, and
/// what the server has asked this Client to serve.
pub struct Client {
    tree: Tree,
    cursor: u64,
    /// The ops of the last `Since` answers that have not been run;
    /// `Host::serve` is the only thing that takes them.
    served: Vec<(Token, Op)>,
}

impl Client {
    /// The `Connect` exchange: a new Session, a client rd under it.
    ///
    /// # Errors
    /// `Kernel` when the Kernel refuses, `Carrier` or `Wire` from the
    /// exchange.
    pub fn connect(
        carrier: Box<dyn Carrier>,
        credential: Vec<u8>,
    ) -> Result<Self, Fault> {
        let tree = Tree {
            carrier,
            endpoint: Endpoint::default(),
            credential,
            session: ROOT,
            cid: ROOT,
            handles: Vec::new(),
        };
        let mut client = Self { tree, cursor: 0, served: Vec::new() };
        let connect = Message::Connect {
            credential: client.tree.credential.clone(),
            session: None,
        };
        let out = client.tree.exchange(&connect)?;
        client.tree.connected(&out)?;
        Ok(client)
    }

    /// The tree itself, for whoever makes calls on it.
    pub const fn tree(&mut self) -> &mut Tree {
        &mut self.tree
    }

    /// Events on the watched rds since the last call; the cursor moves
    /// and the ops the server asks this Client to serve are kept for
    /// [`crate::Host::serve`], which is the only thing that runs one.
    ///
    /// # Errors
    /// As [`Tree::lookup`].
    pub fn since(&mut self) -> Result<Vec<Event>, Fault> {
        match self.tree.request(Op::Since { cursor: self.cursor })? {
            Out::Events { events, cursor, serve } => {
                self.cursor = cursor;
                self.served.extend(serve);
                Ok(events)
            }
            _ => Err(Fault::Wire),
        }
    }

    /// What is left to serve, taken out.
    pub(crate) fn take_served(&mut self) -> Vec<(Token, Op)> {
        core::mem::take(&mut self.served)
    }
}

impl core::ops::Deref for Client {
    type Target = Tree;

    fn deref(&self) -> &Tree {
        &self.tree
    }
}

impl core::ops::DerefMut for Client {
    fn deref_mut(&mut self) -> &mut Tree {
        &mut self.tree
    }
}

impl Tree {
    /// The `Connect` answer: the Session and the client rd.
    fn connected(&mut self, out: &Out) -> Result<(), Fault> {
        match out {
            Out::Rds(ids) if ids.len() == 2 => {
                (self.session, self.cid) = (ids[0], ids[1]);
                Ok(())
            }
            _ => Err(Fault::Wire),
        }
    }

    /// `/session/<id>`.
    #[must_use]
    pub const fn session(&self) -> Rd {
        self.session
    }

    /// `/session/<id>/client/<cid>`.
    #[must_use]
    pub const fn cid(&self) -> Rd {
        self.cid
    }

    /// The handles open on this caller, in the order they were
    /// opened: exactly what `close` has not been called on.
    #[must_use]
    pub fn handles(&self) -> &[Handle] {
        &self.handles
    }

    /// `load <path>`: the Module in that file, on the server's side.
    /// Not an op but a message of its own, as `unload` is.
    ///
    /// # Errors
    /// What the server says; `Unsupported` where nothing can be loaded.
    pub fn load(&mut self, path: &str) -> Result<(), Fault> {
        let load = Message::Load { path: path.into() };
        unit(&self.exchange(&load)?)
    }

    /// `unload <name>`.
    ///
    /// # Errors
    /// What the server says; `Busy` while a handle on the Module is
    /// open.
    pub fn unload(&mut self, name: &str) -> Result<(), Fault> {
        let unload = Message::Unload { name: name.into() };
        unit(&self.exchange(&unload)?)
    }

    /// One message out, the reply to it back; a Pending reply is asked
    /// until it is not Busy.
    fn exchange(&mut self, message: &Message) -> Result<Out, Fault> {
        let (seq, bytes) = self.endpoint.send(message);
        let answer = match self.carrier.exchange(&bytes) {
            Err(Fault::Carrier) => self.recover(seq)?,
            other => other?,
        };
        let Some((_, Message::Reply(reply))) = self.endpoint.receive(&answer)
        else {
            return Err(Fault::Wire);
        };
        match reply {
            Reply::Ready(Ok(out)) => Ok(out),
            Reply::Ready(Err(error)) => Err(Fault::Kernel(error)),
            Reply::Pending(token) => self.wait(token),
        }
    }

    /// The link is gone: a fresh one, `Connect` again with the same
    /// credential into the same Session, then everything unanswered
    /// again in order. The answer to `seq` comes back raw.
    fn recover(&mut self, seq: u64) -> Result<Vec<u8>, Fault> {
        self.carrier.reconnect()?;
        let connect = Message::Connect {
            credential: self.credential.clone(),
            session: Some(self.session),
        };
        let (_, bytes) = self.endpoint.send(&connect);
        let answer = self.carrier.exchange(&bytes)?;
        match self.endpoint.receive(&answer) {
            Some((_, Message::Reply(Reply::Ready(Ok(out))))) => {
                self.connected(&out)?;
            }
            _ => return Err(Fault::Carrier),
        }
        resend(&mut self.endpoint, self.carrier.as_mut(), seq)
    }

    /// A token asked until it is not Busy. It never serves: an op of
    /// this Client's own may not run while the wire is in use, so a
    /// Client must never wait on an answer that needs itself.
    fn wait(&mut self, token: Token) -> Result<Out, Fault> {
        let ask = Message::Request(Op::Ask { token });
        let mut answer = self.exchange(&ask);
        while answer == Err(Fault::Kernel(Error::Busy)) {
            self.carrier.pause();
            answer = self.exchange(&ask);
        }
        answer
    }

    pub(crate) fn request(&mut self, op: Op) -> Result<Out, Fault> {
        self.exchange(&Message::Request(op))
    }

    fn rd(&mut self, op: Op) -> Result<Rd, Fault> {
        match self.request(op)? {
            Out::Rd(rd) => Ok(rd),
            _ => Err(Fault::Wire),
        }
    }

    fn bytes(&mut self, op: Op) -> Result<Vec<u8>, Fault> {
        match self.request(op)? {
            Out::Bytes(bytes) => Ok(bytes),
            _ => Err(Fault::Wire),
        }
    }

    fn unit(&mut self, op: Op) -> Result<(), Fault> {
        match self.request(op)? {
            Out::Unit => Ok(()),
            _ => Err(Fault::Wire),
        }
    }

    /// # Errors
    /// `Kernel` when refused; `Carrier` or `Wire` from the exchange. So
    /// for every op below.
    pub fn lookup(&mut self, rd: Rd, name: &str) -> Result<Rd, Fault> {
        self.rd(Op::Lookup { rd, name: name.to_string() })
    }

    /// # Errors
    /// As [`Self::lookup`].
    pub fn info(&mut self, rd: Rd) -> Result<Info, Fault> {
        match self.request(Op::Info { rd })? {
            Out::Info(info) => Ok(info),
            _ => Err(Fault::Wire),
        }
    }

    /// # Errors
    /// As [`Self::lookup`].
    pub fn parent(&mut self, rd: Rd) -> Result<Rd, Fault> {
        self.rd(Op::Parent { rd })
    }

    /// # Errors
    /// As [`Self::lookup`].
    pub fn children(&mut self, rd: Rd) -> Result<Vec<Rd>, Fault> {
        match self.request(Op::Children { rd })? {
            Out::Rds(rds) => Ok(rds),
            _ => Err(Fault::Wire),
        }
    }

    /// # Errors
    /// As [`Self::lookup`].
    pub fn open(&mut self, rd: Rd, rights: Rights) -> Result<Handle, Fault> {
        match self.request(Op::Open { rd, rights })? {
            Out::Handle(h) => {
                self.handles.push(h);
                Ok(h)
            }
            _ => Err(Fault::Wire),
        }
    }

    /// # Errors
    /// As [`Self::lookup`].
    pub fn read(
        &mut self,
        h: Handle,
        at: u64,
        len: u32,
    ) -> Result<Vec<u8>, Fault> {
        self.bytes(Op::Read { h, at, len })
    }

    /// # Errors
    /// As [`Self::lookup`].
    pub fn write(
        &mut self,
        h: Handle,
        at: u64,
        bytes: Vec<u8>,
    ) -> Result<u32, Fault> {
        match self.request(Op::Write { h, at, bytes })? {
            Out::Len(len) => Ok(len),
            _ => Err(Fault::Wire),
        }
    }

    /// # Errors
    /// As [`Self::lookup`].
    pub fn call(
        &mut self,
        h: Handle,
        api: Hash,
        method: u16,
        input: Vec<u8>,
    ) -> Result<Vec<u8>, Fault> {
        self.bytes(Op::Call { h, api, method, input })
    }

    /// # Errors
    /// As [`Self::lookup`].
    pub fn watch(&mut self, h: Handle, on: bool) -> Result<(), Fault> {
        self.unit(Op::Watch { h, on })
    }

    /// # Errors
    /// As [`Self::lookup`].
    pub fn close(&mut self, h: Handle) -> Result<(), Fault> {
        // The Kernel drops the handle whatever it answers, and what it
        // refuses was never this caller's; either way it is not held
        // here any more.
        self.handles.retain(|open| *open != h);
        self.unit(Op::Close { h })
    }

    /// The rd `target` names, walked from the root: a path from `/`, a
    /// name under it, or `rd:<n>`, all through the one
    /// [`reovim_sdk::walk::walk`].
    ///
    /// # Errors
    /// As [`Self::lookup`], for the segment that stopped the walk.
    pub fn resolve(&mut self, target: &str) -> Result<Rd, Fault> {
        reovim_sdk::walk::walk(ROOT, target, |step| match step {
            Step::Child { rd, name } => self.lookup(rd, name),
            Step::Parent { rd } => self.parent(rd),
        })
    }

    /// Everything readable at `rd`, through a handle opened and closed
    /// here.
    ///
    /// # Errors
    /// As [`Self::lookup`].
    pub fn read_all(&mut self, rd: Rd) -> Result<Vec<u8>, Fault> {
        let h = self.open(rd, Rights::READ)?;
        let out = self.whole(h);
        self.close(h)?;
        out
    }

    /// Everything readable through `h`, from 0 until an empty `Read`.
    fn whole(&mut self, h: Handle) -> Result<Vec<u8>, Fault> {
        let mut out = Vec::new();
        let mut chunk = self.read(h, 0, CHUNK)?;
        while !chunk.is_empty() {
            out.extend_from_slice(&chunk);
            chunk = self.read(h, out.len() as u64, CHUNK)?;
        }
        Ok(out)
    }

    /// `bytes` at 0, through a handle opened and closed here.
    ///
    /// # Errors
    /// As [`Self::lookup`].
    pub fn write_all(&mut self, rd: Rd, bytes: Vec<u8>) -> Result<u32, Fault> {
        let h = self.open(rd, Rights::WRITE)?;
        let len = self.write(h, 0, bytes)?;
        self.close(h)?;
        Ok(len)
    }

    /// The declaration at `/sys/decls/<hash>`.
    ///
    /// # Errors
    /// As [`Self::lookup`]; `Kernel(Invalid)` for bytes that are not one.
    pub fn decl(&mut self, hash: Hash) -> Result<Decl, Fault> {
        let name = format!("{:016x}", hash.0);
        let rd = self.resolve(&format!("/sys/decls/{name}"))?;
        Ok(Decl::parse(&self.read_all(rd)?)?)
    }

    /// Every declaration `rd` offers, with its hash.
    ///
    /// # Errors
    /// As [`Self::decl`].
    pub fn decls(&mut self, rd: Rd) -> Result<Vec<(Hash, Decl)>, Fault> {
        let mut decls = Vec::new();
        for hash in self.info(rd)?.apis {
            decls.push((hash, self.decl(hash)?));
        }
        Ok(decls)
    }

    /// The first method named `name` among what `rd` offers, run with
    /// `input` already encoded, through a handle opened with the
    /// method's right and closed here.
    ///
    /// # Errors
    /// `Unsupported` when no api of `rd` declares that name; otherwise
    /// as [`Self::decl`].
    pub fn call_by_name(
        &mut self,
        rd: Rd,
        name: &str,
        input: Vec<u8>,
    ) -> Result<Vec<u8>, Fault> {
        let named = |(hash, decl): (Hash, Decl)| {
            decl.methods.into_iter().find(|m| m.name == name).map(|m| (hash, m))
        };
        let found = self.decls(rd)?.into_iter().find_map(named);
        let (api, method) = found.ok_or(Fault::Kernel(Error::Unsupported))?;
        let h = self.open(rd, method.right)?;
        let output = self.call(h, api, method.number, input);
        self.close(h)?;
        output
    }

    /// The name of `rd` under its parent, empty for the root.
    ///
    /// # Errors
    /// As [`Self::info`].
    pub fn name(&mut self, rd: Rd) -> Result<String, Fault> {
        Ok(self.info(rd)?.name.unwrap_or_default())
    }
}

/// Everything unanswered again, in order; the answer to `seq` raw, the
/// others into the `Endpoint`.
fn resend(
    endpoint: &mut Endpoint,
    carrier: &mut dyn Carrier,
    seq: u64,
) -> Result<Vec<u8>, Fault> {
    let mut ours = None;
    for (at, bytes) in endpoint.unanswered().to_vec() {
        let answer = carrier.exchange(&bytes)?;
        if at == seq {
            ours = Some(answer);
        } else {
            endpoint.receive(&answer);
        }
    }
    ours.ok_or(Fault::Carrier)
}

/// `Out::Unit` or a wrong shape.
const fn unit(out: &Out) -> Result<(), Fault> {
    match out {
        Out::Unit => Ok(()),
        _ => Err(Fault::Wire),
    }
}

/// A `declare!`d api's caller side runs on the tree.
impl reovim_sdk::Call for Tree {
    type Error = Fault;

    fn call(
        &mut self,
        h: Handle,
        api: Hash,
        number: u16,
        input: Vec<u8>,
    ) -> Result<Vec<u8>, Fault> {
        Self::call(self, h, api, number, input)
    }
}
