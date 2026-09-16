//! The std backend: `std::fs` and `SystemTime`, times in nanoseconds
//! since the Unix epoch.

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, ErrorKind, IsTerminal, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Once;
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

/// A sibling temporary renamed over the path, synced first. A symlink,
/// a file with more than one link, or a directory that refuses the
/// temporary gets the bytes in place, as Vim's `backupcopy=auto` does.
pub fn replace(path: &str, bytes: &[u8]) -> Result<(), Error> {
    let old = std::fs::symlink_metadata(path).ok();
    let linked = old
        .as_ref()
        .is_some_and(|m| m.file_type().is_symlink() || m.nlink() > 1);
    let temporary = alloc::format!("{path}.{}~", std::process::id());
    let fresh = if linked {
        None
    } else {
        OpenOptions::new().write(true).create_new(true).open(&temporary).ok()
    };
    let Some(mut file) = fresh else {
        return write(path, 0, bytes).map(|_| ());
    };
    let done = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| {
            old.map_or(Ok(()), |meta| {
                std::fs::set_permissions(&temporary, meta.permissions())
            })
        })
        .and_then(|()| std::fs::rename(&temporary, path));
    if done.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    Ok(done?)
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

/// The dynamic loader: three libc calls.
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

/// The clipboard tools in the order tried: what reads, what writes.
const CLIPBOARDS: [(&[&str], &[&str]); 4] = [
    (&["wl-paste", "--no-newline"], &["wl-copy"]),
    (
        &["xclip", "-selection", "clipboard", "-o"],
        &["xclip", "-selection", "clipboard"],
    ),
    (&["xsel", "--clipboard", "--output"], &["xsel", "--clipboard", "--input"]),
    (&["pbpaste"], &["pbcopy"]),
];

/// `tool` run to its end; `None` when it is not on PATH.
fn read_tool(tool: &[&str]) -> Option<Result<Vec<u8>, Error>> {
    let run =
        Command::new(tool[0]).args(&tool[1..]).stderr(Stdio::null()).output();
    match run {
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Ok(out) if out.status.success() => Some(Ok(out.stdout)),
        _ => Some(Err(Error::Io)),
    }
}

/// `tool` given `bytes` on its stdin; `None` when it is not on PATH.
fn write_tool(tool: &[&str], bytes: &[u8]) -> Option<Result<(), Error>> {
    let child = Command::new(tool[0])
        .args(&tool[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Err(error) if error.kind() == ErrorKind::NotFound => return None,
        Err(_) => return Some(Err(Error::Io)),
        Ok(child) => child,
    };
    let fed =
        child.stdin.take().map_or(Ok(()), |mut stdin| stdin.write_all(bytes));
    let done = fed.and_then(|()| child.wait());
    Some(done.map(|_| ()).map_err(|_| Error::Io))
}

pub fn clipboard_read() -> Result<Vec<u8>, Error> {
    CLIPBOARDS
        .iter()
        .find_map(|(read, _)| read_tool(read))
        .unwrap_or(Err(Error::Unsupported))
}

pub fn clipboard_write(bytes: &[u8]) -> Result<(), Error> {
    CLIPBOARDS
        .iter()
        .find_map(|(_, write)| write_tool(write, bytes))
        .unwrap_or(Err(Error::Unsupported))
}

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

/// Set by SIGTERM or SIGHUP; `poll` ends and `stopped` says so.
static STOP: AtomicBool = AtomicBool::new(false);
static INSTALLED: Once = Once::new();

/// Raw mode on the alternate screen. The first call installs the
/// panic hook and the signal handlers.
pub fn enter() -> Result<(), Error> {
    terminal::enable_raw_mode()?;
    screen(b"\x1b[?1049h")?;
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = leave();
            previous(info);
        }));
        sig::install();
    });
    Ok(())
}

/// The cursor shown, the main screen, cooked mode.
pub fn leave() -> Result<(), Error> {
    screen(b"\x1b[?25h\x1b[?1049l")?;
    terminal::disable_raw_mode()?;
    Ok(())
}

fn screen(bytes: &[u8]) -> io::Result<()> {
    let mut out = io::stdout();
    out.write_all(bytes)?;
    out.flush()
}

pub fn stopped() -> bool {
    STOP.load(Ordering::Relaxed)
}

/// SIGTERM and SIGHUP store one bool; the loop leaves the normal way.
#[allow(unsafe_code)]
mod sig {
    use core::ffi::c_int;
    use core::sync::atomic::Ordering;

    const SIGHUP: c_int = 1;
    const SIGTERM: c_int = 15;

    unsafe extern "C" {
        fn signal(sig: c_int, handler: extern "C" fn(c_int)) -> usize;
    }

    extern "C" fn stop(_: c_int) {
        super::STOP.store(true, Ordering::Relaxed);
    }

    pub fn install() {
        // SAFETY: `stop` stores one bool and calls nothing, so it may
        // run at any point; the old handlers are not needed back.
        unsafe {
            signal(SIGHUP, stop);
            signal(SIGTERM, stop);
        }
    }
}

/// One event within `ms`; a key is decoded, anything else is `None`,
/// and nothing is waited for after a signal.
pub fn poll(ms: u64) -> Result<Option<Key>, Error> {
    if stopped() || !event::poll(Duration::from_millis(ms))? {
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
