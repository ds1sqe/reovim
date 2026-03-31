//! Leader key provider for personality-aware binding expansion (#700).
//!
//! Personality modules (vim, emacs) register their leader key notation
//! during `init()`. Bootstrap reads it after all modules have initialized
//! and expands `<leader>` tokens in keybinding key strings before parsing.
//!
//! This follows the established `InitialModeProvider` pattern.

use reovim_kernel::api::v1::Service;

/// Provider for the leader key notation.
///
/// Personality modules call [`set`](Self::set) during `init()` to declare
/// their leader key (e.g., `"<Space>"` for vim). Bootstrap calls [`get`](Self::get)
/// after all modules have loaded and expands `<leader>` in keybinding strings.
///
/// The stored value is `&'static str` because all call sites are personality
/// module `init()` methods passing compile-time string literals. This matches
/// the `KeybindingRegistration.keys` lifetime requirement.
///
/// If multiple personality modules are loaded, the last writer wins and a
/// warning should be logged by the caller.
pub struct LeaderKeyProvider {
    key: parking_lot::RwLock<Option<&'static str>>,
}

impl LeaderKeyProvider {
    /// Create a new empty provider.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            key: parking_lot::RwLock::new(None),
        }
    }

    /// Set the leader key notation. Called by personality modules during `init()`.
    ///
    /// The `key` parameter is a key notation string (e.g., `"<Space>"`) that
    /// will replace `<leader>` tokens in keybinding key strings.
    ///
    /// Returns the previous notation if one was already set.
    pub fn set(&self, key: &'static str) -> Option<&'static str> {
        self.key.write().replace(key)
    }

    /// Get the registered leader key notation, if any.
    #[must_use]
    pub fn get(&self) -> Option<&'static str> {
        *self.key.read()
    }

    /// Expand `<leader>` tokens in a key string using the stored notation.
    ///
    /// Returns the original string (owned) if no leader key is registered
    /// or if the input contains no `<leader>` token.
    #[must_use]
    pub fn expand(&self, keys: &str) -> String {
        self.get()
            .map_or_else(|| keys.to_owned(), |notation| expand_leader(keys, notation))
    }
}

impl Default for LeaderKeyProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for LeaderKeyProvider {}

/// Expand all `<leader>` tokens in a key string with the given notation.
///
/// Case-insensitive matching: both `<leader>` and `<Leader>` are replaced.
/// If `notation` is empty, `<leader>` tokens are removed (effectively a
/// no-op binding prefix).
///
/// This is a pure function for testability.
#[must_use]
pub fn expand_leader(keys: &str, notation: &str) -> String {
    // Fast path: no `<` means no special tokens at all.
    if !keys.contains('<') {
        return keys.to_owned();
    }

    let needle = "<leader>";
    let mut result = String::with_capacity(keys.len());
    let mut remaining = keys;

    while let Some(pos) = remaining
        .as_bytes()
        .windows(needle.len())
        .position(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
    {
        result.push_str(&remaining[..pos]);
        result.push_str(notation);
        remaining = &remaining[pos + needle.len()..];
    }
    result.push_str(remaining);
    result
}

#[cfg(test)]
#[path = "leader_key_tests.rs"]
mod tests;
