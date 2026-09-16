//! `server --log PATH`: every `/sys/log` entry appended to a file as
//! one text line. The Kernel keeps its ring; this is a consumer's copy,
//! kept by a local Client that watches the log.

use std::rc::Rc;

use reovim_client::{Client, Handle, Kind, Rd, Rights, log};
use reovim_host::fs;
use reovim_kernel_runtime::Kernel;

use crate::Fail;
use crate::local::Local;

pub struct Sink {
    client: Client,
    log: Rd,
    h: Handle,
    /// The first sequence not yet in the file.
    next: u64,
    /// Read before the first `since`, so what was logged before the
    /// watch is not missed.
    dirty: bool,
    path: String,
}

impl Sink {
    /// A Client on `kernel` watching `/sys/log`; `path` made if missing.
    pub fn open(kernel: &Rc<Kernel>, path: &str) -> Result<Self, Fail> {
        let mut client = Local::client(kernel)?;
        let log = client.resolve("/sys/log")?;
        let h = client.open(log, Rights::READ)?;
        client.watch(h, true)?;
        match fs::create(path) {
            Ok(()) | Err(reovim_host::Error::Exists) => {}
            Err(error) => return Err(error.into()),
        }
        let path = path.to_string();
        Ok(Self { client, log, h, next: 1, dirty: true, path })
    }

    /// Append what arrived since the last step.
    pub fn step(&mut self) -> Result<(), Fail> {
        let events = self.client.since()?;
        let modified = |e: &reovim_client::Event| {
            e.rd == self.log && e.kind == Kind::Modify
        };
        if !self.dirty && !events.iter().any(modified) {
            return Ok(());
        }
        self.dirty = false;
        let entries = log::read(&mut self.client, self.h, self.next)?;
        let Some(last) = entries.last() else { return Ok(()) };
        self.next = last.seq + 1;
        let text: String = entries.iter().map(log::Entry::line).collect();
        let at = fs::stat(&self.path)?.len;
        fs::write(&self.path, at, text.as_bytes())?;
        Ok(())
    }
}
