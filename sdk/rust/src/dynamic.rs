//! Values by a `Ty` known at runtime: what a command line gives a
//! method and what its output reads as, for any api.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use reovim_kernel_interface::codec::{Reader, put_bytes};
use reovim_kernel_interface::{Error, Field, Rd, Ty, value};

/// One decoded value, shaped by its `Ty`.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Unit,
    Bool(bool),
    U8(u8),
    U32(u32),
    U64(u64),
    I64(i64),
    F64(f64),
    Bytes(Vec<u8>),
    Utf8(String),
    Record(Vec<(String, Self)>),
    Sequence(Vec<Self>),
    Choice(String, Box<Self>),
    Rd(Rd),
}

impl Value {
    /// The field named `name` of a Record.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Self> {
        let Self::Record(fields) = self else { return None };
        fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    /// The number a `U64`, `U32`, `U8` or `Rd` holds.
    #[must_use]
    pub const fn number(&self) -> Option<u64> {
        match self {
            Self::U8(n) => Some(*n as u64),
            Self::U32(n) => Some(*n as u64),
            Self::U64(n) | Self::Rd(Rd(n)) => Some(*n),
            _ => None,
        }
    }
}

/// `arg` in the encoding of `ty`: `Unit` takes none, `Rd`, `U64` and
/// `U32` a number, `Utf8` and `Bytes` the text.
///
/// # Errors
/// `Invalid` when `arg` is not one value of `ty`.
pub fn encode_arg(ty: &Ty, arg: Option<&str>) -> Result<Vec<u8>, Error> {
    let number = || arg.and_then(|a| a.parse::<u64>().ok());
    let bytes = match (ty, arg) {
        (Ty::Unit, None) => Some(Vec::new()),
        (Ty::Rd | Ty::U64, Some(_)) => {
            number().map(|n| n.to_le_bytes().to_vec())
        }
        (Ty::U32, Some(_)) => number()
            .and_then(|n| u32::try_from(n).ok())
            .map(|n| n.to_le_bytes().to_vec()),
        (Ty::Utf8(_) | Ty::Bytes(_), Some(text)) => {
            let mut out = Vec::new();
            put_bytes(&mut out, text.as_bytes());
            Some(out)
        }
        _ => None,
    };
    let bytes = bytes.ok_or(Error::Invalid)?;
    if !value::fits(&bytes, ty) {
        return Err(Error::Invalid);
    }
    Ok(bytes)
}

/// `bytes` as one value of `ty`.
///
/// # Errors
/// `Invalid` unless `bytes` is exactly one value of `ty`.
pub fn decode(ty: &Ty, bytes: &[u8]) -> Result<Value, Error> {
    if !value::fits(bytes, ty) {
        return Err(Error::Invalid);
    }
    take(ty, &mut Reader::new(bytes)).ok_or(Error::Invalid)
}

/// One value of `ty`, already known to fit.
fn take(ty: &Ty, r: &mut Reader<'_>) -> Option<Value> {
    let value = match ty {
        Ty::Unit => Value::Unit,
        Ty::Bool => Value::Bool(r.bool()?),
        Ty::U8 => Value::U8(r.u8()?),
        Ty::U32 => Value::U32(r.u32()?),
        Ty::U64 => Value::U64(r.u64()?),
        Ty::I64 => Value::I64(r.i64()?),
        Ty::F64 => Value::F64(r.f64()?),
        Ty::Rd => Value::Rd(Rd(r.u64()?)),
        Ty::Bytes(_) => Value::Bytes(r.bytes()?),
        Ty::Utf8(_) => Value::Utf8(r.str()?),
        Ty::Record(fields) => Value::Record(record(fields, r)?),
        Ty::Sequence(_, item) => {
            let n = r.u32()?;
            Value::Sequence(items(n, item, r)?)
        }
        Ty::Array(n, item) => Value::Sequence(items(*n, item, r)?),
        Ty::Choice(fields) => {
            let field = fields.get(usize::from(r.u8()?))?;
            Value::Choice(field.name.clone(), Box::new(take(&field.ty, r)?))
        }
    };
    Some(value)
}

fn record(
    fields: &[Field],
    r: &mut Reader<'_>,
) -> Option<Vec<(String, Value)>> {
    fields.iter().map(|f| Some((f.name.clone(), take(&f.ty, r)?))).collect()
}

fn items(n: u32, item: &Ty, r: &mut Reader<'_>) -> Option<Vec<Value>> {
    (0..n).map(|_| take(item, r)).collect()
}

/// One line: a scalar as its number or text, a Record as `{a=1 b=x}`,
/// a Sequence as `[1 2]`, a Choice as `name(value)`.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit => Ok(()),
            Self::Bool(b) => write!(f, "{b}"),
            Self::U8(n) => write!(f, "{n}"),
            Self::U32(n) => write!(f, "{n}"),
            Self::U64(n) | Self::Rd(Rd(n)) => write!(f, "{n}"),
            Self::I64(n) => write!(f, "{n}"),
            Self::F64(n) => write!(f, "{n}"),
            Self::Bytes(bytes) => {
                write!(f, "{}", String::from_utf8_lossy(bytes))
            }
            Self::Utf8(text) => write!(f, "{text}"),
            Self::Record(fields) => {
                let pairs =
                    fields.iter().map(|(n, v)| alloc::format!("{n}={v}"));
                list(f, "{", pairs, "}")
            }
            Self::Sequence(items) => list(f, "[", items.iter(), "]"),
            Self::Choice(name, value) => write!(f, "{name}({value})"),
        }
    }
}

/// `items` between `open` and `close`, one space apart.
fn list<T: fmt::Display>(
    f: &mut fmt::Formatter<'_>,
    open: &str,
    items: impl Iterator<Item = T>,
    close: &str,
) -> fmt::Result {
    f.write_str(open)?;
    for (i, item) in items.enumerate() {
        let space = if i == 0 { "" } else { " " };
        write!(f, "{space}{item}")?;
    }
    f.write_str(close)
}

/// `output` as text: nothing for `Unit`, raw for `Bytes`, otherwise
/// the value on one line.
///
/// # Errors
/// `Invalid` when `output` is not one value of `ty`.
pub fn show(ty: &Ty, output: &[u8]) -> Result<Vec<u8>, Error> {
    let text = match decode(ty, output)? {
        Value::Unit => String::new(),
        Value::Bytes(bytes) => return Ok(bytes),
        value => alloc::format!("{value}\n"),
    };
    Ok(text.into_bytes())
}
