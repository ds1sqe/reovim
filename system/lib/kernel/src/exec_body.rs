//! Checked Reovim executable body wrapper.
//!
//! This is the first executable body format above provider envelopes. It keeps
//! provider/catalog checks separate from the body contract while current bodies
//! still delegate to the transitional source-image interpreters.

use crate::source_store;

const EXEC_BODY_MAGIC: &[u8] = b"reovim-exec-body-v1";
const EXEC_BODY_INNER_PREFIX: &[u8] = b"inner=";
const EXEC_BODY_BYTES_PREFIX: &[u8] = b"bytes=";
const EXEC_BODY_CHECKSUM_PREFIX: &[u8] = b"checksum=";

/// Inner executable format carried by a checked Reovim executable body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecBodyInnerFormat {
    /// `/bin` source-image compatibility body.
    BinSourceImage,
    /// `/bin` body that runs through Reovim domain uapi over raw syscall.
    BinUapiV1,
    /// `/payload` source-image compatibility body.
    PayloadSourceImage,
}

impl ExecBodyInnerFormat {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BinSourceImage => "bin-source-image",
            Self::BinUapiV1 => "bin-uapi-v1",
            Self::PayloadSourceImage => "payload-source-image",
        }
    }
}

/// Error while parsing a checked executable body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecBodyError {
    /// The body did not start with the executable-body magic header.
    MissingMagic,
    /// The inner-format row was missing or malformed.
    MissingInner,
    /// The inner-format value is unsupported.
    InvalidInner,
    /// The body byte-count row was missing or malformed.
    MissingBytes,
    /// The checksum row was missing or malformed.
    MissingChecksum,
    /// The declared body length does not match the bytes that follow.
    BodyLengthMismatch,
    /// The declared checksum does not match the inner body.
    ChecksumMismatch,
}

impl ExecBodyError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingMagic => "missing-magic",
            Self::MissingInner => "missing-inner",
            Self::InvalidInner => "invalid-inner",
            Self::MissingBytes => "missing-bytes",
            Self::MissingChecksum => "missing-checksum",
            Self::BodyLengthMismatch => "body-length-mismatch",
            Self::ChecksumMismatch => "checksum-mismatch",
        }
    }
}

/// Parsed executable body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecBody<'a> {
    /// Inner executable format selected by the wrapper.
    pub inner: ExecBodyInnerFormat,
    /// Checked inner body bytes.
    pub bytes: &'a [u8],
    /// Verified checksum over [`Self::bytes`].
    pub checksum: u32,
}

/// Returns whether `bytes` begins with the executable-body magic header.
#[must_use]
pub fn looks_like_exec_body(bytes: &[u8]) -> bool {
    matches!(next_line(bytes, 0), Some((line, _)) if line == EXEC_BODY_MAGIC)
}

/// Parses a checked executable body.
pub fn parse_exec_body(bytes: &[u8]) -> Result<ExecBody<'_>, ExecBodyError> {
    let (magic, offset) = next_line(bytes, 0).ok_or(ExecBodyError::MissingMagic)?;
    if magic != EXEC_BODY_MAGIC {
        return Err(ExecBodyError::MissingMagic);
    }

    let (inner_bytes, offset) =
        parse_row(bytes, offset, EXEC_BODY_INNER_PREFIX, ExecBodyError::MissingInner)?;
    let inner = match inner_bytes {
        b"bin-source-image" => ExecBodyInnerFormat::BinSourceImage,
        b"bin-uapi-v1" => ExecBodyInnerFormat::BinUapiV1,
        b"payload-source-image" => ExecBodyInnerFormat::PayloadSourceImage,
        _ => return Err(ExecBodyError::InvalidInner),
    };

    let (declared_len_bytes, offset) =
        parse_row(bytes, offset, EXEC_BODY_BYTES_PREFIX, ExecBodyError::MissingBytes)?;
    let declared_len = parse_usize(declared_len_bytes).ok_or(ExecBodyError::MissingBytes)?;

    let (checksum_bytes, offset) =
        parse_row(bytes, offset, EXEC_BODY_CHECKSUM_PREFIX, ExecBodyError::MissingChecksum)?;
    let checksum = parse_u32(checksum_bytes).ok_or(ExecBodyError::MissingChecksum)?;

    let body = &bytes[offset..];
    if body.len() != declared_len {
        return Err(ExecBodyError::BodyLengthMismatch);
    }
    if source_store::source_media_checksum32(body) != checksum {
        return Err(ExecBodyError::ChecksumMismatch);
    }

    Ok(ExecBody {
        inner,
        bytes: body,
        checksum,
    })
}

fn parse_row<'a>(
    bytes: &'a [u8],
    offset: usize,
    prefix: &[u8],
    error: ExecBodyError,
) -> Result<(&'a [u8], usize), ExecBodyError> {
    let (line, next) = next_line(bytes, offset).ok_or(error)?;
    if !line.starts_with(prefix) {
        return Err(error);
    }
    Ok((&line[prefix.len()..], next))
}

fn next_line(bytes: &[u8], offset: usize) -> Option<(&[u8], usize)> {
    if offset >= bytes.len() {
        return None;
    }

    let mut end = offset;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }

    let mut next = end;
    if next < bytes.len() && bytes[next] == b'\n' {
        next += 1;
    }

    let mut line = &bytes[offset..end];
    if line.last() == Some(&b'\r') {
        line = &line[..line.len() - 1];
    }

    Some((line, next))
}

fn parse_usize(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty() {
        return None;
    }
    let mut value = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?;
        value = value.checked_add((byte - b'0') as usize)?;
        index += 1;
    }
    Some(value)
}

fn parse_u32(bytes: &[u8]) -> Option<u32> {
    let value = parse_usize(bytes)?;
    if value > u32::MAX as usize {
        return None;
    }
    Some(value as u32)
}

#[cfg(feature = "selftest")]
#[path = "exec_body_tests.rs"]
mod tests;
