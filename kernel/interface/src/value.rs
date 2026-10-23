//! The one value encoding a declared type describes. `fits` is the
//! Kernel's input and output checks and the shape a wire endpoint encodes.

use crate::decl::{Field, Ty};

/// `true` when `bytes` is exactly one value of `ty`.
#[must_use]
pub fn fits(bytes: &[u8], ty: &Ty) -> bool {
    take(bytes, ty) == Some(bytes.len())
}

/// Bytes one value of `ty` occupies at the front of `bytes`, if valid.
fn take(bytes: &[u8], ty: &Ty) -> Option<usize> {
    match ty {
        Ty::Unit => Some(0),
        Ty::Bool => take_bool(bytes),
        Ty::U8 => fixed(bytes, 1),
        Ty::U32 => fixed(bytes, 4),
        Ty::U64 | Ty::I64 | Ty::F64 | Ty::Rd => fixed(bytes, 8),
        Ty::Bytes(max) => take_blob(bytes, *max, false),
        // A `Rest` is a `Utf8` with a tag of its own: one value of
        // either is the same bytes, bound and UTF-8 check included.
        Ty::Utf8(max) | Ty::Rest(max) => take_blob(bytes, *max, true),
        Ty::Record(_, fields) => take_record(bytes, fields),
        Ty::Sequence(max, item) => take_items(bytes, *max, item, true),
        Ty::Array(n, item) => take_items(bytes, *n, item, false),
        Ty::Choice(_, fields) => take_choice(bytes, fields),
    }
}

fn fixed(bytes: &[u8], len: usize) -> Option<usize> {
    (bytes.len() >= len).then_some(len)
}

fn take_bool(bytes: &[u8]) -> Option<usize> {
    matches!(bytes.first(), Some(0 | 1)).then_some(1)
}

fn take_u32(bytes: &[u8]) -> Option<u32> {
    let head: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
    Some(u32::from_le_bytes(head))
}

fn take_blob(bytes: &[u8], max: u32, utf8: bool) -> Option<usize> {
    let len = take_u32(bytes)?;
    if len > max {
        return None;
    }
    let len = len as usize;
    let body = bytes.get(4..4 + len)?;
    if utf8 && core::str::from_utf8(body).is_err() {
        return None;
    }
    Some(4 + len)
}

fn take_record(bytes: &[u8], fields: &[Field]) -> Option<usize> {
    let mut at = 0;
    for field in fields {
        at += take(bytes.get(at..)?, &field.ty)?;
    }
    Some(at)
}

fn take_items(
    bytes: &[u8],
    count: u32,
    item: &Ty,
    counted: bool,
) -> Option<usize> {
    let (mut at, n) = if counted {
        let n = take_u32(bytes)?;
        if n > count {
            return None;
        }
        (4, n)
    } else {
        (0, count)
    };
    for _ in 0..n {
        at += take(bytes.get(at..)?, item)?;
    }
    Some(at)
}

fn take_choice(bytes: &[u8], fields: &[Field]) -> Option<usize> {
    let tag = usize::from(*bytes.first()?);
    let field = fields.get(tag)?;
    Some(1 + take(bytes.get(1..)?, &field.ty)?)
}
