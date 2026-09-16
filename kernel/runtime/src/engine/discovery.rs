use alloc::{boxed::Box, vec::Vec};
use reovim_kernel_interface::{
    Error, Outcome, Value, declaration::Declaration,
};

use super::{access::Scope, codec};
use reovim_kernel_interface::resource::AccessId;

pub(super) fn interfaces(
    scope: &Scope,
    subject: AccessId,
    data: &[u8],
) -> Result<Outcome, Error> {
    let (cursor, limit) = data.split_at_checked(8).ok_or(Error::INVALID)?;
    let cursor = usize::try_from(codec::u64_value(cursor)?)
        .map_err(|_| Error::INVALID)?;
    let limit = usize::try_from(codec::u64_value(limit)?)
        .unwrap_or(usize::MAX)
        .min(scope.limits.values.bytes);
    let offered = &scope.live(subject)?.implementation.interfaces;
    let remaining = offered.get(cursor..).ok_or(Error::INVALID)?;
    let elements = scope.limits.values.elements;
    if limit < 12 || elements < 3 {
        return Err(Error::PRESSURE);
    }
    let page = select(remaining, limit - 12, elements - 3)?;
    let next =
        if page.len() == remaining.len() { 0 } else { cursor + page.len() };
    let size = page.iter().map(|d| 4 + d.bytes().len()).sum::<usize>() + 12;
    let mut output = Vec::new();
    output.try_reserve_exact(size).map_err(|_| Error::PRESSURE)?;
    let count = u32::try_from(page.len()).map_err(|_| Error::PRESSURE)?;
    output.extend_from_slice(&count.to_le_bytes());
    for declaration in page {
        append(&mut output, declaration)?;
    }
    let next = u64::try_from(next).map_err(|_| Error::PRESSURE)?;
    output.extend_from_slice(&next.to_le_bytes());
    Ok(Outcome::Ok(Value {
        data: output.into_boxed_slice(),
        references: Box::new([]),
    }))
}

fn select(
    offered: &[Declaration],
    mut bytes: usize,
    elements: usize,
) -> Result<&[Declaration], Error> {
    let mut count = 0;
    for declaration in offered.iter().take(elements.min(256)) {
        let size = 4 + declaration.bytes().len();
        if size > bytes {
            break;
        }
        bytes -= size;
        count += 1;
    }
    if count == 0 && !offered.is_empty() {
        return Err(Error::PRESSURE);
    }
    Ok(&offered[..count])
}

pub(super) fn append(
    output: &mut Vec<u8>,
    declaration: &Declaration,
) -> Result<(), Error> {
    let bytes = declaration.bytes();
    let size = u32::try_from(bytes.len()).map_err(|_| Error::PRESSURE)?;
    output.extend_from_slice(&size.to_le_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}
