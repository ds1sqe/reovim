use alloc::{boxed::Box, string::String, vec::Vec};
use core::str;

use super::{Declaration, Dispatch, Method, Operation, ValueType};
use crate::Error;

const MAX_BYTES: usize = 64 * 1024;
const MAX_ITEMS: usize = 256;
const MAX_DEPTH: u8 = 16;
const MAX_NAME: usize = 255;

#[derive(Clone, Copy)]
pub(super) struct Cursor<'a>(pub(super) &'a [u8]);

impl<'a> Cursor<'a> {
    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let (value, rest) =
            self.0.split_at_checked(count).ok_or(Error::INVALID)?;
        self.0 = rest;
        Ok(value)
    }

    pub(super) fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn short(&mut self) -> Result<u16, Error> {
        let bytes = self.take(2)?.try_into().map_err(|_| Error::INVALID)?;
        Ok(u16::from_le_bytes(bytes))
    }

    pub(super) fn integer(&mut self) -> Result<u32, Error> {
        let bytes = self.take(4)?.try_into().map_err(|_| Error::INVALID)?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub(super) fn name(&mut self) -> Result<&'a str, Error> {
        let length = usize::from(self.short()?);
        if length == 0 {
            return Err(Error::INVALID);
        }
        bound(length, MAX_NAME)?;
        let name = self.take(length)?;
        if !name.is_ascii() {
            return Err(Error::INVALID);
        }
        str::from_utf8(name).map_err(|_| Error::INVALID)
    }

    pub(super) const fn finish(self) -> Result<(), Error> {
        if self.0.is_empty() { Ok(()) } else { Err(Error::INVALID) }
    }
}

pub(super) fn declaration(bytes: &[u8]) -> Result<Declaration, Error> {
    bound(bytes.len(), MAX_BYTES)?;
    let mut cursor = Cursor(bytes);
    let api = cursor.take(16)?.try_into().map_err(|_| Error::INVALID)?;
    if api == [0; 16] {
        return Err(Error::INVALID);
    }
    let count =
        usize::try_from(cursor.integer()?).map_err(|_| Error::PRESSURE)?;
    bound(count, MAX_ITEMS)?;
    let mut methods = Vec::new();
    methods.try_reserve_exact(count).map_err(|_| Error::PRESSURE)?;
    for _ in 0..count {
        push_method(&mut methods, method(&mut cursor)?)?;
    }
    cursor.finish()?;
    Ok(Declaration { api, methods, bytes: copy(bytes)? })
}

fn push_method(methods: &mut Vec<Method>, next: Method) -> Result<(), Error> {
    if methods.last().is_some_and(|old| old.operation >= next.operation)
        || methods.iter().any(|old| old.name == next.name)
    {
        return Err(Error::INVALID);
    }
    methods.push(next);
    Ok(())
}

fn method(cursor: &mut Cursor<'_>) -> Result<Method, Error> {
    let operation = operation(cursor.byte()?, cursor.integer()?)?;
    let dispatch = match cursor.byte()? {
        0 => Dispatch::Call,
        1 => Dispatch::Edit,
        2 => Dispatch::Factory,
        _ => return Err(Error::INVALID),
    };
    let access = cursor.integer()?;
    if access & !7 != 0 || (operation == Operation::Exec && access & 4 == 0) {
        return Err(Error::INVALID);
    }
    let activity = cursor.byte()?;
    if activity & !7 != 0 {
        return Err(Error::INVALID);
    }
    let name = owned_name(cursor.name()?)?;
    let input = take_type(cursor, true)?;
    let output = take_type(cursor, true)?;
    let progress = take_type(cursor, false)?;
    let created = match dispatch {
        Dispatch::Factory => Some(created_type(cursor)?),
        Dispatch::Call | Dispatch::Edit => None,
    };
    Ok(Method {
        operation,
        dispatch,
        access,
        activity,
        name,
        input,
        output,
        progress,
        created,
    })
}

fn created_type(cursor: &mut Cursor<'_>) -> Result<ValueType, Error> {
    if cursor.0.first() != Some(&12) {
        return Err(Error::INVALID);
    }
    take_type(cursor, true)
}

const fn operation(family: u8, number: u32) -> Result<Operation, Error> {
    match (family, number) {
        (1, 0) => Ok(Operation::Exec),
        (2, 1..) => Ok(Operation::Ioctl(number)),
        _ => Err(Error::INVALID),
    }
}

pub(super) fn value_type(bytes: &[u8]) -> Result<ValueType, Error> {
    bound(bytes.len(), MAX_BYTES)?;
    let mut cursor = Cursor(bytes);
    let value = take_type(&mut cursor, true)?;
    cursor.finish()?;
    Ok(value)
}

fn take_type(
    cursor: &mut Cursor<'_>,
    allow_references: bool,
) -> Result<ValueType, Error> {
    let start = cursor.0;
    grammar(cursor, 1, allow_references)?;
    Ok(ValueType(copy(&start[..start.len() - cursor.0.len()])?))
}

pub(super) fn grammar(
    cursor: &mut Cursor<'_>,
    depth: u8,
    allow_references: bool,
) -> Result<(), Error> {
    bound(usize::from(depth), usize::from(MAX_DEPTH))?;
    match cursor.byte()? {
        0..=6 | 13 => Ok(()),
        7 | 8 => cursor.integer().map(|_| ()),
        9 => fields(cursor, depth, false, allow_references),
        10 | 14 => {
            cursor.integer()?;
            grammar(cursor, depth + 1, allow_references)
        }
        11 => fields(cursor, depth, true, allow_references),
        12 if allow_references => reference(cursor),
        _ => Err(Error::INVALID),
    }
}

fn fields(
    cursor: &mut Cursor<'_>,
    depth: u8,
    choice: bool,
    allow_references: bool,
) -> Result<(), Error> {
    let count = usize::from(cursor.short()?);
    bound(count, MAX_ITEMS)?;
    let mut names = Vec::new();
    let mut tags = Vec::new();
    names.try_reserve_exact(count).map_err(|_| Error::PRESSURE)?;
    tags.try_reserve_exact(count).map_err(|_| Error::PRESSURE)?;
    for _ in 0..count {
        field(cursor, choice, &mut names, &mut tags)?;
        grammar(cursor, depth + 1, allow_references)?;
    }
    Ok(())
}

fn field<'a>(
    cursor: &mut Cursor<'a>,
    choice: bool,
    names: &mut Vec<&'a str>,
    tags: &mut Vec<u32>,
) -> Result<(), Error> {
    if choice {
        let tag = cursor.integer()?;
        if tags.contains(&tag) {
            return Err(Error::INVALID);
        }
        tags.push(tag);
    }
    let name = cursor.name()?;
    if names.contains(&name) {
        return Err(Error::INVALID);
    }
    names.push(name);
    Ok(())
}

fn reference(cursor: &mut Cursor<'_>) -> Result<(), Error> {
    cursor.take(16)?; // Zero permits any Resource; nonzero names an API.
    let access = cursor.integer()?;
    if access & !7 != 0 {
        return Err(Error::INVALID);
    }
    Ok(())
}

fn copy(bytes: &[u8]) -> Result<Box<[u8]>, Error> {
    let mut result = Vec::new();
    result.try_reserve_exact(bytes.len()).map_err(|_| Error::PRESSURE)?;
    result.extend_from_slice(bytes);
    Ok(result.into_boxed_slice())
}

fn owned_name(text: &str) -> Result<Box<str>, Error> {
    let mut name = String::new();
    name.try_reserve_exact(text.len()).map_err(|_| Error::PRESSURE)?;
    name.push_str(text);
    Ok(name.into_boxed_str())
}

const fn bound(actual: usize, maximum: usize) -> Result<(), Error> {
    if actual > maximum { Err(Error::PRESSURE) } else { Ok(()) }
}
