use core::fmt;

use crate::ResourceId;

/// Extensible error code; components own codes under their contracts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ErrorCode(pub u64);

/// Failure with optional diagnostic context, never a recovery obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub id: Option<ResourceId>,
}

impl Error {
    pub const DENIED: Self = Self::new(1);
    pub const UNAVAILABLE: Self = Self::new(2);
    pub const INVALID: Self = Self::new(3);
    pub const UNSUPPORTED: Self = Self::new(4);
    pub const PRESSURE: Self = Self::new(5);
    pub const CLOSED: Self = Self::new(6);
    pub const STALE: Self = Self::new(7);
    pub const BUSY: Self = Self::new(8);
    pub const CANCELLED: Self = Self::new(9);
    pub const NOT_READY: Self = Self::new(10);
    pub const PROVIDER: Self = Self::new(11);
    pub const GAP: Self = Self::new(12);

    #[must_use]
    pub const fn new(code: u64) -> Self {
        Self { code: ErrorCode(code), id: None }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "Resource error {}", self.code.0)
    }
}

impl core::error::Error for Error {}
