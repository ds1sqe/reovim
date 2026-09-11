#![no_std]
#![forbid(unsafe_code)]
//! Semantic edits over public native values; no Kernel implementation imports.

extern crate alloc;

use alloc::{collections::BTreeMap, vec::Vec};
use reovim_kernel_interface::Error;
use reovim_resource_local::{Bytes, Keys};

/// # Errors
/// Requires a heading prefix and an ASCII label; opaque trailing units survive.
pub fn replace_heading(source: &Bytes, label: &[u8]) -> Result<Bytes, Error> {
    let start =
        source.0.iter().position(|byte| *byte != b'#').ok_or(Error::INVALID)?;
    if start == 0
        || source.0.get(start) != Some(&b' ')
        || label.is_empty()
        || !label.iter().all(u8::is_ascii_alphanumeric)
    {
        return Err(Error::INVALID);
    }
    let end = source.0[start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(source.0.len(), |offset| start + offset);
    let mut data = Vec::new();
    data.try_reserve(source.0.len() + label.len())
        .map_err(|_| Error::PRESSURE)?;
    data.extend_from_slice(&source.0[..=start]);
    data.extend_from_slice(label);
    data.extend_from_slice(&source.0[end..]);
    Ok(Bytes(data))
}

/// # Errors
/// Missing keys reject; keys are semantic identities, never byte positions.
pub fn toggle(source: &Keys, key: u64) -> Result<Keys, Error> {
    let value = source.0.get(&key).ok_or(Error::UNAVAILABLE)?;
    let mut values = source.0.clone();
    values.insert(key, !value);
    Ok(Keys(values))
}

/// # Errors
/// Requires unique little-endian u64 keys paired with one boolean byte.
pub fn decode_keys(bytes: &[u8]) -> Result<Keys, Error> {
    if !bytes.len().is_multiple_of(9) {
        return Err(Error::INVALID);
    }
    let mut values = BTreeMap::new();
    for record in bytes.as_chunks::<9>().0 {
        let key = reovim_kernel_interface::buffer::integer(&record[..8])?;
        if record[8] > 1 || values.insert(key, record[8] == 1).is_some() {
            return Err(Error::INVALID);
        }
    }
    Ok(Keys(values))
}

#[must_use]
pub fn encode_keys(values: &Keys) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.0.len() * 9);
    for (key, value) in &values.0 {
        bytes.extend_from_slice(&key.to_le_bytes());
        bytes.push(u8::from(*value));
    }
    bytes
}
