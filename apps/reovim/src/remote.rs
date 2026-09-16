//! A Client over the link: one message out, the next message in.

use reovim_client::{Carrier, Client, Fault};
use reovim_host::link::{self, Link};

use crate::Fail;

pub struct Remote(Link);

impl Remote {
    /// A connected Client on whoever listens at `socket`.
    pub fn client(socket: &str) -> Result<Client, Fail> {
        let end = link::open(socket)?;
        Ok(Client::connect(Box::new(Self(end)), Vec::new())?)
    }
}

impl Carrier for Remote {
    fn exchange(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Fault> {
        link::send(&self.0, bytes).map_err(|_| Fault::Carrier)?;
        link::recv(&self.0).map_err(|_| Fault::Carrier)
    }
}
