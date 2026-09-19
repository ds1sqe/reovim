//! A Client inside the server process: its messages go through a
//! `Connection` on this thread, no link. It loads through the same
//! `Files` the socket server loads through, so `load` from the cli
//! buffer of a bare `reovim FILE` reaches the ABI as it does over a
//! socket.

use std::cell::RefCell;
use std::rc::Rc;

use reovim_client::{Carrier, Client, Fault};
use reovim_kernel_runtime::{Connection, Kernel};

use crate::server::{Files, Links};

pub struct Local {
    kernel: Rc<Kernel>,
    connection: Connection,
    files: Files,
    /// The server's links, when this Client shares a thread with them;
    /// `None` for one that waits on nobody, as the log's sink does.
    links: Option<Rc<RefCell<Links>>>,
}

impl Local {
    /// A connected Client on `kernel`, whose waits serve `links`.
    pub fn client(
        kernel: &Rc<Kernel>,
        links: Option<&Rc<RefCell<Links>>>,
    ) -> Result<Client, Fault> {
        let kernel = kernel.clone();
        let links = links.cloned();
        let local =
            Self { kernel, connection: Connection::new(), files: Files, links };
        Client::connect(Box::new(local), Vec::new())
    }
}

impl Carrier for Local {
    fn exchange(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Fault> {
        self.connection
            .handle(&self.kernel, bytes, &mut self.files)
            .ok_or(Fault::Wire)
    }

    /// The other links served, never this Client's own: what this one
    /// waits on is another Client's to answer, and that Client's
    /// messages arrive on this thread. Without it the editor's own
    /// loop would never be reached again.
    fn pause(&mut self) {
        let Some(links) = &self.links else { return };
        if let Ok(mut links) = links.try_borrow_mut() {
            links.drain(&self.kernel);
        }
    }
}
