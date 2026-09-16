//! `/sys/log`: a plain, growing list of stamped entries. Trimming waits
//! for milestone 4.

use alloc::string::String;
use alloc::vec::Vec;

pub struct Entry {
    pub seq: u64,
    pub time: u64,
    pub mark: String,
    pub level: u8,
    pub bytes: Vec<u8>,
}

pub struct Log {
    entries: Vec<Entry>,
    next_seq: u64,
}

impl Log {
    pub const fn new() -> Self {
        Self { entries: Vec::new(), next_seq: 1 }
    }

    pub fn append(&mut self, time: u64, mark: &str, level: u8, bytes: Vec<u8>) {
        let seq = self.next_seq;
        self.next_seq += 1;
        let mark = String::from(mark);
        self.entries.push(Entry { seq, time, mark, level, bytes });
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
