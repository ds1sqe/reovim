//! Every typed value round trips and is the one value its `Ty` names.

use reovim_kernel_interface::{Rd, Ty, value};
use reovim_sdk::{Decode, Encode, Typed, decode, encode};

fn round<T: Encode + Decode + PartialEq + std::fmt::Debug>(value: T, ty: &Ty) {
    let bytes = encode(&value);
    assert!(value::fits(&bytes, ty), "{value:?} as {ty:?}");
    assert_eq!(decode::<T>(&bytes), Ok(value));
}

#[test]
fn scalars_round_trip_as_their_ty() {
    round((), &<() as Typed>::ty());
    round(true, &bool::ty());
    round(7u8, &u8::ty());
    round(70_000u32, &u32::ty());
    round(u64::MAX, &u64::ty());
    round(-2i64, &i64::ty());
    round(1.5f64, &f64::ty());
    round(Rd(3), &Rd::ty());
    assert_eq!(bool::ty(), Ty::Bool);
    assert_eq!(Rd::ty(), Ty::Rd);
}

#[test]
fn strings_and_sequences_round_trip() {
    round(String::from("héllo"), &Ty::Utf8(16));
    round(vec![1u64, 2, 3], &Ty::Sequence(4, Box::new(Ty::U64)));
    round(
        vec![String::from("a"), String::new()],
        &Ty::Sequence(2, Box::new(Ty::Utf8(1))),
    );
    round(Vec::<Rd>::new(), &Ty::Sequence(0, Box::new(Ty::Rd)));
}

#[test]
fn short_and_long_bytes_are_invalid() {
    assert!(decode::<u64>(&[1, 2, 3]).is_err());
    assert!(decode::<u8>(&[1, 2]).is_err(), "a byte left over");
    assert!(decode::<bool>(&[2]).is_err());
    let mut text = 5u32.to_le_bytes().to_vec();
    text.extend_from_slice(b"abc");
    assert!(decode::<String>(&text).is_err(), "length past the end");
}
