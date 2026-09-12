//! Finite file output through a granted directory Resource and Host workers.

use crate::path;
use crate::worker;

use alloc::{boxed::Box, rc::Rc, sync::Arc};
use core::sync::atomic::{AtomicBool, Ordering};
use reovim_kernel_interface::{
    Contract, Error, Message,
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};
use std::{
    fs::File,
    io::{self, Write},
    path::Path,
    sync::OnceLock,
};

struct Destination {
    parent: Arc<File>,
    child: Option<(Box<str>, OnceLock<File>)>,
}
impl Destination {
    fn execute(&self, input: Message) -> Result<Message, Error> {
        let root = match self.root() {
            Ok(root) => root,
            Err(error) => {
                return Ok(outcome(input.contract, 1, 0, Some(error)));
            }
        };
        execute(root, input)
    }

    fn root(&self) -> io::Result<&File> {
        let Some((name, opened)) = &self.child else {
            return Ok(&self.parent);
        };
        if let Some(file) = opened.get() {
            return Ok(file);
        }
        // Only the admitted Host worker initializes this private directory.
        path::directory(&self.parent, name)?;
        let file = path::child(&self.parent, name)?;
        Ok(opened.get_or_init(|| file))
    }
}

pub struct Directory {
    root: Arc<File>,
}
impl Directory {
    /// Opens the explicit grant without following a symlink.
    /// # Errors
    /// Returns the native error for a missing or inaccessible directory.
    pub fn open(root: &Path) -> io::Result<Self> {
        Ok(Self { root: Arc::new(path::root(root)?) })
    }
    /// Creates a private output directory; never adopts an existing name.
    /// # Errors
    /// Returns the native create/open error, including a colliding name.
    pub fn create(&self, name: &str) -> io::Result<Self> {
        path::directory(&self.root, name)?;
        Ok(Self { root: Arc::new(path::child(&self.root, name)?) })
    }
    #[must_use]
    pub fn provider(&self) -> ResourceProvider {
        provider(Destination { parent: self.root.clone(), child: None })
    }

    /// Reserves a private output name without performing filesystem work.
    /// The first admitted output creates it exclusively on the Host worker.
    /// # Errors
    /// Rejects an invalid path component before constructing the provider.
    pub fn private_provider(&self, name: &str) -> io::Result<ResourceProvider> {
        path::component(name)?;
        Ok(provider(Destination {
            parent: self.root.clone(),
            child: Some((name.into(), OnceLock::new())),
        }))
    }
}
fn provider(destination: Destination) -> ResourceProvider {
    ResourceProvider::new(
        (Arc::new(destination), Arc::new(AtomicBool::new(false))),
        ResourceOps {
            unload: Some(|(_, busy)| {
                busy.load(Ordering::Acquire)
                    .then_some(Error::BUSY)
                    .map_or(Ok(()), Err)
            }),
            read: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
            write: |(root, busy), _, input| {
                let root = root.clone();
                worker::start(busy, move || root.execute(input))
            },
        },
        Box::new([
            surface("host.export.directory"),
            surface("host.export.file"),
        ]),
        Rc::new(()),
    )
}
fn surface(name: &str) -> Surface {
    Surface {
        contract: Contract::new(name, 1),
        methods: Permissions::WRITE,
        input: Box::new([]),
        output: Box::new([]),
    }
}
fn execute(root: &File, input: Message) -> Result<Message, Error> {
    let split = input.data.iter().position(|byte| *byte == 0);
    let name =
        core::str::from_utf8(&input.data[..split.unwrap_or(input.data.len())])
            .map_err(|_| Error::INVALID)?;
    if input.contract.name.as_ref() == "host.export.directory" {
        if split.is_some() {
            return Err(Error::INVALID);
        }
        return Ok(create_directory(root, name, input.contract));
    }
    let split = split.ok_or(Error::INVALID)?;
    let data = &input.data[split + 1..];
    if data.len() > 16 * 1024 || name.len() > 8192 {
        return Err(Error::PRESSURE);
    }
    let (status, written, error) = write(root, name, data);
    Ok(outcome(input.contract, status, written, error))
}
fn create_directory(root: &File, name: &str, contract: Contract) -> Message {
    if let Err(error) = path::make_directory(root, name) {
        return outcome(contract, 1, 0, Some(error));
    }
    match root.sync_all() {
        Ok(()) => outcome(contract, 0, 0, None),
        Err(error) => outcome(contract, 2, 0, Some(error)),
    }
}
fn write(root: &File, name: &str, data: &[u8]) -> (u8, u64, Option<io::Error>) {
    let mut file = match path::file(root, name) {
        Ok(file) => file,
        Err(error) => return (1, 0, Some(error)),
    };
    let mut written = 0;
    while written < data.len() {
        match file.write(&data[written..]) {
            Ok(0) => {
                return (
                    1,
                    written as u64,
                    Some(io::ErrorKind::WriteZero.into()),
                );
            }
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
            Err(error) => return (1, written as u64, Some(error)),
        }
    }
    match file.sync_all() {
        Ok(()) => (0, written as u64, None),
        Err(error) => (2, written as u64, Some(error)),
    }
}
fn outcome(
    contract: Contract,
    status: u8,
    written: u64,
    error: Option<io::Error>,
) -> Message {
    let mut data = [0; 17];
    data[0] = status;
    data[1..9].copy_from_slice(&written.to_le_bytes());
    let code = error.map_or(0, |e| {
        e.raw_os_error().and_then(|n| u64::try_from(n).ok()).unwrap_or(u64::MAX)
    });
    data[9..].copy_from_slice(&code.to_le_bytes());
    Message::new(contract, &data)
}
