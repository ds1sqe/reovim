//! Register storage for yank/paste operations.
//!
//! This module provides pure register storage without any clipboard integration.
//! System clipboard access is a driver-level concern.
//!
//! # Vim Register Types
//!
//! - `""` - Unnamed register (default for yank/delete)
//! - `"a` to `"z` - Named registers
//! - `"+` and `"*` - System clipboard (handled by drivers, not here)

use std::collections::HashMap;

/// Type of yank operation.
///
/// This affects how paste operations behave:
/// - **Characterwise**: Paste at cursor position
/// - **Linewise**: Paste below/above current line
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// // yw yanks characterwise
/// let char_yank = YankType::Characterwise;
///
/// // yy yanks linewise
/// let line_yank = YankType::Linewise;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum YankType {
    /// Characterwise yank (e.g., yw, y$, d2w)
    #[default]
    Characterwise,
    /// Linewise yank (e.g., yy, yj, dd)
    Linewise,
}

/// Content stored in a register.
///
/// Combines the yanked text with its yank type for proper paste behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterContent {
    /// The yanked text.
    pub text: String,
    /// How the text was yanked (affects paste behavior).
    pub yank_type: YankType,
}

impl RegisterContent {
    /// Create new register content with specified yank type.
    #[must_use]
    pub fn new(text: impl Into<String>, yank_type: YankType) -> Self {
        Self {
            text: text.into(),
            yank_type,
        }
    }

    /// Create characterwise register content.
    #[must_use]
    pub fn characterwise(text: impl Into<String>) -> Self {
        Self::new(text, YankType::Characterwise)
    }

    /// Create linewise register content.
    #[must_use]
    pub fn linewise(text: impl Into<String>) -> Self {
        Self::new(text, YankType::Linewise)
    }

    /// Check if content is empty.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // String::is_empty is not const
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Check if content is characterwise.
    #[must_use]
    pub const fn is_characterwise(&self) -> bool {
        matches!(self.yank_type, YankType::Characterwise)
    }

    /// Check if content is linewise.
    #[must_use]
    pub const fn is_linewise(&self) -> bool {
        matches!(self.yank_type, YankType::Linewise)
    }
}

impl Default for RegisterContent {
    fn default() -> Self {
        Self {
            text: String::new(),
            yank_type: YankType::Characterwise,
        }
    }
}

/// Register storage for yank/paste operations.
///
/// This is a pure data structure without any system clipboard integration.
/// System clipboard access (`"+` and `"*`) is handled at the driver level.
///
/// # Supported Registers
///
/// - Unnamed register (`""`) - Default for all operations
/// - Named registers (`"a` to `"z`) - User-specified storage
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let mut bank = RegisterBank::new();
///
/// // Set unnamed register (default yank target)
/// bank.set(RegisterContent::characterwise("hello"));
/// assert_eq!(bank.get().text, "hello");
///
/// // Use named register
/// bank.set_named('a', RegisterContent::linewise("line content"));
/// assert_eq!(bank.get_named('a').map(|r| r.text.as_str()), Some("line content"));
/// ```
#[derive(Debug, Clone)]
pub struct RegisterBank {
    /// Unnamed register (default target).
    unnamed: RegisterContent,
    /// Named registers (a-z).
    named: HashMap<char, RegisterContent>,
}

impl RegisterBank {
    /// Create a new empty register bank.
    #[must_use]
    pub fn new() -> Self {
        Self {
            unnamed: RegisterContent::default(),
            named: HashMap::new(),
        }
    }

    /// Get the unnamed register content.
    #[must_use]
    pub const fn get(&self) -> &RegisterContent {
        &self.unnamed
    }

    /// Set the unnamed register content.
    pub fn set(&mut self, content: RegisterContent) {
        self.unnamed = content;
    }

    /// Get a named register content ('a'-'z').
    ///
    /// Returns `None` if the register name is invalid or empty.
    #[must_use]
    pub fn get_named(&self, name: char) -> Option<&RegisterContent> {
        if name.is_ascii_lowercase() {
            self.named.get(&name)
        } else {
            None
        }
    }

    /// Set a named register content ('a'-'z').
    ///
    /// Returns `true` if successful, `false` if the register name is invalid.
    pub fn set_named(&mut self, name: char, content: RegisterContent) -> bool {
        if name.is_ascii_lowercase() {
            self.named.insert(name, content);
            true
        } else {
            false
        }
    }

    /// Append to a named register ('A'-'Z' appends to 'a'-'z').
    ///
    /// Returns `true` if successful, `false` if the register name is invalid.
    pub fn append_named(&mut self, name: char, content: &str) -> bool {
        if name.is_ascii_uppercase() {
            let lower = name.to_ascii_lowercase();
            if let Some(existing) = self.named.get_mut(&lower) {
                existing.text.push_str(content);
            } else {
                self.named
                    .insert(lower, RegisterContent::characterwise(content.to_string()));
            }
            true
        } else {
            false
        }
    }

    /// Clear the unnamed register.
    pub fn clear(&mut self) {
        self.unnamed = RegisterContent::default();
    }

    /// Clear a named register.
    ///
    /// Returns `true` if the register existed and was cleared.
    pub fn clear_named(&mut self, name: char) -> bool {
        if name.is_ascii_lowercase() {
            self.named.remove(&name).is_some()
        } else {
            false
        }
    }

    /// Clear all registers.
    pub fn clear_all(&mut self) {
        self.unnamed = RegisterContent::default();
        self.named.clear();
    }

    /// Get register by name.
    ///
    /// - `None` or `'"'` returns the unnamed register
    /// - `'a'`-`'z'` returns named registers
    #[must_use]
    pub fn get_by_name(&self, name: Option<char>) -> Option<&RegisterContent> {
        match name {
            None | Some('"') => Some(&self.unnamed),
            Some(c) if c.is_ascii_lowercase() => self.named.get(&c),
            _ => None,
        }
    }

    /// Iterate over all non-empty registers.
    ///
    /// Returns an iterator of (name, content) pairs.
    /// The unnamed register uses `'"'` as its name.
    pub fn iter_non_empty(&self) -> impl Iterator<Item = (char, &RegisterContent)> {
        // Start with unnamed register if non-empty
        let unnamed_iter = if self.unnamed.is_empty() {
            None
        } else {
            Some(('"', &self.unnamed))
        };

        // Chain with named registers, filtering empty ones
        unnamed_iter.into_iter().chain(
            self.named
                .iter()
                .filter(|(_, content)| !content.is_empty())
                .map(|(name, content)| (*name, content)),
        )
    }

    /// Set register by name.
    ///
    /// - `None` or `'"'` sets the unnamed register
    /// - `'a'`-`'z'` sets named registers
    /// - `'A'`-`'Z'` appends to named registers
    ///
    /// Returns `true` if successful.
    pub fn set_by_name(&mut self, name: Option<char>, content: RegisterContent) -> bool {
        match name {
            None | Some('"') => {
                self.unnamed = content;
                true
            }
            Some(c) if c.is_ascii_lowercase() => {
                self.named.insert(c, content);
                true
            }
            Some(c) if c.is_ascii_uppercase() => {
                let lower = c.to_ascii_lowercase();
                if let Some(existing) = self.named.get_mut(&lower) {
                    existing.text.push_str(&content.text);
                } else {
                    self.named.insert(lower, content);
                }
                true
            }
            _ => false,
        }
    }
}

impl Default for RegisterBank {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_content_new() {
        let content = RegisterContent::new("hello", YankType::Characterwise);
        assert_eq!(content.text, "hello");
        assert!(content.is_characterwise());
    }

    #[test]
    fn test_register_content_characterwise() {
        let content = RegisterContent::characterwise("test");
        assert!(content.is_characterwise());
        assert!(!content.is_linewise());
    }

    #[test]
    fn test_register_content_linewise() {
        let content = RegisterContent::linewise("test");
        assert!(!content.is_characterwise());
        assert!(content.is_linewise());
    }

    #[test]
    fn test_register_bank_unnamed() {
        let mut bank = RegisterBank::new();
        assert!(bank.get().is_empty());

        bank.set(RegisterContent::characterwise("hello"));
        assert_eq!(bank.get().text, "hello");
    }

    #[test]
    fn test_register_bank_named() {
        let mut bank = RegisterBank::new();

        assert!(bank.set_named('a', RegisterContent::characterwise("alpha")));
        assert_eq!(bank.get_named('a').map(|r| r.text.as_str()), Some("alpha"));

        // Invalid register name
        assert!(!bank.set_named('1', RegisterContent::characterwise("invalid")));
        assert!(bank.get_named('1').is_none());
    }

    #[test]
    fn test_register_bank_append() {
        let mut bank = RegisterBank::new();

        bank.set_named('a', RegisterContent::characterwise("hello"));
        bank.append_named('A', " world");

        assert_eq!(bank.get_named('a').map(|r| r.text.as_str()), Some("hello world"));
    }

    #[test]
    fn test_register_bank_get_by_name() {
        let mut bank = RegisterBank::new();
        bank.set(RegisterContent::characterwise("unnamed"));
        bank.set_named('x', RegisterContent::characterwise("named"));

        assert_eq!(bank.get_by_name(None).map(|r| r.text.as_str()), Some("unnamed"));
        assert_eq!(bank.get_by_name(Some('"')).map(|r| r.text.as_str()), Some("unnamed"));
        assert_eq!(bank.get_by_name(Some('x')).map(|r| r.text.as_str()), Some("named"));
        assert!(bank.get_by_name(Some('+')).is_none()); // System clipboard not handled here
    }

    #[test]
    fn test_register_bank_set_by_name() {
        let mut bank = RegisterBank::new();

        assert!(bank.set_by_name(None, RegisterContent::characterwise("unnamed")));
        assert!(bank.set_by_name(Some('a'), RegisterContent::characterwise("alpha")));
        assert!(bank.set_by_name(Some('A'), RegisterContent::characterwise(" appended")));

        assert_eq!(bank.get().text, "unnamed");
        assert_eq!(bank.get_named('a').map(|r| r.text.as_str()), Some("alpha appended"));
    }

    #[test]
    fn test_register_bank_clear() {
        let mut bank = RegisterBank::new();
        bank.set(RegisterContent::characterwise("hello"));
        bank.set_named('a', RegisterContent::characterwise("alpha"));

        bank.clear();
        assert!(bank.get().is_empty());
        assert!(bank.get_named('a').is_some()); // Named not affected

        bank.clear_all();
        assert!(bank.get_named('a').is_none());
    }

    // === RegisterContent::default() ===

    #[test]
    fn test_register_content_default() {
        let content = RegisterContent::default();
        assert!(content.is_empty());
        assert!(content.is_characterwise());
        assert!(!content.is_linewise());
    }

    // === clear_named ===

    #[test]
    fn test_clear_named_existing() {
        let mut bank = RegisterBank::new();
        bank.set_named('a', RegisterContent::characterwise("hello"));
        assert!(bank.clear_named('a'));
        assert!(bank.get_named('a').is_none());
    }

    #[test]
    fn test_clear_named_nonexisting() {
        let mut bank = RegisterBank::new();
        assert!(!bank.clear_named('a'));
    }

    #[test]
    fn test_clear_named_invalid() {
        let mut bank = RegisterBank::new();
        assert!(!bank.clear_named('1'));
        assert!(!bank.clear_named('A'));
    }

    // === iter_non_empty ===

    #[test]
    fn test_iter_non_empty_with_unnamed() {
        let mut bank = RegisterBank::new();
        bank.set(RegisterContent::characterwise("hello"));
        bank.set_named('a', RegisterContent::characterwise("alpha"));

        let items: Vec<_> = bank.iter_non_empty().collect();
        assert!(items.len() >= 2);
        assert!(items.iter().any(|(name, _)| *name == '"'));
        assert!(items.iter().any(|(name, _)| *name == 'a'));
    }

    #[test]
    fn test_iter_non_empty_skips_empty() {
        let bank = RegisterBank::new();
        // Unnamed is empty by default
        assert_eq!(bank.iter_non_empty().count(), 0);
    }

    // === set_by_name uppercase to non-existing ===

    #[test]
    fn test_set_by_name_uppercase_creates_new() {
        let mut bank = RegisterBank::new();
        // Append to non-existing register creates it
        assert!(bank.set_by_name(Some('A'), RegisterContent::characterwise("hello")));
        assert_eq!(bank.get_named('a').map(|r| r.text.as_str()), Some("hello"));
    }

    // === set_by_name invalid char ===

    #[test]
    fn test_set_by_name_invalid() {
        let mut bank = RegisterBank::new();
        assert!(!bank.set_by_name(Some('+'), RegisterContent::characterwise("test")));
        assert!(!bank.set_by_name(Some('1'), RegisterContent::characterwise("test")));
    }

    // === append_named to non-existing ===

    #[test]
    fn test_append_named_creates_new() {
        let mut bank = RegisterBank::new();
        assert!(bank.append_named('A', "world"));
        assert_eq!(bank.get_named('a').map(|r| r.text.as_str()), Some("world"));
    }

    #[test]
    fn test_append_named_invalid() {
        let mut bank = RegisterBank::new();
        assert!(!bank.append_named('a', "test")); // lowercase is invalid for append
        assert!(!bank.append_named('1', "test"));
    }

    // === get_by_name with quote ===

    #[test]
    fn test_get_by_name_quote() {
        let mut bank = RegisterBank::new();
        bank.set(RegisterContent::characterwise("unnamed"));
        assert_eq!(bank.get_by_name(Some('"')).map(|r| r.text.as_str()), Some("unnamed"));
    }

    // === RegisterBank Default ===

    #[test]
    fn test_register_bank_default() {
        let bank = RegisterBank::default();
        assert!(bank.get().is_empty());
    }
}
