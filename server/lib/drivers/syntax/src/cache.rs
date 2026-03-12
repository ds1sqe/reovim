//! Syntax cache trait.
//!
//! This module defines the [`SyntaxCache`] trait for caching highlight results.

use std::ops::Range;

use crate::highlight::Annotation;

/// Cache for annotation results.
///
/// Implementations can provide various caching strategies for
/// annotation data to improve performance.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
///
/// # Example
///
/// ```ignore
/// // Insert annotations for a range
/// cache.insert(0..100, annotations);
///
/// // Later, retrieve cached annotations
/// if let Some(cached) = cache.get(0..100) {
///     // Use cached annotations
/// }
///
/// // After an edit, invalidate affected ranges
/// cache.invalidate_range(50..75);
/// ```
pub trait SyntaxCache: Send + Sync {
    /// Get cached annotations for a byte range.
    ///
    /// Returns `None` if the range is not cached or cache is stale.
    ///
    /// # Arguments
    ///
    /// * `byte_range` - The byte range to look up
    fn get(&self, byte_range: Range<usize>) -> Option<Vec<Annotation>>;

    /// Insert annotations into the cache.
    ///
    /// # Arguments
    ///
    /// * `byte_range` - The byte range these annotations cover
    /// * `highlights` - The annotations to cache
    fn insert(&mut self, byte_range: Range<usize>, highlights: Vec<Annotation>);

    /// Invalidate cache entries that overlap with a range.
    ///
    /// Called after edits to mark affected regions as stale.
    ///
    /// # Arguments
    ///
    /// * `byte_range` - The byte range that was modified
    fn invalidate_range(&mut self, byte_range: Range<usize>);

    /// Clear all cached data.
    fn clear(&mut self);

    /// Check if the cache is empty.
    fn is_empty(&self) -> bool;
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
