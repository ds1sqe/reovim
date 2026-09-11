#![no_std]
#![forbid(unsafe_code)]
//! Optional info convention shared by the CLI and loaded call modules.
//! Kernel names and request dispatch do not depend on this convention.

extern crate alloc;

use alloc::boxed::Box;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Encoding {
    Utf8,
    Arguments,
}

/// Client presentation of a successfully returned value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Output {
    Message,
    Bytes,
    Close,
}

pub struct DefaultCall {
    pub contract: Contract,
    pub encoding: Encoding,
    pub output: Output,
}

impl DefaultCall {
    /// Reads an ordinary info value; unrelated usage lines are passive prose.
    /// ```
    /// use reovim_call::DefaultCall;
    /// let info = b"write\tcommand.ls\t1\targs\noutput\tbytes\n";
    /// let call = DefaultCall::parse(info)?;
    /// // An omitted argument list is one complete empty request.
    /// assert!(call.data(&[])?.is_empty());
    /// # Ok::<(), reovim_kernel_interface::Error>(())
    /// ```
    /// # Errors
    /// Rejects missing/ambiguous calls, versions or unsupported encodings.
    pub fn parse(info: &[u8]) -> Result<Self, Error> {
        let text = core::str::from_utf8(info).map_err(|_| Error::INVALID)?;
        let mut calls =
            text.lines().filter_map(|line| line.strip_prefix("write\t"));
        let call = calls.next().ok_or(Error::UNSUPPORTED)?;
        if calls.next().is_some() {
            return Err(Error::INVALID);
        }
        let mut fields = call.split('\t');
        let name = fields
            .next()
            .filter(|name| !name.is_empty())
            .ok_or(Error::INVALID)?;
        let version = fields
            .next()
            .ok_or(Error::INVALID)?
            .parse()
            .map_err(|_| Error::INVALID)?;
        let encoding = match fields.next() {
            Some("utf8") => Encoding::Utf8,
            Some("args") => Encoding::Arguments,
            _ => return Err(Error::UNSUPPORTED),
        };
        if fields.next().is_some() {
            return Err(Error::INVALID);
        }
        Ok(Self {
            contract: Contract::new(name, version),
            encoding,
            output: output(text)?,
        })
    }

    /// Encodes supplied arguments without guessing a module's request schema.
    /// # Errors
    /// UTF-8 calls accept at most one value; argument lists cannot contain NUL.
    pub fn data(&self, args: &[&str]) -> Result<Box<[u8]>, Error> {
        if args.iter().any(|arg| arg.contains('\0')) {
            return Err(Error::INVALID);
        }
        match (self.encoding, args) {
            (_, []) => Ok(Box::new([])),
            (Encoding::Utf8, [value]) => Ok(value.as_bytes().into()),
            (Encoding::Utf8, _) => Err(Error::INVALID),
            (Encoding::Arguments, values) => {
                Ok(values.join("\0").into_bytes().into())
            }
        }
    }
}

fn output(text: &str) -> Result<Output, Error> {
    let mut fields =
        text.lines().filter_map(|line| line.strip_prefix("output\t"));
    let output = match fields.next() {
        None | Some("message") => Output::Message,
        Some("bytes") => Output::Bytes,
        Some("close") => Output::Close,
        _ => return Err(Error::UNSUPPORTED),
    };
    if fields.next().is_some() { Err(Error::INVALID) } else { Ok(output) }
}

/// Reads `info` without invoking the selected Resource's write operation.
/// # Errors
/// Missing info leaves explicit contract invocation available to the caller.
#[allow(clippy::future_not_send)]
pub async fn discover(
    connection: &Connection,
    subject: ResourceId,
) -> Result<DefaultCall, RequestError> {
    let info = connection.resolve_at(subject, "info").await?;
    let value = connection
        .read(info, &Message::new(Contract::new("resource.bytes", 1), &[]))
        .await?;
    DefaultCall::parse(&value.data).map_err(RequestError::Failed)
}

/// Normalizes a path relative to CWD; absolute paths start at the granted root.
#[must_use]
pub fn path(cwd: &str, value: &str) -> alloc::string::String {
    let base = if value.starts_with('/') { "" } else { cwd };
    let mut parts: alloc::vec::Vec<_> =
        base.split('/').filter(|v| !v.is_empty()).collect();
    for part in value.split('/') {
        match part {
            "" | "." => (),
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

mod request;
pub use request::{arguments, cwd, empty, invoke, optional, subject};
