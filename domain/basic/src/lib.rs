#![no_std]
#![forbid(unsafe_code)]
//! Semantic edits over public native values; no Kernel implementation imports.

extern crate alloc;

use alloc::{collections::BTreeMap, vec::Vec};
use reovim_kernel_interface::Error;

/// Replaces a heading label, e.g. `# old\n` becomes `# new\n`.
/// # Errors
/// Requires a heading prefix and an ASCII label; opaque trailing units survive.
pub fn replace_heading(source: &[u8], label: &[u8]) -> Result<Vec<u8>, Error> {
    let start =
        source.iter().position(|byte| *byte != b'#').ok_or(Error::INVALID)?;
    if start == 0
        || source.get(start) != Some(&b' ')
        || label.is_empty()
        || !label.iter().all(u8::is_ascii_alphanumeric)
    {
        return Err(Error::INVALID);
    }
    let end = source[start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(source.len(), |offset| start + offset);
    let mut data = Vec::new();
    data.try_reserve(source.len() + label.len())
        .map_err(|_| Error::PRESSURE)?;
    data.extend_from_slice(&source[..=start]);
    data.extend_from_slice(label);
    data.extend_from_slice(&source[end..]);
    Ok(data)
}

/// Returns independent key values with one boolean inverted.
/// For example, toggling key 17 preserves every other key and the input map.
/// # Errors
/// Missing keys reject; keys are semantic identities, never byte positions.
pub fn toggle(
    source: &BTreeMap<u64, bool>,
    key: u64,
) -> Result<BTreeMap<u64, bool>, Error> {
    let value = source.get(&key).ok_or(Error::UNAVAILABLE)?;
    let mut values = source.clone();
    values.insert(key, !value);
    Ok(values)
}

/// Decodes key/boolean records, e.g. key 17 followed by byte 1 means true.
/// # Errors
/// Requires unique little-endian u64 keys paired with one boolean byte.
pub fn decode_keys(bytes: &[u8]) -> Result<BTreeMap<u64, bool>, Error> {
    if !bytes.len().is_multiple_of(9) {
        return Err(Error::INVALID);
    }
    let mut values = BTreeMap::new();
    for record in bytes.as_chunks::<9>().0 {
        let key = u64::from_le_bytes(
            record[..8].try_into().map_err(|_| Error::INVALID)?,
        );
        if record[8] > 1 || values.insert(key, record[8] == 1).is_some() {
            return Err(Error::INVALID);
        }
    }
    Ok(values)
}

/// Encodes values in key order; decoding preserves their key/boolean meaning.
/// For example, an empty map encodes as empty bytes.
#[must_use]
pub fn encode_keys(values: &BTreeMap<u64, bool>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 9);
    for (key, value) in values {
        bytes.extend_from_slice(&key.to_le_bytes());
        bytes.push(u8::from(*value));
    }
    bytes
}
