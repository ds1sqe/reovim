use super::{ValueType, decode::Cursor};
use crate::Error;

pub(super) fn bound(value: &ValueType, limit: usize) -> usize {
    walk(&mut Cursor(value.bytes()), limit).expect("validated value grammar")
}

fn walk(cursor: &mut Cursor<'_>, limit: usize) -> Result<usize, Error> {
    match cursor.byte()? {
        0..=6 | 13 => Ok(0),
        7 | 8 => cursor.integer().map(|_| 0),
        9 => fields(cursor, limit, false),
        11 => fields(cursor, limit, true),
        10 | 14 => {
            let count =
                usize::try_from(cursor.integer()?).unwrap_or(usize::MAX);
            Ok(walk(cursor, limit)?.saturating_mul(count).min(limit))
        }
        12 => {
            cursor.take(20)?;
            Ok(limit.min(1))
        }
        _ => Err(Error::INVALID),
    }
}

fn fields(
    cursor: &mut Cursor<'_>,
    limit: usize,
    choice: bool,
) -> Result<usize, Error> {
    let count = cursor.short()?;
    let mut total: usize = 0;
    for _ in 0..count {
        if choice {
            cursor.integer()?;
        }
        cursor.name()?;
        let next = walk(cursor, limit)?;
        total = if choice {
            total.max(next)
        } else {
            total.saturating_add(next).min(limit)
        };
    }
    Ok(total)
}
