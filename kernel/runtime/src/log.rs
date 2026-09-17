//! `/sys/log`: one bounded ring of stamped entries; the oldest are
//! trimmed. A reader that fell behind continues from the oldest kept
//! and sees the gap in the seqs.

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

/// syslog's eight levels, `emerg` 0 to `debug` 7, in the entry's byte.
/// The Kernel writes at these; a writer to `/sys/log` says which with
/// a `<N>` in front, as printk does.
pub const ERR: u8 = 3;
pub const WARN: u8 = 4;
pub const NOTICE: u8 = 5;
pub const INFO: u8 = 6;
pub const DEBUG: u8 = 7;

/// The eight in level order; `/sys/loglevel` says the level by name.
const NAMES: [&str; 8] =
    ["emerg", "alert", "crit", "err", "warn", "notice", "info", "debug"];

/// The name of a level; a byte past the eight is `debug`, the least
/// urgent of them.
pub fn name(level: u8) -> &'static str {
    NAMES.get(usize::from(level)).copied().unwrap_or("debug")
}

/// The level a name or its number stands for: `debug` and `7` are the
/// same level, and nothing else is one.
pub fn level_named(text: &str) -> Option<u8> {
    if let Some(at) = NAMES.iter().position(|name| *name == text) {
        return u8::try_from(at).ok();
    }
    text.parse().ok().filter(|level| usize::from(*level) < NAMES.len())
}

/// The `<N>` a written entry may start with, and the bytes after it.
/// No prefix, or one that is not a single digit of the eight, is
/// `INFO` and the bytes as they came.
pub fn level_of(bytes: &[u8]) -> (u8, &[u8]) {
    match bytes {
        [b'<', digit @ b'0'..=b'7', b'>', rest @ ..] => (digit - b'0', rest),
        _ => (INFO, bytes),
    }
}

pub struct Entry {
    pub seq: u64,
    pub time: u64,
    pub mark: String,
    pub level: u8,
    pub bytes: Vec<u8>,
}

pub struct Log {
    entries: VecDeque<Entry>,
    next_seq: u64,
    level: u8,
}

impl Log {
    /// Empty, at `INFO` and above. How many entries it keeps is not
    /// its own: `/sys/limit/log_ring` is the one source and every
    /// append is given it, so the two can never drift.
    pub const fn new() -> Self {
        Self { entries: VecDeque::new(), next_seq: 1, level: INFO }
    }

    /// The level the ring keeps up to, printk's `console_loglevel` made
    /// the ring's own; `/sys/loglevel` reads and writes it.
    pub const fn level(&self) -> u8 {
        self.level
    }

    pub const fn set_level(&mut self, level: u8) {
        self.level = level;
    }

    /// Whether an entry at `level` is kept at all. An entry above it is
    /// never built, so what the level turns off costs nothing.
    pub const fn keeps(&self, level: u8) -> bool {
        level <= self.level
    }

    /// One entry, the oldest trimmed until `cap` are left: a `cap`
    /// smaller than the ring is applied here, on the next append.
    pub fn append(
        &mut self,
        time: u64,
        mark: &str,
        level: u8,
        bytes: Vec<u8>,
        cap: usize,
    ) {
        let seq = self.next_seq;
        self.next_seq += 1;
        let mark = String::from(mark);
        self.entries.push_back(Entry { seq, time, mark, level, bytes });
        while self.entries.len() > cap {
            self.entries.pop_front();
        }
    }

    /// Every entry kept, oldest first; the core writes them all.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    /// Whole entries from `seq` on, encoded, while they fit in `max`.
    /// Encoding: u64 seq, u64 time, u32 mark length, mark, u8 level,
    /// u32 length, bytes; all little endian.
    pub fn read(&self, seq: u64, max: usize) -> Vec<u8> {
        collect(self.entries.iter().filter(|e| e.seq >= seq), max)
    }
}

fn collect<'a>(
    entries: impl Iterator<Item = &'a Entry>,
    max: usize,
) -> Vec<u8> {
    let mut out = Vec::new();
    for entry in entries {
        let encoded = encode(entry);
        if out.len() + encoded.len() > max {
            break;
        }
        out.extend_from_slice(&encoded);
    }
    out
}

fn encode(entry: &Entry) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&entry.seq.to_le_bytes());
    out.extend_from_slice(&entry.time.to_le_bytes());
    put_blob(&mut out, entry.mark.as_bytes());
    out.push(entry.level);
    put_blob(&mut out, &entry.bytes);
    out
}

fn put_blob(out: &mut Vec<u8>, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
}
