//! The std backend: `std::fs` and `SystemTime`, times in nanoseconds
//! since the Unix epoch.

use alloc::string::ToString;
use alloc::vec::Vec;
use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Entry, Error, Kind, Stat};

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            ErrorKind::NotFound => Self::NotFound,
            ErrorKind::PermissionDenied => Self::Denied,
            ErrorKind::AlreadyExists => Self::Exists,
            _ => Self::Io,
        }
    }
}

fn kind(meta: &Metadata) -> Option<Kind> {
    if meta.is_dir() {
        Some(Kind::Dir)
    } else if meta.is_file() {
        Some(Kind::File)
    } else {
        None
    }
}

fn nanos(time: SystemTime) -> u64 {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    u64::try_from(since.as_nanos()).unwrap_or(u64::MAX)
}

/// One `list` row, or `None` for a name or kind the seam cannot say.
fn entry(entry: &std::fs::DirEntry) -> Option<Entry> {
    let name = entry.file_name().to_str()?.to_string();
    let kind = kind(&entry.path().metadata().ok()?)?;
    Some(Entry { name, kind })
}

pub fn list(dir: &str) -> Result<Vec<Entry>, Error> {
    let mut entries = Vec::new();
    for found in std::fs::read_dir(dir)? {
        entries.extend(entry(&found?));
    }
    Ok(entries)
}

pub fn stat(path: &str) -> Result<Stat, Error> {
    let meta = std::fs::metadata(path)?;
    let kind = kind(&meta).ok_or(Error::Io)?;
    let mtime = meta.modified().map(nanos).unwrap_or_default();
    Ok(Stat { kind, len: meta.len(), mtime })
}

pub fn read(path: &str, at: u64, len: usize) -> Result<Vec<u8>, Error> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(at))?;
    let mut bytes = Vec::new();
    file.take(len as u64).read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub fn write(path: &str, at: u64, bytes: &[u8]) -> Result<usize, Error> {
    let mut file = OpenOptions::new().write(true).open(path)?;
    file.seek(SeekFrom::Start(at))?;
    file.write_all(bytes)?;
    file.set_len(at + bytes.len() as u64)?;
    Ok(bytes.len())
}

pub fn create(path: &str) -> Result<(), Error> {
    OpenOptions::new().write(true).create_new(true).open(path)?;
    Ok(())
}

pub fn now() -> u64 {
    nanos(SystemTime::now())
}
