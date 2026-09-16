//! `reovim server`: the Kernel and every linked module on this thread,
//! one io thread per link that moves bytes and nothing else.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

use reovim_host::{clock, link};
use reovim_kernel_interface::Booted;
use reovim_kernel_runtime::{Connection, Kernel, Limits};

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

/// The links: an acceptor thread, an io thread per link, and on this
/// thread one `Connection` per link.
pub struct Links {
    from_links: Receiver<Inbound>,
    connections: BTreeMap<u64, Connection>,
}

impl Links {
    /// Listen at `socket`.
    pub fn listen(socket: &str) -> Result<Self, Fail> {
        let listener = link::listen(socket)?;
        let (inbound, from_links) = channel();
        thread::spawn(move || accept_loop(&listener, &inbound));
        Ok(Self { from_links, connections: BTreeMap::new() })
    }

    /// Serve one message if one arrives within `wait`, forever with
    /// `None`. True when something was served.
    pub fn step(&mut self, kernel: &Kernel, wait: Option<Duration>) -> bool {
        let item = match wait {
            None => self.from_links.recv().ok(),
            Some(wait) => self.from_links.recv_timeout(wait).ok(),
        };
        let Some(item) = item else { return false };
        let Some(bytes) = item.bytes else {
            self.connections.remove(&item.link);
            return true;
        };
        let end = self
            .connections
            .entry(item.link)
            .or_insert_with(|| Connection::new(kernel));
        let _ = item.answer.send(end.handle(kernel, &bytes));
        true
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
