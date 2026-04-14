use crate::{Cursor, CursorHeader, Position, PositionHeader};

/// Codec for decoding wire bytes into a boxed [`Position`] trait object.
///
/// Each domain registers one `PositionCodec` per `(domain_id, inner_id)` pair
/// at startup via [`CoordinationRegistry::register_position_codec`](super::CoordinationRegistry::register_position_codec).
/// The server uses the codec to reconstruct positions from wire bytes.
pub trait PositionCodec: Send + Sync {
    /// The domain ID this codec handles.
    fn domain_id(&self) -> u32;

    /// The inner type ID this codec handles.
    fn inner_id(&self) -> u16;

    /// Decode content bytes (without the header) into a position.
    ///
    /// Returns `None` if the content bytes are invalid or malformed.
    fn decode(&self, header: &PositionHeader, content: &[u8]) -> Option<Box<dyn Position>>;
}

/// Codec for decoding wire bytes into a boxed [`Cursor`] trait object.
///
/// Each domain registers one `CursorCodec` per `(domain_id, inner_id)` pair
/// at startup via [`CoordinationRegistry::register_cursor_codec`](super::CoordinationRegistry::register_cursor_codec).
pub trait CursorCodec: Send + Sync {
    /// The domain ID this codec handles.
    fn domain_id(&self) -> u32;

    /// The inner type ID this codec handles.
    fn inner_id(&self) -> u16;

    /// Decode content bytes (without the header) into a cursor.
    ///
    /// Returns `None` if the content bytes are invalid or malformed.
    fn decode(&self, header: &CursorHeader, content: &[u8]) -> Option<Box<dyn Cursor>>;
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
