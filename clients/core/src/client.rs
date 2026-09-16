//! The Client over a `Carrier`: the ops, and the helpers above them.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_kernel_interface::wire::{Endpoint, Message};
use reovim_kernel_interface::{
    Decl, Error, Event, Handle, Hash, Info, Method, Op, Out, Rd, Reply, Rights,
    Token,
};

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
}

/// One connected Client: its Session, its client rd and its watch
/// cursor.
pub struct Client {
    carrier: Box<dyn Carrier>,
    endpoint: Endpoint,
    session: Rd,
    cid: Rd,
    cursor: u64,
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
        let endpoint = Endpoint::default();
        let mut client =
            Self { carrier, endpoint, session: ROOT, cid: ROOT, cursor: 0 };
        let connect = Message::Connect { credential, session: None };
        match client.exchange(&connect)? {
            Out::Rds(ids) if ids.len() == 2 => {
                client.session = ids[0];
                client.cid = ids[1];
                Ok(client)
            }
            _ => Err(Fault::Wire),
        }
    }

    /// `load <path>`: the Module in that file, on the server's side.
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

    /// One message out, the reply to it back; a Pending reply is asked
    /// until it is not Busy.
    fn exchange(&mut self, message: &Message) -> Result<Out, Fault> {
        let (_, bytes) = self.endpoint.send(message);
        let answer = self.carrier.exchange(&bytes)?;
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

    fn wait(&mut self, token: Token) -> Result<Out, Fault> {
        let ask = Message::Request(Op::Ask { token });
        let mut answer = self.exchange(&ask);
        while answer == Err(Fault::Kernel(Error::Busy)) {
            answer = self.exchange(&ask);
        }
        answer
    }

    fn request(&mut self, op: Op) -> Result<Out, Fault> {
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
            Out::Handle(h) => Ok(h),
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
        self.unit(Op::Close { h })
    }

    /// Events on the watched rds since the last call; the cursor moves.
    ///
    /// # Errors
    /// As [`Self::lookup`].
    pub fn since(&mut self) -> Result<Vec<Event>, Fault> {
        match self.request(Op::Since { cursor: self.cursor })? {
            Out::Events { events, cursor } => {
                self.cursor = cursor;
                Ok(events)
            }
            _ => Err(Fault::Wire),
        }
    }

    /// `/a/b/c` by `lookup` from the root, or `rd:<n>`.
    ///
    /// # Errors
    /// `Usage` for another shape; otherwise as [`Self::lookup`].
    pub fn resolve(&mut self, target: &str) -> Result<Rd, Fault> {
        let bad = || Fault::Usage(format!("not a path or rd:<n>: {target}"));
        if let Some(number) = target.strip_prefix("rd:") {
            return number.parse().map(Rd).map_err(|_| bad());
        }
        let path = target.strip_prefix('/').ok_or_else(bad)?;
        let mut rd = ROOT;
        for name in path.split('/').filter(|n| !n.is_empty()) {
            rd = self.lookup(rd, name)?;
        }
        Ok(rd)
    }

    /// Everything readable at `rd`, through a handle opened and closed
    /// here.
    ///
    /// # Errors
    /// As [`Self::lookup`].
    pub fn read_all(&mut self, rd: Rd) -> Result<Vec<u8>, Fault> {
        let h = self.open(rd, Rights::READ)?;
        let out = self.read_whole(h);
        self.close(h)?;
        out
    }

    /// Everything readable through `h`, from 0 until an empty `Read`.
    ///
    /// # Errors
    /// As [`Self::lookup`].
    pub fn read_whole(&mut self, h: Handle) -> Result<Vec<u8>, Fault> {
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

    /// The first method named `name` among what `rd` offers.
    ///
    /// # Errors
    /// `Usage` when there is none; otherwise as [`Self::decl`].
    pub fn method(
        &mut self,
        rd: Rd,
        name: &str,
    ) -> Result<(Hash, Method), Fault> {
        let named = |(hash, decl): (Hash, Decl)| {
            decl.methods.into_iter().find(|m| m.name == name).map(|m| (hash, m))
        };
        self.decls(rd)?.into_iter().find_map(named).ok_or_else(|| {
            Fault::Usage(format!("no method `{name}` on rd {}", rd.0))
        })
    }

    /// Run `method` on `rd` with `input` already encoded, through a
    /// handle opened with the method's right and closed here.
    ///
    /// # Errors
    /// As [`Self::lookup`].
    pub fn call_method(
        &mut self,
        rd: Rd,
        api: Hash,
        method: &Method,
        input: Vec<u8>,
    ) -> Result<Vec<u8>, Fault> {
        let h = self.open(rd, method.right)?;
        let output = self.call(h, api, method.number, input);
        self.close(h)?;
        output
    }

    /// [`Self::method`] then [`Self::call_method`].
    ///
    /// # Errors
    /// As both.
    pub fn call_by_name(
        &mut self,
        rd: Rd,
        name: &str,
        input: Vec<u8>,
    ) -> Result<Vec<u8>, Fault> {
        let (api, method) = self.method(rd, name)?;
        self.call_method(rd, api, &method, input)
    }

    /// The name of `rd` under its parent, empty for the root.
    ///
    /// # Errors
    /// As [`Self::info`].
    pub fn name(&mut self, rd: Rd) -> Result<String, Fault> {
        Ok(self.info(rd)?.name.unwrap_or_default())
    }
}

/// `Out::Unit` or a wrong shape.
const fn unit(out: &Out) -> Result<(), Fault> {
    match out {
        Out::Unit => Ok(()),
        _ => Err(Fault::Wire),
    }
}
