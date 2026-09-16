//! Byte helpers for the value encoding: little-endian scalars, u32 length
//! then payload. Shared by the canonical declaration form and the wire.

use alloc::string::String;
use alloc::vec::Vec;

pub fn put_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

pub fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_u32(out, u32::try_from(bytes.len()).unwrap_or(u32::MAX));
    out.extend_from_slice(bytes);
}

pub fn put_str(out: &mut Vec<u8>, text: &str) {
    put_bytes(out, text.as_bytes());
}

/// Reads one value after another; `None` once the bytes run out.
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    /// Every byte was consumed.
    pub const fn done(&self) -> bool {
        self.at == self.bytes.len()
    }

    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let slice = self.bytes.get(self.at..self.at.checked_add(len)?)?;
        self.at += len;
        Some(slice)
    }

    pub fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    pub fn u16(&mut self) -> Option<u16> {
        self.take(2)?.try_into().ok().map(u16::from_le_bytes)
    }

    pub fn u32(&mut self) -> Option<u32> {
        self.take(4)?.try_into().ok().map(u32::from_le_bytes)
    }

    pub fn u64(&mut self) -> Option<u64> {
        self.take(8)?.try_into().ok().map(u64::from_le_bytes)
    }

    pub fn bool(&mut self) -> Option<bool> {
        match self.u8()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    pub fn bytes(&mut self) -> Option<Vec<u8>> {
        let len = usize::try_from(self.u32()?).ok()?;
        self.take(len).map(<[u8]>::to_vec)
    }

    pub fn str(&mut self) -> Option<String> {
        String::from_utf8(self.bytes()?).ok()
    }
}
