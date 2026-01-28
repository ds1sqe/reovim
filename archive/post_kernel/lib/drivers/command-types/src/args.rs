//! Argument types for self-describing commands.
//!
//! This module provides the argument specification system for commands:
//! - [`ArgSpec`] - Metadata about a command argument
//! - [`ArgKind`] - The type of argument (count, register, motion, etc.)
//! - [`ArgValue`] - Parsed argument value from user input

/// Argument specification for self-describing commands.
///
/// Defines metadata about a command argument including its name, type,
/// description, and whether it's required.
#[derive(Debug, Clone)]
pub struct ArgSpec {
    /// The argument name (used in `CommandContext::get`).
    pub name: &'static str,
    /// Human-readable description.
    pub description: &'static str,
    /// The kind of argument.
    pub kind: ArgKind,
    /// Whether this argument is required.
    pub required: bool,
}

impl ArgSpec {
    /// Create a required argument specification.
    #[must_use]
    pub const fn required(name: &'static str, kind: ArgKind, description: &'static str) -> Self {
        Self {
            name,
            description,
            kind,
            required: true,
        }
    }

    /// Create an optional argument specification.
    #[must_use]
    pub const fn optional(name: &'static str, kind: ArgKind, description: &'static str) -> Self {
        Self {
            name,
            description,
            kind,
            required: false,
        }
    }
}

/// The kind of argument a command accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    /// Numeric count (e.g., `3j` for "move down 3 lines").
    Count,
    /// Register name (e.g., `"a` for register 'a').
    Register,
    /// Motion command (e.g., `w` in `dw` for "delete word").
    Motion,
    /// Line range (e.g., `1,5` for "lines 1 through 5").
    Range,
    /// File path (e.g., `foo.txt` in `:w foo.txt`).
    FilePath,
    /// Generic string argument.
    String,
    /// Bang modifier (e.g., `!` in `:q!`).
    Bang,
    /// Buffer identifier (set by runner before command execution).
    BufferId,
    /// Single character (e.g., for find-char operations).
    Char,
}

/// Argument value parsed from user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgValue {
    /// A numeric count.
    Count(usize),
    /// A register name.
    Register(char),
    /// A motion identifier (command ID as string for simplicity).
    Motion(String),
    /// A line range (start, end).
    Range(usize, usize),
    /// A file path.
    FilePath(String),
    /// A generic string.
    String(String),
    /// A bang modifier.
    Bang(bool),
    /// A buffer identifier (raw usize, converted to `BufferId` by helper).
    BufferId(usize),
    /// A single character (e.g., for find-char targets).
    Char(char),
    /// A position (line, column) for operator ranges (Epic #415).
    Position(usize, usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arg_spec_required() {
        let spec = ArgSpec::required("count", ArgKind::Count, "Number of times");
        assert_eq!(spec.name, "count");
        assert!(spec.required);
        assert_eq!(spec.kind, ArgKind::Count);
    }

    #[test]
    fn test_arg_spec_optional() {
        let spec = ArgSpec::optional("register", ArgKind::Register, "Target register");
        assert_eq!(spec.name, "register");
        assert!(!spec.required);
        assert_eq!(spec.kind, ArgKind::Register);
    }
}
