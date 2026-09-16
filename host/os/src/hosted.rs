//! The std backend: `std::fs` and `SystemTime`, times in nanoseconds
//! since the Unix epoch.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, ErrorKind, IsTerminal, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use crossterm::terminal;

use crate::term::Key;
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
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
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

pub struct Listener(UnixListener);
pub struct Link(UnixStream);

pub fn listen(name: &str) -> Result<Listener, Error> {
    if Path::new(name).exists() {
        std::fs::remove_file(name)?;
    }
    Ok(Listener(UnixListener::bind(name)?))
}

pub fn accept(listener: &Listener) -> Result<Link, Error> {
    Ok(Link(listener.0.accept()?.0))
}

pub fn open(name: &str) -> Result<Link, Error> {
    Ok(Link(UnixStream::connect(name)?))
}

/// `u32` little-endian length, then the bytes.
pub fn send(link: &Link, bytes: &[u8]) -> Result<(), Error> {
    let len = u32::try_from(bytes.len()).map_err(|_| Error::Io)?;
    let mut stream = &link.0;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(bytes)?;
    Ok(())
}

pub fn recv(link: &Link) -> Result<Vec<u8>, Error> {
    let mut stream = &link.0;
    let mut head = [0; 4];
    stream.read_exact(&mut head)?;
    let len =
        usize::try_from(u32::from_le_bytes(head)).map_err(|_| Error::Io)?;
    let mut bytes = alloc::vec![0; len];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub fn close(link: Link) {
    drop(link);
}

/// The dynamic loader. The only `unsafe` in the host: three libc calls.
#[allow(unsafe_code)]
mod dl {
    use alloc::ffi::CString;
    use core::ffi::{c_char, c_int, c_void};

    use crate::Error;

    const RTLD_NOW: c_int = 2;

    unsafe extern "C" {
        fn dlopen(path: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
    }

    /// One `dlopen` handle.
    pub struct Library(*mut c_void);

    impl Drop for Library {
        fn drop(&mut self) {
            // SAFETY: `0` came from a successful `dlopen` and is closed
            // once, here.
            unsafe { dlclose(self.0) };
        }
    }

    pub fn load(path: &str) -> Result<Library, Error> {
        if std::fs::metadata(path).is_err() {
            return Err(Error::NotFound);
        }
        let path = CString::new(path).map_err(|_| Error::Io)?;
        // SAFETY: `path` is NUL-terminated; the caller trusts the file.
        let handle = unsafe { dlopen(path.as_ptr(), RTLD_NOW) };
        if handle.is_null() {
            return Err(Error::Io);
        }
        Ok(Library(handle))
    }

    pub fn symbol(library: &Library) -> Result<*const c_void, Error> {
        // SAFETY: the handle is live and the name NUL-terminated.
        let symbol = unsafe { dlsym(library.0, c"rv_module_entry".as_ptr()) };
        if symbol.is_null() {
            return Err(Error::Unsupported);
        }
        Ok(symbol.cast_const())
    }
}

pub use dl::{Library, load, symbol};

/// `$XDG_RUNTIME_DIR`, else `/tmp/reovim-<uid>`; made if missing.
pub fn runtime() -> Result<String, Error> {
    let dir = match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) if !dir.is_empty() => dir,
        _ => {
            let uid = std::fs::metadata("/proc/self")?.uid();
            alloc::format!("/tmp/reovim-{uid}")
        }
    };
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn work() -> Result<String, Error> {
    let dir = std::env::current_dir()?;
    dir.into_os_string().into_string().map_err(|_| Error::Io)
}

pub fn is_tty() -> bool {
    std::io::stdin().is_terminal()
}

pub fn enter() -> Result<(), Error> {
    terminal::enable_raw_mode()?;
    Ok(())
}

pub fn leave() -> Result<(), Error> {
    terminal::disable_raw_mode()?;
    Ok(())
}

/// One event within `ms`; a key is decoded, anything else is `None`.
pub fn poll(ms: u64) -> Result<Option<Key>, Error> {
    if !event::poll(Duration::from_millis(ms))? {
        return Ok(None);
    }
    match event::read()? {
        Event::Key(key) => Ok(decode(key)),
        _ => Ok(None),
    }
}

pub fn size() -> Result<(u16, u16), Error> {
    Ok(terminal::size()?)
}

fn decode(key: KeyEvent) -> Option<Key> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let decoded = match key.code {
        KeyCode::Char(c) if ctrl => Key::Ctrl(c.to_ascii_lowercase()),
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Tab => Key::Tab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::Esc => Key::Esc,
        _ => return None,
    };
    Some(decoded)
}
