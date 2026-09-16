use core::fmt;

use crate::ResourceId;

/// Extensible error code; components own codes under their contracts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ErrorCode(pub u32);

/// Failure with optional diagnostic context, never a recovery obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub id: Option<ResourceId>,
}

impl Error {
    pub const NOT_READY: Self = Self::new(1);
    pub const CLOSED: Self = Self::new(2);
    pub const BUSY: Self = Self::new(3);
    pub const INVALID: Self = Self::new(4);
    pub const DENIED: Self = Self::new(5);
    pub const UNSUPPORTED: Self = Self::new(6);
    pub const PRESSURE: Self = Self::new(7);
    pub const CONFLICT: Self = Self::new(8);
    pub const UNAVAILABLE: Self = Self::new(9);
    pub const CANCELLED: Self = Self::new(10);
    pub const PROVIDER: Self = Self::new(11);
    pub const GAP: Self = Self::new(12);
    pub const INCOMPLETE: Self = Self::new(13);

    #[must_use]
    pub const fn new(code: u32) -> Self {
        Self { code: ErrorCode(code), id: None }
    }

    #[must_use]
    pub const fn with_id(code: u32, id: ResourceId) -> Self {
        Self { code: ErrorCode(code), id: Some(id) }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "Resource error {}", self.code.0)
    }
}

impl core::error::Error for Error {}
