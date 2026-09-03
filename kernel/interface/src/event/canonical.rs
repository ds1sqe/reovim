use reovim_foundation::bound::{BoundValue, CheckedByteCount, ZeroPolicy};

use super::{CanonicalEventBytes, EventError, StartupEventDescriptor};

mod grammar;

pub(super) use grammar::{
    StartupHealthCode, StartupLine, StartupOutcomeCode, StartupRollbackCode,
};

impl<'a> crate::event::CanonicalEventBytes<'a> {
    /// Validates one borrowed canonical line without rendering or allocating.
    ///
    /// This checks the byte bound before traversal, then validates the exact
    /// startup-v1 grammar and descriptor-bound fields. The separately admitted
    /// renderer remains responsible for producing those bytes.
    ///
    /// # Errors
    ///
    /// Returns a typed empty, byte-bound, ASCII, or framing failure.
    pub fn new(
        descriptor: &StartupEventDescriptor,
        bytes: &'a [u8],
    ) -> Result<Self, EventError> {
        if bytes.is_empty() {
            return Err(EventError::EmptyCanonicalBytes);
        }
        let Ok(size) = u64::try_from(bytes.len()) else {
            return Err(EventError::CanonicalLengthOverflow);
        };
        let size = CheckedByteCount::new(size, ZeroPolicy::Reject)
            .map_err(EventError::Bound)?;
        descriptor
            .limits()
            .canonical_bytes()
            .admit_per_object(BoundValue::Bytes(size))
            .map_err(EventError::Bound)?;
        if !bytes.is_ascii() {
            return Err(EventError::NonAsciiCanonicalBytes);
        }
        if !has_exact_framing(bytes) {
            return Err(EventError::InvalidCanonicalFraming);
        }
        grammar::parse_startup_v1(descriptor, &bytes[..bytes.len() - 1])?;
        Ok(Self { bytes, size })
    }

    /// Returns the validated borrowed canonical bytes.
    #[must_use]
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Returns the checked canonical byte count.
    #[must_use]
    pub const fn size(self) -> CheckedByteCount {
        self.size
    }
}

pub(super) fn parse_startup_line(
    descriptor: &StartupEventDescriptor,
    canonical: CanonicalEventBytes<'_>,
) -> Result<StartupLine, EventError> {
    let bytes = canonical.bytes();
    grammar::parse_startup_v1(descriptor, &bytes[..bytes.len() - 1])
}

fn has_exact_framing(bytes: &[u8]) -> bool {
    if bytes.last() != Some(&b'\n') {
        return false;
    }
    !bytes[..bytes.len() - 1].contains(&b'\n') && !bytes.contains(&b'\r')
}
