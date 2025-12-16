//! Count prefix parsing for commands (e.g., 5j moves down 5 lines)

use crate::modd::Mod;

/// Manages count prefix state for vim-style commands
#[derive(Debug, Default)]
pub struct CountParser {
    pending_count: Option<usize>,
}

impl CountParser {
    #[must_use]
    pub const fn new() -> Self {
        Self { pending_count: None }
    }

    /// Check if a key is a digit that should be accumulated as count
    #[must_use]
    pub fn is_count_digit(&self, key: &str, mode: &Mod) -> bool {
        // Only parse counts in Normal or Visual mode
        if !matches!(mode, Mod::Normal | Mod::Visual(_)) {
            return false;
        }

        if let Some(c) = key.chars().next()
            && c.is_ascii_digit()
        {
            // '0' is only a count digit if we already have a count started
            // Otherwise '0' is the line-start command
            if c == '0' {
                return self.pending_count.is_some();
            }
            return true;
        }
        false
    }

    /// Accumulate a digit into pending count
    #[allow(clippy::cast_possible_truncation)]
    pub fn accumulate(&mut self, key: &str) {
        if let Some(c) = key.chars().next()
            && let Some(digit) = c.to_digit(10)
        {
            let current = self.pending_count.unwrap_or(0);
            self.pending_count = Some(current * 10 + digit as usize);
        }
    }

    /// Get and clear the pending count
    #[allow(clippy::missing_const_for_fn)]
    pub fn take(&mut self) -> Option<usize> {
        self.pending_count.take()
    }

    /// Get current pending count without clearing
    #[must_use]
    pub const fn peek(&self) -> Option<usize> {
        self.pending_count
    }

    /// Check if there's a pending count
    #[must_use]
    #[allow(dead_code)]
    pub const fn has_count(&self) -> bool {
        self.pending_count.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_digit() {
        let mut parser = CountParser::new();
        parser.accumulate("5");
        assert_eq!(parser.take(), Some(5));
    }

    #[test]
    fn test_multi_digit() {
        let mut parser = CountParser::new();
        parser.accumulate("1");
        parser.accumulate("2");
        parser.accumulate("3");
        assert_eq!(parser.take(), Some(123));
    }

    #[test]
    fn test_zero_as_first_digit() {
        let parser = CountParser::new();
        // '0' without existing count should NOT be a count digit
        assert!(!parser.is_count_digit("0", &Mod::Normal));
    }

    #[test]
    fn test_zero_after_digit() {
        let mut parser = CountParser::new();
        parser.accumulate("1");
        // '0' with existing count SHOULD be a count digit
        assert!(parser.is_count_digit("0", &Mod::Normal));
        parser.accumulate("0");
        assert_eq!(parser.take(), Some(10));
    }

    #[test]
    fn test_count_only_in_normal_visual() {
        let parser = CountParser::new();
        assert!(parser.is_count_digit("5", &Mod::Normal));
        assert!(parser.is_count_digit("5", &Mod::Visual(crate::modd::ModExtension::Normal)));
        assert!(!parser.is_count_digit("5", &Mod::Insert(crate::modd::ModExtension::Normal)));
        assert!(!parser.is_count_digit("5", &Mod::Command));
    }

    #[test]
    fn test_take_clears_count() {
        let mut parser = CountParser::new();
        parser.accumulate("5");
        assert_eq!(parser.take(), Some(5));
        assert_eq!(parser.take(), None);
    }
}
