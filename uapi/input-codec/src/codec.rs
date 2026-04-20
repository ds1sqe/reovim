//! Codec trait — the plug-in contract for encoding/decoding input event bodies.
//!
//! Each platform codec implements this trait to register with the
//! `InputCodecRegistry` (defined in `reovim-subsys-input`).
//!
//! # Kind numbering convention
//!
//! | Range | Owner |
//! |-------|-------|
//! | `0x0001-0x00FF` | In-repo well-known codecs |
//! | `0x0100-0xFFFF` | External codec crates |

use crate::InputPayloadError;

/// Plug-in contract for encoding and decoding a single input kind.
///
/// Implementors are registered with the `InputCodecRegistry` at module load
/// time and unregistered at module unload time.  The registry holds
/// `Arc<dyn Codec>` for the module's loaded span.
///
/// # Object-safety
///
/// This trait is intentionally object-safe so the registry can store
/// `Arc<dyn Codec>` without knowing the concrete type at compile time.
pub trait Codec: Send + Sync + 'static {
    /// The kind value this codec handles (bytes 0-1 of every payload).
    fn kind(&self) -> u16;

    /// Encode an opaque value into a payload body (bytes 8+ of an
    /// `InputEvent` payload).  The 8-byte header is the caller's
    /// responsibility.
    ///
    /// # Errors
    ///
    /// Returns [`InputPayloadError`] if the value cannot be encoded.
    fn encode(&self, value: &dyn std::any::Any) -> Result<Vec<u8>, InputPayloadError>;

    /// Decode a payload body (bytes 8+ of an `InputEvent` payload) into
    /// an opaque box.
    ///
    /// # Errors
    ///
    /// Returns [`InputPayloadError::TooShort`] if the payload is shorter
    /// than the minimum body length this codec requires.
    fn decode(&self, body: &[u8]) -> Result<Box<dyn std::any::Any + Send>, InputPayloadError>;
}
