use alloc::vec::Vec;
use core::str;

use super::{
    ValueType,
    decode::{Cursor, grammar},
};
use crate::Error;

/// Runtime-selected bounds for validating one encoded value and its slots.
///
/// ```
/// use reovim_kernel_interface::declaration::ValueLimits;
/// let limits = ValueLimits { bytes: 4096, elements: 256, references: 8 };
/// assert!(limits.bytes >= 4); // Accommodates a canonical U32 argument.
/// ```
#[derive(Clone, Copy, Debug)]
pub struct ValueLimits {
    /// Maximum data bytes. Example: `bytes: 4096` rejects a 4097-byte value.
    pub bytes: usize,
    /// Maximum visited value nodes, including Unit and aggregate nodes.
    /// Example: `elements: 4` permits a record and three scalar fields.
    pub elements: usize,
    /// Maximum separately supplied slots, all of which must be used.
    /// Example: `references: 0` permits only reference-free actual values.
    pub references: usize,
}

struct Validation<'a, F> {
    data: Cursor<'a>,
    remaining: usize,
    used: Vec<bool>,
    check: F,
}

pub(super) fn validate(
    ty: &ValueType,
    data: &[u8],
    references: usize,
    limits: ValueLimits,
    check: impl FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
) -> Result<(), Error> {
    let remaining = prefix(ty, data, references, limits, check)?;
    if remaining.is_empty() { Ok(()) } else { Err(Error::INVALID) }
}

pub(super) fn prefix<'a>(
    ty: &ValueType,
    data: &'a [u8],
    references: usize,
    limits: ValueLimits,
    check: impl FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
) -> Result<&'a [u8], Error> {
    if data.len() > limits.bytes || references > limits.references {
        return Err(Error::PRESSURE);
    }
    let mut used = Vec::new();
    used.try_reserve_exact(references).map_err(|_| Error::PRESSURE)?;
    used.resize(references, false);
    let mut state = Validation {
        data: Cursor(data),
        remaining: limits.elements,
        used,
        check,
    };
    walk(&mut Cursor(ty.bytes()), &mut state)?;
    if state.used.iter().any(|used| !used) {
        return Err(Error::INVALID);
    }
    Ok(state.data.0)
}

fn walk<F>(
    schema: &mut Cursor<'_>,
    state: &mut Validation<'_, F>,
) -> Result<(), Error>
where
    F: FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
{
    state.remaining = state.remaining.checked_sub(1).ok_or(Error::PRESSURE)?;
    match schema.byte()? {
        0 => Ok(()),
        1 => boolean(&mut state.data),
        2 => state.data.take(1).map(|_| ()),
        3 => state.data.take(4).map(|_| ()),
        4..=6 => state.data.take(8).map(|_| ()),
        7 => bytes(schema, &mut state.data, false),
        8 => bytes(schema, &mut state.data, true),
        9 => record(schema, state),
        10 => sequence(schema, state, false),
        11 => choice(schema, state),
        12 => reference(schema, state),
        13 => identity(&mut state.data),
        14 => sequence(schema, state, true),
        _ => Err(Error::INVALID),
    }
}

fn boolean(data: &mut Cursor<'_>) -> Result<(), Error> {
    if data.byte()? > 1 { Err(Error::INVALID) } else { Ok(()) }
}

fn bytes(
    schema: &mut Cursor<'_>,
    data: &mut Cursor<'_>,
    utf8: bool,
) -> Result<(), Error> {
    let maximum = schema.integer()?;
    let count = data.integer()?;
    if count > maximum {
        return Err(Error::INVALID);
    }
    let count = usize::try_from(count).map_err(|_| Error::PRESSURE)?;
    let value = data.take(count)?;
    if utf8 {
        str::from_utf8(value).map_err(|_| Error::INVALID)?;
    }
    Ok(())
}

fn record<F>(
    schema: &mut Cursor<'_>,
    state: &mut Validation<'_, F>,
) -> Result<(), Error>
where
    F: FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
{
    for _ in 0..schema.short()? {
        schema.name()?;
        walk(schema, state)?;
    }
    Ok(())
}

fn sequence<F>(
    schema: &mut Cursor<'_>,
    state: &mut Validation<'_, F>,
    fixed: bool,
) -> Result<(), Error>
where
    F: FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
{
    let maximum = schema.integer()?;
    let count = if fixed { maximum } else { state.data.integer()? };
    if count > maximum {
        return Err(Error::INVALID);
    }
    let count = usize::try_from(count).map_err(|_| Error::PRESSURE)?;
    if count > state.remaining {
        return Err(Error::PRESSURE);
    }
    let element = *schema;
    grammar(schema, 1, true)?;
    for _ in 0..count {
        walk(&mut element.clone(), state)?;
    }
    Ok(())
}

fn choice<F>(
    schema: &mut Cursor<'_>,
    state: &mut Validation<'_, F>,
) -> Result<(), Error>
where
    F: FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
{
    let count = schema.short()?;
    let selected = state.data.integer()?;
    let mut found = false;
    for _ in 0..count {
        let tag = schema.integer()?;
        schema.name()?;
        found |= variant(schema, state, tag == selected)?;
    }
    if found { Ok(()) } else { Err(Error::INVALID) }
}

fn variant<F>(
    schema: &mut Cursor<'_>,
    state: &mut Validation<'_, F>,
    selected: bool,
) -> Result<bool, Error>
where
    F: FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
{
    if selected {
        walk(schema, state)?;
    } else {
        grammar(schema, 1, true)?;
    }
    Ok(selected)
}

fn reference<F>(
    schema: &mut Cursor<'_>,
    state: &mut Validation<'_, F>,
) -> Result<(), Error>
where
    F: FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
{
    let api = schema.take(16)?.try_into().map_err(|_| Error::INVALID)?;
    let rights = schema.integer()?;
    let slot = state.data.integer()?;
    let index = usize::try_from(slot).map_err(|_| Error::INVALID)?;
    let used = state.used.get_mut(index).ok_or(Error::INVALID)?;
    (state.check)(slot, (api != [0; 16]).then_some(api), rights)?;
    *used = true;
    Ok(())
}

fn identity(data: &mut Cursor<'_>) -> Result<(), Error> {
    data.take(16)?; // Kernel epoch; identity is metadata, never access.
    let serial = data.take(8)?;
    if serial == [0; 8] { Err(Error::INVALID) } else { Ok(()) }
}
