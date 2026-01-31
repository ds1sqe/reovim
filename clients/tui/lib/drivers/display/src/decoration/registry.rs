//! Decoration provider registry.
//!
//! Provides type-safe lookup for decoration providers registered by modules.

use {
    super::{
        key::{DecorationProviderKey, DecorationSourceKey},
        provider::{BufferDecorationSource, DecorationProvider},
    },
    reovim_kernel::api::v1::MultiServiceRegistry,
};

/// Registry for per-buffer decoration providers, keyed by type.
///
/// Used with `DecorationProviderFactory` pattern for per-buffer providers.
/// For global providers with `buffer_id` awareness, use `BufferDecorationSourceRegistry`.
///
/// # Architecture
///
/// This follows the mechanism/policy separation:
/// - **Mechanism** (this driver): Registry and key types
/// - **Policy** (modules): Actual provider implementations
pub type DecorationProviderRegistry =
    MultiServiceRegistry<DecorationProviderKey, dyn DecorationProvider>;

/// Registry for global buffer decoration sources, keyed by string-based keys.
///
/// Modules register their decoration sources during init using string keys.
/// The runner queries providers generically without knowing about specific modules.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_display::decoration::{
///     DecorationSourceKey, BufferDecorationSourceRegistry,
/// };
///
/// // Module registers source during init
/// let registry = BufferDecorationSourceRegistry::new();
/// let key = DecorationSourceKey::new("pair.rainbow");
/// registry.register(key, Arc::new(pair_state));
///
/// // Runner iterates all sources generically
/// for key in registry.keys() {
///     if let Some(source) = registry.get(&key) {
///         let decorations = source.decorations_for_buffer(buffer_id, content, cursor);
///     }
/// }
/// ```
///
/// # Architecture
///
/// This follows the mechanism/policy separation:
/// - **Mechanism** (this driver): Registry and key types
/// - **Policy** (modules): Key definitions and source implementations
pub type BufferDecorationSourceRegistry =
    MultiServiceRegistry<DecorationSourceKey, dyn BufferDecorationSource>;
