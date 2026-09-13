//! Worker-owned checkpoint replacement before dependent service I/O.

use crate::{Notice, checkpoint, session::Session};
use reovim_kernel_interface::ResourceId;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::mpsc,
};

pub struct Journal {
    directory: PathBuf,
    factory: ResourceId,
    notices: mpsc::Sender<Notice>,
}

impl Journal {
    pub fn create(
        path: &str,
        factory: ResourceId,
        notices: mpsc::Sender<Notice>,
    ) -> io::Result<Self> {
        create_directory(path)?;
        let directory = PathBuf::from(path);
        fs::File::open(
            directory
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(".")),
        )?
        .sync_all()?;
        Ok(Self { directory, factory, notices })
    }

    pub fn save(&self, session: &Session) -> io::Result<()> {
        let (send, receive) = mpsc::channel();
        self.notices.send(Notice::Capture(send)).map_err(io::Error::other)?;
        let input = receive.recv().map_err(io::Error::other)??;
        let service = checkpoint::encode(session, self.factory)?;
        let bytes = checkpoint::compose(&service, &input)?;
        let temporary = self.directory.join("next");
        let mut file = private_file(&temporary)?;
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, self.directory.join("state"))?;
            fs::File::open(&self.directory)?.sync_all()
        })();
        if result.is_err() {
            // Only a temporary created by this save is eligible for removal.
            let _ = fs::remove_file(temporary);
        }
        result
    }
}

#[cfg(unix)]
fn create_directory(path: &str) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_directory(_: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "private journal unavailable",
    ))
}

#[cfg(unix)]
fn private_file(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)
}

#[cfg(not(unix))]
fn private_file(_: &Path) -> io::Result<fs::File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "private journal unavailable",
    ))
}
