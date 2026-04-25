//! SHA-256 over cdylib bytes.
//!
//! Streams the file through `sha2::Sha256` in 64 KiB chunks rather
//! than loading the whole artifact into memory, so arbitrarily
//! large cdylibs hash without allocating proportional heap.

use std::{io::Read, path::Path};

use sha2::{Digest, Sha256};

use crate::error::InstallError;

const CHUNK: usize = 64 * 1024;

pub fn digest(path: &Path) -> Result<String, InstallError> {
    let mut file = std::fs::File::open(path).map_err(|e| read_failed(path, &e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let read = file.read(&mut buf).map_err(|e| read_failed(path, &e))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    let out = hasher.finalize();
    Ok(hex_lower(&out))
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn read_failed(at: &Path, e: &std::io::Error) -> InstallError {
    InstallError::ArtifactReadFailed {
        at: at.to_path_buf(),
        reason: e.to_string(),
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

#[cfg(test)]
#[path = "hash_tests.rs"]
mod hash_tests;
