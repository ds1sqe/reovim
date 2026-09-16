//! `reovim server`: the Kernel and every linked module on this thread,
//! one io thread per link that moves bytes and nothing else.

use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use reovim_host::{clock, link};
use reovim_kernel_runtime::{Connection, Kernel, Limits};

use crate::Fail;
use crate::link::MODULES;

/// One message from a link, with where its answer goes. `bytes` is
/// `None` when the link ended.
struct Inbound {
    link: u64,
    bytes: Option<Vec<u8>>,
    answer: Sender<Option<Vec<u8>>>,
}

/// Serve `socket` from the directory `root` until killed.
pub fn run(socket: &str, root: &str) -> Result<(), Fail> {
    let listener = link::listen(socket)?;
    let kernel = Kernel::new(clock::now, Limits::default());
    let mut booted = Vec::new();
    for (name, boot) in MODULES {
        let module = boot(root);
        kernel.load(name, module.module.as_ref(), module.drivers.clone())?;
        booted.push(module);
    }
    eprintln!("reovim server: {socket} root {root}");
    let (inbound, from_links) = channel();
    thread::spawn(move || accept_loop(&listener, &inbound));
    serve(&kernel, &from_links);
    drop(booted);
    Ok(())
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

/// The Kernel thread: one `Connection` per link.
fn serve(kernel: &Kernel, from_links: &Receiver<Inbound>) {
    let mut links: BTreeMap<u64, Connection> = BTreeMap::new();
    while let Ok(item) = from_links.recv() {
        let Some(bytes) = item.bytes else {
            links.remove(&item.link);
            continue;
        };
        let end =
            links.entry(item.link).or_insert_with(|| Connection::new(kernel));
        let _ = item.answer.send(end.handle(kernel, &bytes));
    }
}
