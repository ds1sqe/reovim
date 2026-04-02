//! Domain trait for content-type abstraction.
//!
//! Each content domain (text, audio, video, etc.) implements [`Domain`]
//! to define its position, edit, and content types. This enables generic
//! codec and provider traits (`Decode<D>`, `Encode<D>`, `Index<D>`) that
//! work across any domain without knowing the concrete types.
//!
//! # Zero Dependencies
//!
//! This crate has no dependencies. It defines only the trait contract.
//! Concrete domain implementations live in their own crates
//! (e.g., `reovim-types-text` for the text domain).
//!
//! # Example
//!
//! ```
//! use reovim_domain::Domain;
//!
//! struct Text;
//!
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! struct TextPosition { line: usize, col: usize }
//!
//! #[derive(Debug, Clone)]
//! struct TextEdit { start: TextPosition, end: TextPosition, text: String }
//!
//! impl Domain for Text {
//!     type Position = TextPosition;
//!     type Edit = TextEdit;
//!     type Content = String;
//! }
//! ```

use core::fmt::Debug;

/// A content domain marker.
///
/// Each content type (text, audio, video, binary) implements this trait
/// to declare its fundamental types:
///
/// - [`Position`](Self::Position) — a location within the domain (line:col for text, sample offset for audio)
/// - [`Edit`](Self::Edit) — a domain-level modification (text insertion, audio splice)
/// - [`Content`](Self::Content) — decoded domain content (String for text, sample buffer for audio)
///
/// These associated types flow through the generic codec and provider traits,
/// enabling domain-agnostic infrastructure without any concrete type knowledge.
pub trait Domain: Send + Sync + 'static {
    /// A location within the content domain.
    ///
    /// For text: line and column. For audio: sample offset and channel.
    type Position: Send + Sync + Clone + Debug + PartialEq + 'static;

    /// A domain-level edit operation.
    ///
    /// For text: replace range with string. For audio: splice samples.
    type Edit: Send + Sync + Clone + Debug + 'static;

    /// Decoded content in this domain.
    ///
    /// For text: `String`. For audio: sample buffer.
    type Content: Send + Sync + 'static;
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
