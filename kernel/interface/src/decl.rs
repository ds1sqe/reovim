//! Declarations: data, not code. An api name, a version, per method
//! a number, a name, an input type, an output type and the right it
//! needs, and the records the declaration names. The canonical bytes
//! are identical from any language and their hash is the
//! declaration's identity.

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
    /// [`Self::Utf8`]'s bytes and bound, under a tag of its own: the
    /// field a shell fills with the rest of the line it was given,
    /// from where the field's own value begins to the end, verbatim —
    /// quotes, blanks, `|` and `>` all part of the value. A record
    /// holds at most one and it is the record's last field, which
    /// [`Decl::check`] refuses a declaration for breaking.
    Rest(u32),
    /// Qualified type name and fields in order.
    Record(String, Vec<Field>),
    /// u32 count then at most that many items.
    Sequence(u32, Box<Self>),
    /// Exactly that many items.
    Array(u32, Box<Self>),
    /// Qualified type name; u8 tag then that variant's payload.
    Choice(String, Vec<Field>),
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
    /// Written input label, absent when the method takes no input.
    pub input_name: Option<String>,
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
    /// The records the declaration names, each with its fields, in
    /// the order declared.
    ///
    /// A record reaches a method as its input or output type, so the
    /// methods alone would carry every shape a caller sends — except
    /// where there is no method: a record-only api (`keyboard`,
    /// `resolver`) is its records and nothing else, and without them
    /// its identity would be its name and version alone.
    pub records: Vec<(String, Vec<Field>)>,
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
            Self::Record(..) => 9,
            Self::Sequence(..) => 10,
            Self::Array(..) => 11,
            Self::Choice(..) => 12,
            Self::Rd => 13,
            Self::Rest(_) => 14,
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.push(self.tag());
        match self {
            Self::Bytes(max) | Self::Utf8(max) | Self::Rest(max) => {
                put_u32(out, *max);
            }
            Self::Record(name, fields) | Self::Choice(name, fields) => {
                put_str(out, name);
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
            9 => Self::Record(r.str()?, read_fields(r, depth)?),
            10 => Self::Sequence(r.u32()?, Box::new(Self::read(r, depth + 1)?)),
            11 => Self::Array(r.u32()?, Box::new(Self::read(r, depth + 1)?)),
            12 => Self::Choice(r.str()?, read_fields(r, depth)?),
            13 => Self::Rd,
            14 => Self::Rest(r.u32()?),
            _ => return None,
        };
        Some(ty)
    }

    fn depth(&self) -> usize {
        match self {
            Self::Record(_, fields) | Self::Choice(_, fields) => {
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
    fn write(&self, out: &mut Vec<u8>) {
        put_u16(out, self.number);
        put_str(out, &self.name);
        out.push(u8::from(self.input_name.is_some()));
        if let Some(name) = &self.input_name {
            put_str(out, name);
        }
        self.input.write(out);
        self.output.write(out);
        put_u32(out, self.right.bits());
    }

    fn read(r: &mut Reader) -> Option<Self> {
        Some(Self {
            number: r.u16()?,
            name: r.str()?,
            input_name: if r.bool()? { Some(r.str()?) } else { None },
            input: Ty::read(r, 1)?,
            output: Ty::read(r, 1)?,
            right: Rights::from_bits(r.u32()?)?,
        })
    }

    /// Whether every `Rest` this method's types hold stands where one
    /// may: the last field of a record, and nowhere else.
    fn places_rest(&self) -> bool {
        placed(&self.input) && placed(&self.output)
    }

    fn is_bounded(&self) -> bool {
        let deep = self.input.depth().max(self.output.depth());
        let one_right =
            self.right == Rights::READ || self.right == Rights::WRITE;
        deep <= MAX_DEPTH && one_right
    }
}

/// Whether `ty` puts every `Rest` it holds where one may stand: the
/// last field of a record. A `Rest` is the rest of a line, so a line
/// has one and ends with it — anywhere else (before another field, in
/// a sequence, in a choice, as a whole input) there is no "rest" for
/// it to be, and the declaration is refused.
fn placed(ty: &Ty) -> bool {
    if matches!(ty, Ty::Rest(_)) {
        return false;
    }
    inner(ty)
}

/// `placed` for a type that may itself be a `Rest`: the last field of
/// a record is the one place one stands.
fn inner(ty: &Ty) -> bool {
    match ty {
        Ty::Record(_, fields) => {
            fields.split_last().is_none_or(|(last, rest)| {
                rest.iter().all(|field| placed(&field.ty)) && inner(&last.ty)
            })
        }
        Ty::Choice(_, fields) => fields.iter().all(|field| placed(&field.ty)),
        Ty::Sequence(_, item) | Ty::Array(_, item) => placed(item),
        _ => true,
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
            method.write(&mut out);
        }
        let records = u32::try_from(self.records.len());
        put_u32(&mut out, records.unwrap_or(u32::MAX));
        for (name, fields) in &self.records {
            put_str(&mut out, name);
            put_fields(&mut out, fields);
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
        let n = r.u32()?;
        let record = |_| Some((r.str()?, read_fields(r, 1)?));
        let records = (0..n).map(record).collect::<Option<_>>()?;
        Some(Self { api, ver, methods, records })
    }

    /// The identity: FNV-1a 64 over the canonical bytes.
    #[must_use]
    pub fn hash(&self) -> Hash {
        fnv1a(&self.canonical())
    }

    /// Bounds and uniqueness.
    ///
    /// # Errors
    /// `Invalid` when a bound is exceeded, a method number repeats, a
    /// method names no right or both, or a `Rest` stands anywhere but
    /// last in a record.
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
        if !self.methods.iter().all(Method::places_rest) {
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
