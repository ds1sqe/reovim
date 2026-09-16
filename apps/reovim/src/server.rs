//! `reovim server`: the Kernel and every linked module on this thread,
//! one io thread per link that moves bytes and nothing else.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

use reovim_host::{clock, link};
use reovim_kernel_interface::{Booted, Error};
use reovim_kernel_runtime::{Connection, Kernel, Limits, Loader};

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

/// The Kernel with every linked module loaded over `root`. The modules
/// live as long as the Kernel does.
pub fn boot(root: &str) -> Result<(Rc<Kernel>, Vec<Booted>), Fail> {
    let kernel = Kernel::new(clock::now, Limits::default());
    let mut booted = Vec::new();
    for (name, boot) in MODULES {
        let module = boot(root);
        kernel.load(name, module.module.clone(), module.drivers.clone())?;
        booted.push(module);
    }
    Ok((Rc::new(kernel), booted))
}

/// `load` and `unload` on a link: `.rvm` files through kernel/abi; the
/// Modules of the `modules` table are the binary's and stay.
struct Files;

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
}

impl Links {
    /// Listen at `socket`.
    pub fn listen(socket: &str) -> Result<Self, Fail> {
        let listener = link::listen(socket)?;
        let (sender, inbound) = channel();
        thread::spawn(move || accept_loop(&listener, &sender));
        Ok(Self { inbound, connections: BTreeMap::new(), files: Files })
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
pub fn run(socket: &str, root: &str, log: Option<&str>) -> Result<(), Fail> {
    let (kernel, _modules) = boot(root)?;
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
