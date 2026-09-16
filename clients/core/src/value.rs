//! Text to and from the value encoding, by declared type: what a command
//! line gives a method and what its output reads as.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use reovim_kernel_interface::{Field, Rd, Ty, value};

use crate::Fault;

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
/// `U32` a number, `Utf8` and `Bytes` the text. A missing or unfit
/// argument is `Usage`, so nothing is sent.
///
/// # Errors
/// `Usage` when `arg` is not one value of `ty`.
pub fn encode_arg(ty: &Ty, arg: Option<&str>) -> Result<Vec<u8>, Fault> {
    let bad = || Fault::Usage(format!("argument does not fit {ty:?}"));
    let number = || arg.ok_or_else(bad)?.parse::<u64>().map_err(|_| bad());
    let bytes = match (ty, arg) {
        (Ty::Unit, None) => Vec::new(),
        (Ty::Rd | Ty::U64, Some(_)) => number()?.to_le_bytes().to_vec(),
        (Ty::U32, Some(_)) => {
            u32::try_from(number()?).map_err(|_| bad())?.to_le_bytes().to_vec()
        }
        (Ty::Utf8(_) | Ty::Bytes(_), Some(text)) => {
            let len = u32::try_from(text.len()).map_err(|_| bad())?;
            let mut out = len.to_le_bytes().to_vec();
            out.extend_from_slice(text.as_bytes());
            out
        }
        _ => return Err(bad()),
    };
    if !value::fits(&bytes, ty) {
        return Err(bad());
    }
    Ok(bytes)
}

/// `bytes` as one value of `ty`.
///
/// # Errors
/// `Wire` unless `bytes` is exactly one value of `ty`.
pub fn decode(ty: &Ty, bytes: &[u8]) -> Result<Value, Fault> {
    if !value::fits(bytes, ty) {
        return Err(Fault::Wire);
    }
    let mut at = 0;
    take(ty, bytes, &mut at).ok_or(Fault::Wire)
}

/// One value of `ty` at `at`, already known to fit.
fn take(ty: &Ty, bytes: &[u8], at: &mut usize) -> Option<Value> {
    let value = match ty {
        Ty::Unit => Value::Unit,
        Ty::Bool => Value::Bool(fixed::<1>(bytes, at)?[0] == 1),
        Ty::U8 => Value::U8(fixed::<1>(bytes, at)?[0]),
        Ty::U32 => Value::U32(u32::from_le_bytes(fixed(bytes, at)?)),
        Ty::U64 => Value::U64(u64::from_le_bytes(fixed(bytes, at)?)),
        Ty::I64 => Value::I64(i64::from_le_bytes(fixed(bytes, at)?)),
        Ty::F64 => Value::F64(f64::from_le_bytes(fixed(bytes, at)?)),
        Ty::Rd => Value::Rd(Rd(u64::from_le_bytes(fixed(bytes, at)?))),
        Ty::Bytes(_) => Value::Bytes(blob(bytes, at)?),
        Ty::Utf8(_) => Value::Utf8(String::from_utf8(blob(bytes, at)?).ok()?),
        Ty::Record(fields) => Value::Record(record(fields, bytes, at)?),
        Ty::Sequence(_, item) => {
            let n = u32::from_le_bytes(fixed(bytes, at)?);
            Value::Sequence(items(n, item, bytes, at)?)
        }
        Ty::Array(n, item) => Value::Sequence(items(*n, item, bytes, at)?),
        Ty::Choice(fields) => {
            let field = fields.get(usize::from(fixed::<1>(bytes, at)?[0]))?;
            let value = take(&field.ty, bytes, at)?;
            Value::Choice(field.name.clone(), Box::new(value))
        }
    };
    Some(value)
}

fn fixed<const N: usize>(bytes: &[u8], at: &mut usize) -> Option<[u8; N]> {
    let out = bytes.get(*at..*at + N)?.try_into().ok()?;
    *at += N;
    Some(out)
}

fn blob(bytes: &[u8], at: &mut usize) -> Option<Vec<u8>> {
    let len = usize::try_from(u32::from_le_bytes(fixed(bytes, at)?)).ok()?;
    let out = bytes.get(*at..*at + len)?.to_vec();
    *at += len;
    Some(out)
}

fn record(
    fields: &[Field],
    bytes: &[u8],
    at: &mut usize,
) -> Option<Vec<(String, Value)>> {
    fields
        .iter()
        .map(|f| Some((f.name.clone(), take(&f.ty, bytes, at)?)))
        .collect()
}

fn items(
    n: u32,
    item: &Ty,
    bytes: &[u8],
    at: &mut usize,
) -> Option<Vec<Value>> {
    (0..n).map(|_| take(item, bytes, at)).collect()
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
                let pairs = fields.iter().map(|(n, v)| format!("{n}={v}"));
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
/// `Wire` when `output` is not one value of `ty`.
pub fn show(ty: &Ty, output: &[u8]) -> Result<Vec<u8>, Fault> {
    let text = match decode(ty, output)? {
        Value::Unit => String::new(),
        Value::Bytes(bytes) => return Ok(bytes),
        value => format!("{value}\n"),
    };
    Ok(text.into_bytes())
}
