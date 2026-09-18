//! The SDK's dynamic value form, its failures as the `Fault`s a
//! command line reports: an argument that does not fit is `Usage`, an
//! answer that does not fit is `Wire`.

use alloc::format;
use alloc::vec::Vec;

use reovim_kernel_interface::Ty;
pub use reovim_sdk::dynamic::Value;

use crate::Fault;

/// `value` in the encoding of `ty`; see [`reovim_sdk::dynamic::encode`].
///
/// # Errors
/// `Usage` when `value` is not one value of `ty`, so nothing is sent.
pub fn encode(ty: &Ty, value: &Value) -> Result<Vec<u8>, Fault> {
    reovim_sdk::dynamic::encode(ty, value)
        .map_err(|_| Fault::Usage(format!("value does not fit {ty:?}")))
}

/// `arg` in the encoding of `ty`; see [`reovim_sdk::dynamic::encode_arg`].
///
/// # Errors
/// `Usage` when `arg` is not one value of `ty`, so nothing is sent.
pub fn encode_arg(ty: &Ty, arg: Option<&str>) -> Result<Vec<u8>, Fault> {
    reovim_sdk::dynamic::encode_arg(ty, arg)
        .map_err(|_| Fault::Usage(format!("argument does not fit {ty:?}")))
}

/// `bytes` as one value of `ty`.
///
/// # Errors
/// `Wire` unless `bytes` is exactly one value of `ty`.
pub fn decode(ty: &Ty, bytes: &[u8]) -> Result<Value, Fault> {
    reovim_sdk::dynamic::decode(ty, bytes).map_err(|_| Fault::Wire)
}

/// `output` as text; see [`reovim_sdk::dynamic::show`].
///
/// # Errors
/// `Wire` when `output` is not one value of `ty`.
pub fn show(ty: &Ty, output: &[u8]) -> Result<Vec<u8>, Fault> {
    reovim_sdk::dynamic::show(ty, output).map_err(|_| Fault::Wire)
}
