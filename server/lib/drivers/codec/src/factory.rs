//! Content codec factory trait.
//!
//! Factories create codec instances for specific content types.
//! They are registered by modules during `init()` and stored in
//! [`ContentCodecFactoryStore`](crate::ContentCodecFactoryStore).

use crate::{ContentType, codec::ContentCodec};

/// Factory for creating content codecs.
///
/// Each module that provides codec support implements this trait to
/// create codec instances for its supported content types.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` for use across async tasks.
///
/// # Examples
///
/// ```ignore
/// struct Utf8CodecFactory;
///
/// impl ContentCodecFactory for Utf8CodecFactory {
///     fn create(&self, content_type: &ContentType) -> Option<Box<dyn ContentCodec>> {
///         if content_type.as_str() == "text/utf-8" {
///             Some(Box::new(Utf8Codec))
///         } else {
///             None
///         }
///     }
///
///     fn supported_content_types(&self) -> Vec<&str> {
///         vec!["text/utf-8"]
///     }
///
///     fn name(&self) -> &str { "utf-8" }
/// }
/// ```
pub trait ContentCodecFactory: Send + Sync {
    /// Create a codec for the given content type.
    ///
    /// Returns `Some(codec)` if this factory supports the content type,
    /// `None` otherwise.
    fn create(&self, content_type: &ContentType) -> Option<Box<dyn ContentCodec>>;

    /// List all content types this factory supports.
    fn supported_content_types(&self) -> Vec<&str>;

    /// Human-readable name for this factory.
    fn name(&self) -> &'static str;
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
