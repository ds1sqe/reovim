//! Declarations: data, not code. An api name, a version and per method
//! a number, a name, an input type, an output type and the right it
//! needs. The canonical bytes are identical from any language and their
//! hash is the declaration's identity.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::codec::{Reader, put_str, put_u16, put_u32};
use crate::types::{Error, Hash, Rights};

/// Largest canonical form the Kernel stores.
pub const MAX_BYTES: usize = 64 * 1024;
/// Most methods in one declaration.
pub const MAX_METHODS: usize = 256;
/// Deepest type nesting.
pub const MAX_DEPTH: usize = 16;

/// The closed type tag set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Ty {
    /// No bytes.
    Unit,
    /// One byte, 0 or 1.
    Bool,
    /// One byte.
    U8,
    /// Four bytes, little endian.
    U32,
    /// Eight bytes, little endian.
    U64,
    /// Eight bytes, little endian, two's complement.
    I64,
    /// Eight bytes, IEEE 754 little endian.
    F64,
    /// u32 length then at most that many bytes.
    Bytes(u32),
    /// u32 length then at most that many bytes of UTF-8.
    Utf8(u32),
    /// Fields in order.
    Record(Vec<Field>),
    /// u32 count then at most that many items.
    Sequence(u32, Box<Self>),
    /// Exactly that many items.
    Array(u32, Box<Self>),
    /// u8 tag then that variant's payload.
    Choice(Vec<Field>),
    /// An rd id as u64; the callee opens it under its own rights.
    Rd,
}

/// A named part of a Record or Choice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Field {
    /// Field or variant name.
    pub name: String,
    /// Its type.
    pub ty: Ty,
}

/// One method.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Method {
    /// Number a caller uses; unique inside the declaration.
    pub number: u16,
    /// Name for language helpers.
    pub name: String,
    /// Input type.
    pub input: Ty,
    /// Output type.
    pub output: Ty,
    /// The right the handle must hold.
    pub right: Rights,
}

/// One declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decl {
    /// Api name.
    pub api: String,
    /// Version; new bytes are a new hash either way.
    pub ver: u32,
    /// Methods in any order; canonical form sorts by number.
    pub methods: Vec<Method>,
}

impl Ty {
    const fn tag(&self) -> u8 {
        match self {
            Self::Unit => 0,
            Self::Bool => 1,
            Self::U8 => 2,
            Self::U32 => 3,
            Self::U64 => 4,
            Self::I64 => 5,
            Self::F64 => 6,
            Self::Bytes(_) => 7,
            Self::Utf8(_) => 8,
            Self::Record(_) => 9,
            Self::Sequence(..) => 10,
            Self::Array(..) => 11,
            Self::Choice(_) => 12,
            Self::Rd => 13,
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.push(self.tag());
        match self {
            Self::Bytes(max) | Self::Utf8(max) => put_u32(out, *max),
            Self::Record(fields) | Self::Choice(fields) => {
                put_fields(out, fields);
            }
            Self::Sequence(n, item) | Self::Array(n, item) => {
                put_u32(out, *n);
                item.write(out);
            }
            _ => {}
        }
    }

    /// One type from the canonical form, at most `MAX_DEPTH` deep.
    fn read(r: &mut Reader, depth: usize) -> Option<Self> {
        if depth > MAX_DEPTH {
            return None;
        }
        let ty = match r.u8()? {
            0 => Self::Unit,
            1 => Self::Bool,
            2 => Self::U8,
            3 => Self::U32,
            4 => Self::U64,
            5 => Self::I64,
            6 => Self::F64,
            7 => Self::Bytes(r.u32()?),
            8 => Self::Utf8(r.u32()?),
            9 => Self::Record(read_fields(r, depth)?),
            10 => Self::Sequence(r.u32()?, Box::new(Self::read(r, depth + 1)?)),
            11 => Self::Array(r.u32()?, Box::new(Self::read(r, depth + 1)?)),
            12 => Self::Choice(read_fields(r, depth)?),
            13 => Self::Rd,
            _ => return None,
        };
        Some(ty)
    }

    fn depth(&self) -> usize {
        match self {
            Self::Record(fields) | Self::Choice(fields) => {
                1 + fields.iter().map(|f| f.ty.depth()).max().unwrap_or(0)
            }
            Self::Sequence(_, item) | Self::Array(_, item) => 1 + item.depth(),
            _ => 1,
        }
    }
}

fn put_fields(out: &mut Vec<u8>, fields: &[Field]) {
    put_u32(out, u32::try_from(fields.len()).unwrap_or(u32::MAX));
    for field in fields {
        put_str(out, &field.name);
        field.ty.write(out);
    }
}

fn read_fields(r: &mut Reader, depth: usize) -> Option<Vec<Field>> {
    let n = r.u32()?;
    (0..n)
        .map(|_| Some(Field { name: r.str()?, ty: Ty::read(r, depth + 1)? }))
        .collect()
}

impl Method {
    fn read(r: &mut Reader) -> Option<Self> {
        Some(Self {
            number: r.u16()?,
            name: r.str()?,
            input: Ty::read(r, 1)?,
            output: Ty::read(r, 1)?,
            right: Rights::from_bits(r.u32()?)?,
        })
    }

    fn is_bounded(&self) -> bool {
        let deep = self.input.depth().max(self.output.depth());
        let one_right =
            self.right == Rights::READ || self.right == Rights::WRITE;
        deep <= MAX_DEPTH && one_right
    }
}

impl Decl {
    /// The bytes every language produces for this declaration.
    #[must_use]
    pub fn canonical(&self) -> Vec<u8> {
        let mut methods: Vec<&Method> = self.methods.iter().collect();
        methods.sort_by_key(|m| m.number);
        let mut out = Vec::new();
        put_str(&mut out, &self.api);
        put_u32(&mut out, self.ver);
        put_u32(&mut out, u32::try_from(methods.len()).unwrap_or(u32::MAX));
        for method in methods {
            put_u16(&mut out, method.number);
            put_str(&mut out, &method.name);
            method.input.write(&mut out);
            method.output.write(&mut out);
            put_u32(&mut out, method.right.bits());
        }
        out
    }

    /// The declaration whose canonical bytes these are.
    ///
    /// # Errors
    /// `Invalid` unless `bytes` is exactly one canonical form that passes
    /// `check`.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let mut r = Reader::new(bytes);
        let decl = Self::read(&mut r).ok_or(Error::Invalid)?;
        if !r.done() {
            return Err(Error::Invalid);
        }
        decl.check()?;
        Ok(decl)
    }

    fn read(r: &mut Reader) -> Option<Self> {
        let api = r.str()?;
        let ver = r.u32()?;
        let n = r.u32()?;
        let methods = (0..n).map(|_| Method::read(r)).collect::<Option<_>>()?;
        Some(Self { api, ver, methods })
    }

    /// The identity: FNV-1a 64 over the canonical bytes.
    #[must_use]
    pub fn hash(&self) -> Hash {
        fnv1a(&self.canonical())
    }

    /// Bounds and uniqueness.
    ///
    /// # Errors
    /// `Invalid` when a bound is exceeded, a method number repeats or a
    /// method names no right or both.
    pub fn check(&self) -> Result<(), Error> {
        if self.methods.len() > MAX_METHODS {
            return Err(Error::Invalid);
        }
        let mut numbers: Vec<u16> =
            self.methods.iter().map(|m| m.number).collect();
        numbers.sort_unstable();
        if numbers.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::Invalid);
        }
        if !self.methods.iter().all(Method::is_bounded) {
            return Err(Error::Invalid);
        }
        if self.canonical().len() > MAX_BYTES {
            return Err(Error::Invalid);
        }
        Ok(())
    }

    /// The method with that number, if declared.
    #[must_use]
    pub fn method(&self, number: u16) -> Option<&Method> {
        self.methods.iter().find(|m| m.number == number)
    }
}

/// FNV-1a, 64 bits. Fast and non-cryptographic; dispatch is one probe.
#[must_use]
pub fn fnv1a(bytes: &[u8]) -> Hash {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Hash(hash)
}
