#![no_std]
#![forbid(unsafe_code)]
//! Optional CLI argument and path conventions, outside Kernel dispatch.
//!
//! Example: a command module can parse its declared byte argument payload here.
//! Invocations use bound declarations and SDK Request, never info text.

extern crate alloc;

use alloc::{string::String, vec::Vec};
use reovim_kernel_interface::{Error, Value};

/// Split a CLI convention's NUL-separated arguments; this is not an ABI codec.
/// Example: `arguments(b"a\0b")` returns two arguments, retaining their order.
/// # Errors
/// Invalid UTF-8 rejects; binary input requires a different declared grammar.
pub fn arguments(data: &[u8]) -> Result<Vec<&str>, Error> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    let text = core::str::from_utf8(data).map_err(|_| Error::INVALID)?;
    Ok(text.split('\0').collect())
}

/// Parse an optional single CLI argument, e.g. an omitted directory uses `.`.
/// # Errors
/// More than one argument or invalid UTF-8 rejects.
pub fn optional(data: &[u8], default: &str) -> Result<String, Error> {
    match arguments(data)?.as_slice() {
        [] => Ok(default.into()),
        [value] => Ok((*value).into()),
        _ => Err(Error::INVALID),
    }
}

/// Require canonical Unit without references, e.g. a no-argument command.
/// # Errors
/// Extra data or reference slots reject Invalid.
pub fn empty(input: &Value) -> Result<(), Error> {
    if input.data.is_empty() && input.references.is_empty() {
        Ok(())
    } else {
        Err(Error::INVALID)
    }
}

/// Normalize relative to the caller's CWD, returning an absolute locator.
///
/// Example: `path("/project/src", "../README") == "/project/README"`.
/// This lexical operation grants no access and does not follow Resource links.
#[must_use]
pub fn path(cwd: &str, value: &str) -> String {
    let base = if value.starts_with('/') { "" } else { cwd };
    let mut parts: Vec<_> = base.split('/').filter(|v| !v.is_empty()).collect();
    for part in value.split('/') {
        match part {
            "" | "." => (),
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let mut path = String::from("/");
    path.push_str(&parts.join("/"));
    path
}
