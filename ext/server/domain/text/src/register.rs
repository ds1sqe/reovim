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
/// use reovim_domain_text::YankType;
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

    /// Return the yank type as a string label ("char" or "line").
    ///
    /// Used by gRPC serialization to avoid importing `YankType` directly.
    #[must_use]
    pub const fn yank_type_str(&self) -> &'static str {
        match self.yank_type {
            YankType::Characterwise => "char",
            YankType::Linewise => "line",
        }
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
/// use reovim_domain_text::Register;
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
/// use reovim_domain_text::*;
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
    /// use reovim_domain_text::*;
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
    /// use reovim_domain_text::*;
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
#[path = "register_tests.rs"]
mod tests;
