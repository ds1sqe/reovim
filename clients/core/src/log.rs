//! `/sys/log` as a Client reads it: whole entries from a sequence.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_kernel_interface::{Handle, Rd, Rights};

use crate::{Client, Fault};

/// Bytes asked of the log per `Read`.
const CHUNK: u32 = 64 * 1024;

/// One entry: `(seq, time, mark, level, bytes)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub seq: u64,
    pub time: u64,
    pub mark: String,
    pub level: u8,
    pub bytes: Vec<u8>,
}

/// syslog's eight, `emerg` 0 to `debug` 7, as the level byte counts
/// them. The Kernel writes its own entries at these and takes a
/// writer's `<N>` into the byte.
pub const LEVELS: [&str; 8] =
    ["emerg", "alert", "crit", "err", "warn", "notice", "info", "debug"];

/// The byte a level name stands for; `None` for a name that is not one
/// of the eight.
#[must_use]
pub fn level(name: &str) -> Option<u8> {
    let at = LEVELS.iter().position(|level| *level == name)?;
    u8::try_from(at).ok()
}

impl Entry {
    /// The name of this entry's level; a byte past the eight is
    /// `debug`, the least urgent of them.
    #[must_use]
    pub fn level_name(&self) -> &'static str {
        let at = usize::from(self.level);
        LEVELS.get(at).copied().unwrap_or("debug")
    }

    /// The text, lossy UTF-8.
    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }

    /// `seq time mark level text` and a newline, text lossy UTF-8.
    #[must_use]
    pub fn line(&self) -> String {
        let text = String::from_utf8_lossy(&self.bytes);
        format!(
            "{} {} {} {} {}\n",
            self.seq, self.time, self.mark, self.level, text
        )
    }
}

/// Whole entries at the front of `bytes`, in the encoding the Kernel
/// serves: u64 seq, u64 time, u32-length mark, u8 level, u32-length
/// bytes. Stops at a partial one.
#[must_use]
pub fn parse(bytes: &[u8]) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut rest = bytes;
    while let Some((entry, after)) = one(rest) {
        entries.push(entry);
        rest = after;
    }
    entries
}

fn one(bytes: &[u8]) -> Option<(Entry, &[u8])> {
    let (seq, bytes) = u64_at(bytes)?;
    let (time, bytes) = u64_at(bytes)?;
    let (mark, bytes) = blob_at(bytes)?;
    let (&level, bytes) = bytes.split_first()?;
    let (payload, bytes) = blob_at(bytes)?;
    let mark = String::from_utf8_lossy(mark).into_owned();
    let entry = Entry { seq, time, mark, level, bytes: payload.to_vec() };
    Some((entry, bytes))
}

fn u64_at(bytes: &[u8]) -> Option<(u64, &[u8])> {
    let (head, rest) = bytes.split_first_chunk::<8>()?;
    Some((u64::from_le_bytes(*head), rest))
}

fn blob_at(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    let (head, rest) = bytes.split_first_chunk::<4>()?;
    let len = usize::try_from(u32::from_le_bytes(*head)).ok()?;
    (rest.len() >= len).then(|| rest.split_at(len))
}

/// Every entry from `from` on, through `h`, a handle on the log opened
/// READ. A `Read` at a sequence answers whole entries from there.
///
/// # Errors
/// As `Client::read`.
pub fn read(
    client: &mut Client,
    h: Handle,
    from: u64,
) -> Result<Vec<Entry>, Fault> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut next = from;
    loop {
        let got = parse(&client.read(h, next, CHUNK)?);
        let Some(last) = got.last() else { return Ok(entries) };
        next = last.seq + 1;
        entries.extend(got);
    }
}

/// The log rd, `/sys/log`, and every entry from `from`, through a handle
/// opened and closed here.
///
/// # Errors
/// As `Client::resolve` and [`read`].
pub fn tail(client: &mut Client, from: u64) -> Result<Vec<Entry>, Fault> {
    let rd = client.resolve("/sys/log")?;
    read_rd(client, rd, from)
}

fn read_rd(
    client: &mut Client,
    rd: Rd,
    from: u64,
) -> Result<Vec<Entry>, Fault> {
    let h = client.open(rd, Rights::READ)?;
    let entries = read(client, h, from);
    client.close(h)?;
    entries
}
