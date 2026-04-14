use crate::codec::{CursorCodec, PositionCodec};

/// Error type for domain enlistment and codec registration.
#[derive(Debug)]
pub enum EnlistError {
    /// A domain with this name is already registered.
    DomainNameTaken(String),
    /// The specified domain ID does not exist (not yet enlisted).
    DomainNotFound(u32),
    /// A codec is already registered for this `(domain_id, inner_id)` pair.
    CodecAlreadyRegistered {
        /// The domain ID.
        domain_id: u32,
        /// The inner type ID.
        inner_id: u16,
    },
}

impl core::fmt::Display for EnlistError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DomainNameTaken(name) => {
                write!(f, "domain name already taken: {name:?}")
            }
            Self::DomainNotFound(id) => {
                write!(f, "domain not found: domain_id={id}")
            }
            Self::CodecAlreadyRegistered {
                domain_id,
                inner_id,
            } => {
                write!(f, "codec already registered: domain_id={domain_id}, inner_id={inner_id}")
            }
        }
    }
}

impl std::error::Error for EnlistError {}

/// Registry for domain coordination — driver enlistment and codec lookup.
///
/// The server creates a single `CoordinationRegistry` at startup. Drivers
/// enlist their domains and register codecs during initialization. After
/// all drivers have enlisted, the registry is effectively sealed (no new
/// registrations should occur during normal operation).
///
/// This is a trait (not a concrete type) to allow mock injection in tests.
///
/// # Interior mutability
///
/// Implementations use interior mutability (`RwLock` or similar) because
/// `enlist_domain` and `register_*_codec` take `&self`. The registry is
/// shared via `Arc<dyn CoordinationRegistry>`.
///
/// # Lookup lifetime
///
/// The `position_codec` and `cursor_codec` methods return `Option<&dyn ...>`
/// tied to `&self`. Implementations must store codecs in a structure that
/// allows returning references without holding a lock — typically by using
/// a two-phase pattern: mutable registration during startup, then frozen
/// immutable storage for lookups.
///
/// # Name invariants
///
/// All `name` and `description` parameters are `&'static str` — they must
/// live for the entire program lifetime. Names are for display/debugging
/// only (logging, enlistment log), not for runtime lookup.
pub trait CoordinationRegistry: Send + Sync {
    /// Register a new domain. The server assigns a unique `domain_id`.
    ///
    /// `domain_id=0` is reserved as sentinel and will never be assigned.
    ///
    /// # Errors
    ///
    /// Returns [`EnlistError::DomainNameTaken`] if `name` is already registered.
    fn enlist_domain(
        &self,
        name: &'static str,
        description: &'static str,
    ) -> Result<u32, EnlistError>;

    /// Register a position codec for a `(domain_id, inner_id)` pair.
    ///
    /// # Errors
    ///
    /// Returns [`EnlistError::DomainNotFound`] if `domain_id` has not been
    /// enlisted, or [`EnlistError::CodecAlreadyRegistered`] if a codec is
    /// already registered for this pair.
    fn register_position_codec(
        &self,
        domain_id: u32,
        inner_id: u16,
        name: &'static str,
        codec: Box<dyn PositionCodec>,
    ) -> Result<(), EnlistError>;

    /// Register a cursor codec for a `(domain_id, inner_id)` pair.
    ///
    /// # Errors
    ///
    /// Returns [`EnlistError::DomainNotFound`] if `domain_id` has not been
    /// enlisted, or [`EnlistError::CodecAlreadyRegistered`] if a codec is
    /// already registered for this pair.
    fn register_cursor_codec(
        &self,
        domain_id: u32,
        inner_id: u16,
        name: &'static str,
        codec: Box<dyn CursorCodec>,
    ) -> Result<(), EnlistError>;

    /// Look up a domain's name by its ID (for logging/debugging).
    ///
    /// Returns `None` if the domain ID is not registered.
    /// The returned `&str` has `'static` lifetime (provided at enlistment).
    fn domain_name(&self, domain_id: u32) -> Option<&str>;

    /// Look up a position codec by `(domain_id, inner_id)`.
    fn position_codec(&self, domain_id: u32, inner_id: u16) -> Option<&dyn PositionCodec>;

    /// Look up a cursor codec by `(domain_id, inner_id)`.
    fn cursor_codec(&self, domain_id: u32, inner_id: u16) -> Option<&dyn CursorCodec>;
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
