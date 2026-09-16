//! Text to and from the value encoding, by declared type: what a command
//! line gives a method and what its output reads as.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_kernel_interface::{Ty, value};

use crate::Fault;

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

/// `output` as text: nothing for `Unit`, a number and a newline for
/// `Rd`, `U64` and `U32`, the text and a newline for `Utf8`, the raw
/// bytes for anything else.
///
/// # Errors
/// `Wire` when `output` is not one value of `ty`.
pub fn show(ty: &Ty, output: &[u8]) -> Result<Vec<u8>, Fault> {
    let text = match ty {
        Ty::Unit => String::new(),
        Ty::Rd | Ty::U64 => {
            let bytes = output.try_into().map_err(|_| Fault::Wire)?;
            format!("{}\n", u64::from_le_bytes(bytes))
        }
        Ty::U32 => {
            let bytes = output.try_into().map_err(|_| Fault::Wire)?;
            format!("{}\n", u32::from_le_bytes(bytes))
        }
        Ty::Utf8(_) => {
            let text = output.get(4..).ok_or(Fault::Wire)?;
            format!("{}\n", String::from_utf8_lossy(text))
        }
        _ => return Ok(output.to_vec()),
    };
    Ok(text.into_bytes())
}
