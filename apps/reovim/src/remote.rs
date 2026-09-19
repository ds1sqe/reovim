//! A Client over the link: one message out, the next message in; a
//! fresh link to the same socket when the old one is gone.

use reovim_client::{Carrier, Client, Fault};
use reovim_host::link::{self, Link};

use crate::Fail;

pub struct Remote {
    link: Link,
    socket: String,
}

impl Remote {
    /// A Client on whoever listens at `socket`, connected as
    /// `credential`.
    pub fn client(socket: &str, credential: Vec<u8>) -> Result<Client, Fail> {
        let link = link::open(socket)?;
        let remote = Self { link, socket: socket.to_string() };
        Ok(Client::connect(Box::new(remote), credential)?)
    }
}

impl Carrier for Remote {
    fn exchange(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Fault> {
        link::send(&self.link, bytes).map_err(|_| Fault::Carrier)?;
        link::recv(&self.link).map_err(|_| Fault::Carrier)
    }

    fn reconnect(&mut self) -> Result<(), Fault> {
        self.link = link::open(&self.socket).map_err(|_| Fault::Carrier)?;
        Ok(())
    }

    /// A millisecond: the answer is another process's to give, and an
    /// asker that spun would keep the editor it asks of from ever
    /// reaching the loop that would answer.
    fn pause(&mut self) {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
