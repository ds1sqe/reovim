//! Register storage and addressing for yank/paste operations.
//!
//! This module provides:
//! - **`Register`** - Type-safe register addressing (mechanism names, not vim terms)
//! - **`RegisterBank`** - Pure register storage without clipboard integration
//! - **`RegisterContent`** - Content stored in a register (text + yank type)
//!
//! System clipboard access is a driver-level concern.
//! Session-scoped registers are stored at the session level, not here.

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

// ============================================================================
// Register Addressing
// ============================================================================

/// Register addressing for the kernel register subsystem.
///
/// Represents storage locations using mechanism names (not editor-specific
/// terminology). The kernel provides WHAT registers exist; modules decide
/// HOW they map to user-facing keys.
///
/// # Storage Routing
///
/// Different variants are stored in different subsystems:
///
/// | Variant | Storage | Mutability |
/// |---------|---------|------------|
/// | `Default` | Per-client `RegisterBank` | Read/Write |
/// | `Slot(char)` | Per-client `RegisterBank` | Read/Write |
/// | `History(u8)` | Per-client `HistoryRing` | Read-only |
/// | `System` | OS clipboard (driver) | Read/Write |
/// | `Session(char)` | Session-level shared storage | Read/Write |
/// | `PeerHistory` | Another client's `HistoryRing` | Read-only |
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::Register;
///
/// let default = Register::Default;
/// assert!(default.is_bank_register());
/// assert!(!default.is_read_only());
///
/// let slot = Register::Slot('a');
/// assert!(slot.is_bank_register());
///
/// let history = Register::History(0);
/// assert!(history.is_read_only());
///
/// let session = Register::Session('A');
/// assert!(session.is_session_scoped());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Register {
    /// The default/fallback register.
    Default,
    /// A keyed storage slot (a-z, 26 per-client slots).
    Slot(char),
    /// Index into the per-client history ring (0-255).
    History(u8),
    /// System clipboard (OS-level).
    System,
    /// Session-scoped shared register (A-Z, shared across all clients).
    Session(char),
    /// Read another client's history ring entry.
    PeerHistory {
        /// Target client identifier.
        client: usize,
        /// History ring index (0-255).
        index: u8,
    },
}

impl Register {
    /// Whether this register is stored in per-client `RegisterBank`.
    #[must_use]
    pub const fn is_bank_register(&self) -> bool {
        matches!(self, Self::Default | Self::Slot(_))
    }

    /// Whether this register is read-only.
    ///
    /// History and peer history registers cannot be written to directly.
    /// They are populated as side effects of other operations.
    #[must_use]
    pub const fn is_read_only(&self) -> bool {
        matches!(self, Self::History(_) | Self::PeerHistory { .. })
    }

    /// Whether this register requires session-level access.
    ///
    /// Session registers and peer history both need access to shared
    /// session state rather than per-client state.
    #[must_use]
    pub const fn is_session_scoped(&self) -> bool {
        matches!(self, Self::Session(_) | Self::PeerHistory { .. })
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

    // ========================================================================
    // Register-typed accessors (#515 Phase 5)
    // ========================================================================

    /// Get register content by typed `Register`.
    ///
    /// Returns `None` for `History`, `System`, `Session`, and `PeerHistory`
    /// variants (not stored in the per-client bank).
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::*;
    ///
    /// let mut bank = RegisterBank::new();
    /// bank.set(RegisterContent::characterwise("hello"));
    ///
    /// assert_eq!(bank.get_register(&Register::Default).map(|r| r.text.as_str()), Some("hello"));
    /// assert!(bank.get_register(&Register::System).is_none());
    /// ```
    #[must_use]
    pub fn get_register(&self, reg: &Register) -> Option<&RegisterContent> {
        match reg {
            Register::Default => Some(&self.unnamed),
            Register::Slot(c) if c.is_ascii_lowercase() => self.named.get(c),
            _ => None,
        }
    }

    /// Set register content by typed `Register`.
    ///
    /// Returns `false` for read-only or non-bank registers.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::*;
    ///
    /// let mut bank = RegisterBank::new();
    /// assert!(bank.set_register(&Register::Slot('a'), RegisterContent::characterwise("alpha")));
    /// assert!(!bank.set_register(&Register::System, RegisterContent::characterwise("nope")));
    /// ```
    pub fn set_register(&mut self, reg: &Register, content: RegisterContent) -> bool {
        match reg {
            Register::Default => {
                self.unnamed = content;
                true
            }
            Register::Slot(c) if c.is_ascii_lowercase() => {
                self.named.insert(*c, content);
                true
            }
            _ => false,
        }
    }

    /// Append content to a slot register (lowercase a-z).
    ///
    /// If the slot doesn't exist, it is created with the given content.
    /// Returns `false` if the slot character is not lowercase a-z.
    pub fn append_slot(&mut self, slot: char, content: &str) -> bool {
        if !slot.is_ascii_lowercase() {
            return false;
        }
        if let Some(existing) = self.named.get_mut(&slot) {
            existing.text.push_str(content);
        } else {
            self.named
                .insert(slot, RegisterContent::characterwise(content.to_string()));
        }
        true
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

    // ========================================================================
    // Register enum tests (#515 Phase 5)
    // ========================================================================

    #[test]
    fn test_register_default_is_bank() {
        assert!(Register::Default.is_bank_register());
        assert!(!Register::Default.is_read_only());
        assert!(!Register::Default.is_session_scoped());
    }

    #[test]
    fn test_register_slot_is_bank() {
        let slot = Register::Slot('a');
        assert!(slot.is_bank_register());
        assert!(!slot.is_read_only());
        assert!(!slot.is_session_scoped());
    }

    #[test]
    fn test_register_history_is_read_only() {
        let history = Register::History(0);
        assert!(!history.is_bank_register());
        assert!(history.is_read_only());
        assert!(!history.is_session_scoped());
    }

    #[test]
    fn test_register_history_max() {
        let history = Register::History(255);
        assert!(history.is_read_only());
    }

    #[test]
    fn test_register_system() {
        let sys = Register::System;
        assert!(!sys.is_bank_register());
        assert!(!sys.is_read_only());
        assert!(!sys.is_session_scoped());
    }

    #[test]
    fn test_register_session_is_session_scoped() {
        let session = Register::Session('A');
        assert!(!session.is_bank_register());
        assert!(!session.is_read_only());
        assert!(session.is_session_scoped());
    }

    #[test]
    fn test_register_peer_history() {
        let peer = Register::PeerHistory {
            client: 1,
            index: 5,
        };
        assert!(!peer.is_bank_register());
        assert!(peer.is_read_only());
        assert!(peer.is_session_scoped());
    }

    #[test]
    fn test_register_eq() {
        assert_eq!(Register::Default, Register::Default);
        assert_eq!(Register::Slot('a'), Register::Slot('a'));
        assert_ne!(Register::Slot('a'), Register::Slot('b'));
        assert_eq!(Register::History(5), Register::History(5));
        assert_ne!(Register::History(0), Register::History(1));
        assert_eq!(Register::System, Register::System);
        assert_eq!(Register::Session('A'), Register::Session('A'));
        assert_ne!(Register::Session('A'), Register::Session('B'));
        assert_eq!(
            Register::PeerHistory {
                client: 1,
                index: 0
            },
            Register::PeerHistory {
                client: 1,
                index: 0
            }
        );
        assert_ne!(
            Register::PeerHistory {
                client: 1,
                index: 0
            },
            Register::PeerHistory {
                client: 2,
                index: 0
            }
        );
    }

    #[test]
    fn test_register_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Register::Default);
        set.insert(Register::Slot('a'));
        set.insert(Register::System);
        set.insert(Register::Session('A'));
        set.insert(Register::History(0));
        set.insert(Register::PeerHistory {
            client: 1,
            index: 0,
        });
        assert_eq!(set.len(), 6);
        // Duplicate should not increase count
        set.insert(Register::Default);
        assert_eq!(set.len(), 6);
    }

    #[test]
    fn test_register_debug() {
        let debug = format!("{:?}", Register::Default);
        assert!(debug.contains("Default"));

        let debug = format!("{:?}", Register::Slot('z'));
        assert!(debug.contains("Slot"));
        assert!(debug.contains('z'));

        let debug = format!(
            "{:?}",
            Register::PeerHistory {
                client: 42,
                index: 7
            }
        );
        assert!(debug.contains("PeerHistory"));
    }

    #[test]
    fn test_register_clone_copy() {
        let reg = Register::Slot('m');
        let cloned = reg;
        assert_eq!(cloned, reg);
    }

    #[test]
    fn test_register_all_variants_predicates() {
        // Exhaustive predicate coverage for all variants
        let variants = [
            Register::Default,
            Register::Slot('a'),
            Register::History(0),
            Register::System,
            Register::Session('A'),
            Register::PeerHistory {
                client: 0,
                index: 0,
            },
        ];

        let expected_bank = [true, true, false, false, false, false];
        let expected_readonly = [false, false, true, false, false, true];
        let expected_session = [false, false, false, false, true, true];

        for (i, variant) in variants.iter().enumerate() {
            assert_eq!(
                variant.is_bank_register(),
                expected_bank[i],
                "is_bank_register mismatch for {variant:?}"
            );
            assert_eq!(
                variant.is_read_only(),
                expected_readonly[i],
                "is_read_only mismatch for {variant:?}"
            );
            assert_eq!(
                variant.is_session_scoped(),
                expected_session[i],
                "is_session_scoped mismatch for {variant:?}"
            );
        }
    }

    // ========================================================================
    // RegisterBank get_register/set_register/append_slot tests (#515 Phase 5)
    // ========================================================================

    #[test]
    fn test_get_register_default() {
        let mut bank = RegisterBank::new();
        bank.set(RegisterContent::characterwise("hello"));
        assert_eq!(
            bank.get_register(&Register::Default)
                .map(|r| r.text.as_str()),
            Some("hello")
        );
    }

    #[test]
    fn test_get_register_slot() {
        let mut bank = RegisterBank::new();
        bank.set_named('a', RegisterContent::characterwise("alpha"));
        assert_eq!(
            bank.get_register(&Register::Slot('a'))
                .map(|r| r.text.as_str()),
            Some("alpha")
        );
    }

    #[test]
    fn test_get_register_slot_empty() {
        let bank = RegisterBank::new();
        assert!(bank.get_register(&Register::Slot('z')).is_none());
    }

    #[test]
    fn test_get_register_slot_uppercase_returns_none() {
        let mut bank = RegisterBank::new();
        bank.set_named('a', RegisterContent::characterwise("val"));
        // Uppercase slots are not bank registers
        assert!(bank.get_register(&Register::Slot('A')).is_none());
    }

    #[test]
    fn test_get_register_non_bank_variants_return_none() {
        let bank = RegisterBank::new();
        assert!(bank.get_register(&Register::History(0)).is_none());
        assert!(bank.get_register(&Register::System).is_none());
        assert!(bank.get_register(&Register::Session('A')).is_none());
        assert!(
            bank.get_register(&Register::PeerHistory {
                client: 0,
                index: 0
            })
            .is_none()
        );
    }

    #[test]
    fn test_set_register_default() {
        let mut bank = RegisterBank::new();
        assert!(bank.set_register(&Register::Default, RegisterContent::characterwise("set")));
        assert_eq!(bank.get().text, "set");
    }

    #[test]
    fn test_set_register_slot() {
        let mut bank = RegisterBank::new();
        assert!(bank.set_register(&Register::Slot('b'), RegisterContent::characterwise("bravo")));
        assert_eq!(bank.get_named('b').map(|r| r.text.as_str()), Some("bravo"));
    }

    #[test]
    fn test_set_register_slot_uppercase_fails() {
        let mut bank = RegisterBank::new();
        assert!(!bank.set_register(&Register::Slot('A'), RegisterContent::characterwise("nope")));
    }

    #[test]
    fn test_set_register_non_bank_variants_fail() {
        let mut bank = RegisterBank::new();
        assert!(!bank.set_register(&Register::History(0), RegisterContent::characterwise("nope")));
        assert!(!bank.set_register(&Register::System, RegisterContent::characterwise("nope")));
        assert!(
            !bank.set_register(&Register::Session('A'), RegisterContent::characterwise("nope"))
        );
        assert!(!bank.set_register(
            &Register::PeerHistory {
                client: 0,
                index: 0
            },
            RegisterContent::characterwise("nope")
        ));
    }

    #[test]
    fn test_set_register_overwrites() {
        let mut bank = RegisterBank::new();
        bank.set_register(&Register::Slot('a'), RegisterContent::characterwise("first"));
        bank.set_register(&Register::Slot('a'), RegisterContent::characterwise("second"));
        assert_eq!(
            bank.get_register(&Register::Slot('a'))
                .map(|r| r.text.as_str()),
            Some("second")
        );
    }

    #[test]
    fn test_append_slot_existing() {
        let mut bank = RegisterBank::new();
        bank.set_named('a', RegisterContent::characterwise("hello"));
        assert!(bank.append_slot('a', " world"));
        assert_eq!(bank.get_named('a').map(|r| r.text.as_str()), Some("hello world"));
    }

    #[test]
    fn test_append_slot_new() {
        let mut bank = RegisterBank::new();
        assert!(bank.append_slot('b', "created"));
        assert_eq!(bank.get_named('b').map(|r| r.text.as_str()), Some("created"));
    }

    #[test]
    fn test_append_slot_invalid_uppercase() {
        let mut bank = RegisterBank::new();
        assert!(!bank.append_slot('A', "nope"));
    }

    #[test]
    fn test_append_slot_invalid_digit() {
        let mut bank = RegisterBank::new();
        assert!(!bank.append_slot('1', "nope"));
    }

    #[test]
    fn test_append_slot_all_lowercase() {
        let mut bank = RegisterBank::new();
        for c in 'a'..='z' {
            assert!(bank.append_slot(c, &format!("val-{c}")));
        }
        assert_eq!(bank.get_named('a').map(|r| r.text.as_str()), Some("val-a"));
        assert_eq!(bank.get_named('z').map(|r| r.text.as_str()), Some("val-z"));
    }
}
