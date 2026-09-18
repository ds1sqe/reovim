//! `reovim server`: the Kernel and every linked module on this thread,
//! one io thread per link that moves bytes and nothing else.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

use reovim_host::{clock, fs, link, mem};
use reovim_kernel_interface::{Booted, Error, Op, Out, Reply, Rights};
use reovim_kernel_runtime::{Connection, Heap, Kernel, Limits, Loader, ROOT};

use crate::Fail;
use crate::link::MODULES;
use crate::log::Sink;

/// One message from a link, with where its answer goes. `bytes` is
/// `None` when the link ended.
struct Inbound {
    link: u64,
    bytes: Option<Vec<u8>>,
    answer: Sender<Option<Vec<u8>>>,
}

/// The process heap as the host counts it, in the Kernel's own shape.
/// The Kernel owns no allocator, so it takes this reading as it takes
/// the clock.
fn heap() -> Heap {
    let counted = mem::heap();
    Heap { live: counted.live, peak: counted.peak, allocs: counted.allocs }
}

/// The Kernel with every linked module loaded over `root`. The modules
/// live as long as the Kernel does.
///
/// `sys` are the `--sys NAME=VALUE` of the command line, each written
/// to `/sys/NAME` before the first Module is there to see it, as the
/// kernel command line's `loglevel=7` is set before init runs. A write
/// the Kernel refuses refuses the boot.
pub fn boot(
    root: &str,
    sys: &[(String, String)],
) -> Result<(Rc<Kernel>, Vec<Booted>), Fail> {
    let kernel = Kernel::new(clock::now, heap, Limits::default());
    for (name, value) in sys {
        set(&kernel, name, value)
            .map_err(|error| Fail::Sys(name.clone(), value.clone(), error))?;
    }
    let mut booted = Vec::new();
    for (name, boot) in MODULES {
        let module = boot(root);
        kernel.load(name, module.module.clone(), module.drivers.clone())?;
        booted.push(module);
    }
    Ok((Rc::new(kernel), booted))
}

/// `/sys/<name>` written `value`, on a caller of the boot's own that
/// ends with the write.
fn set(kernel: &Kernel, name: &str, value: &str) -> Result<(), Error> {
    let caller = kernel.caller_new();
    let written = written(kernel, caller, name, value);
    kernel.caller_end(caller);
    written
}

fn written(
    kernel: &Kernel,
    caller: reovim_kernel_interface::Caller,
    name: &str,
    value: &str,
) -> Result<(), Error> {
    let seq = std::cell::Cell::new(0);
    let step = |op: Op| -> Result<Out, Error> {
        seq.set(seq.get() + 1);
        match kernel.submit(caller, seq.get(), op) {
            Reply::Ready(result) => result,
            Reply::Pending(_) => Err(Error::Unsupported),
        }
    };
    let mut rd = ROOT;
    for part in format!("sys/{name}").split('/') {
        let name = part.to_string();
        let Out::Rd(next) = step(Op::Lookup { rd, name })? else {
            return Err(Error::Invalid);
        };
        rd = next;
    }
    let Out::Handle(h) = step(Op::Open { rd, rights: Rights::WRITE })? else {
        return Err(Error::Invalid);
    };
    let bytes = value.as_bytes().to_vec();
    step(Op::Write { h, at: 0, bytes })?;
    step(Op::Close { h })?;
    Ok(())
}

/// `load` and `unload`: `.rvm` files through kernel/abi; the Modules of
/// the `modules` table are the binary's and stay. The socket server
/// serves it, and so does the Client inside the bare editor, so a load
/// reaches the same ABI either way.
pub struct Files;

impl Loader for Files {
    fn load(&mut self, kernel: &Kernel, path: &str) -> Result<(), Error> {
        let (name, booted) = reovim_kernel_abi::load(path)?;
        kernel.load(&name, booted.module, booted.drivers).map(|_| ())
    }

    fn unload(&mut self, kernel: &Kernel, name: &str) -> Result<(), Error> {
        if MODULES.iter().any(|(linked, _)| *linked == name) {
            return Err(Error::Denied);
        }
        kernel.unload(name)
    }
}

/// The links: an acceptor thread, an io thread per link, and on this
/// thread one `Connection` per link.
pub struct Links {
    inbound: Receiver<Inbound>,
    connections: BTreeMap<u64, Connection>,
    files: Files,
    socket: String,
}

/// The socket file is this process's while the links live, so it goes
/// with them and the name is free again. A killed process leaves one
/// behind; the next [`Links::listen`] takes it over.
impl Drop for Links {
    fn drop(&mut self) {
        let _ = fs::remove(&self.socket);
    }
}

impl Links {
    /// Listen at `socket`, replacing a socket file left by an earlier
    /// run.
    pub fn listen(socket: &str) -> Result<Self, Fail> {
        let listener = link::listen(socket)?;
        let (sender, inbound) = channel();
        thread::spawn(move || accept_loop(&listener, &sender));
        let socket = socket.to_string();
        Ok(Self { inbound, connections: BTreeMap::new(), files: Files, socket })
    }

    /// Serve one message if one arrives within `wait`, forever with
    /// `None`. True when something was served.
    pub fn step(&mut self, kernel: &Kernel, wait: Option<Duration>) -> bool {
        let item = match wait {
            None => self.inbound.recv().ok(),
            Some(wait) => self.inbound.recv_timeout(wait).ok(),
        };
        let Some(item) = item else { return false };
        let Some(bytes) = item.bytes else {
            self.ended(kernel, item.link);
            return true;
        };
        let end = self.connections.entry(item.link).or_default();
        let _ = item.answer.send(end.handle(kernel, &bytes, &mut self.files));
        true
    }

    /// Serve every message already waiting, then return.
    pub fn drain(&mut self, kernel: &Kernel) {
        while self.step(kernel, Some(Duration::ZERO)) {}
    }
}

impl Links {
    /// The link is gone: its connection ends.
    fn ended(&mut self, kernel: &Kernel, link: u64) {
        if let Some(mut end) = self.connections.remove(&link) {
            end.end(kernel);
        }
    }
}

/// Serve `socket` from the directory `root` until killed; with `log`,
/// every log entry goes to that file too.
pub fn run(
    socket: &str,
    root: &str,
    log: Option<&str>,
    sys: &[(String, String)],
) -> Result<(), Fail> {
    let (kernel, _modules) = boot(root, sys)?;
    crate::dump::watch(&kernel);
    let mut links = Links::listen(socket)?;
    let mut sink = log.map(|path| Sink::open(&kernel, path)).transpose()?;
    eprintln!("reovim server: {socket} root {root}");
    loop {
        links.step(&kernel, None);
        if let Some(sink) = &mut sink {
            sink.step()?;
        }
    }
}

fn accept_loop(listener: &link::Listener, inbound: &Sender<Inbound>) {
    let mut next = 1;
    while let Ok(end) = link::accept(listener) {
        let id = next;
        next += 1;
        let inbound = inbound.clone();
        thread::spawn(move || io_loop(id, &end, &inbound));
    }
}

/// Move messages in and answers out until the link or the server ends.
fn io_loop(id: u64, end: &link::Link, inbound: &Sender<Inbound>) {
    let (answer, answers) = channel();
    while let Ok(bytes) = link::recv(end) {
        let item =
            Inbound { link: id, bytes: Some(bytes), answer: answer.clone() };
        if inbound.send(item).is_err() {
            return;
        }
        match answers.recv() {
            Ok(Some(reply)) if link::send(end, &reply).is_err() => break,
            Ok(_) => {}
            Err(_) => return,
        }
    }
    let _ = inbound.send(Inbound { link: id, bytes: None, answer });
}
