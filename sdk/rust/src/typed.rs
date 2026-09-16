//! Values as Rust types: a type knows its `Ty` and its bytes.

use alloc::string::String;
use alloc::vec::Vec;

use reovim_kernel_interface::codec::{Reader, put_str, put_u32};
use reovim_kernel_interface::{Error, Rd, Ty};

/// The declared type of a Rust type, for the canonical bytes. Not
/// for `String` and `Vec`: their bound is the declaration's.
pub trait Typed {
    fn ty() -> Ty;
}

/// Its bytes, appended.
pub trait Encode {
    fn encode(&self, out: &mut Vec<u8>);
}

/// One value from the front of a `Reader`; `None` when the bytes do
/// not hold one.
pub trait Decode: Sized {
    fn decode(r: &mut Reader<'_>) -> Option<Self>;
}

/// The bytes of `value`.
#[must_use]
pub fn encode<T: Encode + ?Sized>(value: &T) -> Vec<u8> {
    let mut out = Vec::new();
    value.encode(&mut out);
    out
}

/// `bytes` as one `T`.
///
/// # Errors
/// `Invalid` unless `bytes` is exactly one value.
pub fn decode<T: Decode>(bytes: &[u8]) -> Result<T, Error> {
    let mut r = Reader::new(bytes);
    let value = T::decode(&mut r).ok_or(Error::Invalid)?;
    if !r.done() {
        return Err(Error::Invalid);
    }
    Ok(value)
}

macro_rules! scalar {
    ($t:ty, $ty:expr, $read:ident, |$v:ident| $write:expr) => {
        impl Typed for $t {
            fn ty() -> Ty {
                $ty
            }
        }

        impl Encode for $t {
            fn encode(&self, out: &mut Vec<u8>) {
                let $v = self;
                out.extend_from_slice(&$write);
            }
        }

        impl Decode for $t {
            fn decode(r: &mut Reader<'_>) -> Option<Self> {
                r.$read()
            }
        }
    };
}

scalar!(bool, Ty::Bool, bool, |v| [u8::from(*v)]);
scalar!(u8, Ty::U8, u8, |v| [*v]);
scalar!(u32, Ty::U32, u32, |v| v.to_le_bytes());
scalar!(u64, Ty::U64, u64, |v| v.to_le_bytes());
scalar!(i64, Ty::I64, i64, |v| v.to_le_bytes());
scalar!(f64, Ty::F64, f64, |v| v.to_le_bytes());

impl Typed for () {
    fn ty() -> Ty {
        Ty::Unit
    }
}

impl Encode for () {
    fn encode(&self, _: &mut Vec<u8>) {}
}

impl Decode for () {
    fn decode(_: &mut Reader<'_>) -> Option<Self> {
        Some(())
    }
}

impl Typed for Rd {
    fn ty() -> Ty {
        Ty::Rd
    }
}

impl Encode for Rd {
    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.0.to_le_bytes());
    }
}

impl Decode for Rd {
    fn decode(r: &mut Reader<'_>) -> Option<Self> {
        r.u64().map(Self)
    }
}

impl Encode for String {
    fn encode(&self, out: &mut Vec<u8>) {
        put_str(out, self);
    }
}

impl Decode for String {
    fn decode(r: &mut Reader<'_>) -> Option<Self> {
        r.str()
    }
}

impl<T: Encode> Encode for Vec<T> {
    fn encode(&self, out: &mut Vec<u8>) {
        put_u32(out, u32::try_from(self.len()).unwrap_or(u32::MAX));
        for item in self {
            item.encode(out);
        }
    }
}

impl<T: Decode> Decode for Vec<T> {
    fn decode(r: &mut Reader<'_>) -> Option<Self> {
        (0..r.u32()?).map(|_| T::decode(r)).collect()
    }
}
