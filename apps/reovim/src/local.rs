//! A Client inside the server process: its messages go through a
//! `Connection` on this thread, no link.

use std::rc::Rc;

use reovim_client::{Carrier, Client, Fault};
use reovim_kernel_runtime::{Connection, Kernel, NoLoader};

pub struct Local {
    kernel: Rc<Kernel>,
    connection: Connection,
}

impl Local {
    /// A connected Client on `kernel`.
    pub fn client(kernel: &Rc<Kernel>) -> Result<Client, Fault> {
        let connection = Connection::new();
        let local = Self { kernel: kernel.clone(), connection };
        Client::connect(Box::new(local), Vec::new())
    }
}

impl Carrier for Local {
    fn exchange(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Fault> {
        self.connection
            .handle(&self.kernel, bytes, &mut NoLoader)
            .ok_or(Fault::Wire)
    }
}
