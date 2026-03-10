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
    /// Boolean flag (e.g., `linewise`, `find_inclusive`).
    Bool,
    /// Buffer identifier (set by runner before command execution).
    BufferId,
    /// Single character (e.g., for find-char operations).
    Char,
    /// All remaining text after previous arguments (e.g., `:colorscheme dark`).
    Rest,
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
    /// A boolean flag (e.g., `linewise`, `find_inclusive`).
    Bool(bool),
    /// A buffer identifier (raw usize, converted to `BufferId` by helper).
    BufferId(usize),
    /// A single character (e.g., for find-char targets).
    Char(char),
    /// A position (line, column) for operator ranges (Epic #415).
    Position(usize, usize),
    /// A window identifier (raw usize, converted to `WindowId` by helper).
    WindowId(usize),
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
        assert_eq!(spec.description, "Number of times");
    }

    #[test]
    fn test_arg_spec_optional() {
        let spec = ArgSpec::optional("register", ArgKind::Register, "Target register");
        assert_eq!(spec.name, "register");
        assert!(!spec.required);
        assert_eq!(spec.kind, ArgKind::Register);
        assert_eq!(spec.description, "Target register");
    }

    #[test]
    fn test_arg_spec_all_kinds_required() {
        let kinds = [
            ArgKind::Count,
            ArgKind::Register,
            ArgKind::Motion,
            ArgKind::Range,
            ArgKind::FilePath,
            ArgKind::String,
            ArgKind::Bang,
            ArgKind::Bool,
            ArgKind::BufferId,
            ArgKind::Char,
            ArgKind::Rest,
        ];
        for kind in kinds {
            let spec = ArgSpec::required("test", kind, "desc");
            assert!(spec.required);
            assert_eq!(spec.kind, kind);
        }
    }

    #[test]
    fn test_arg_spec_all_kinds_optional() {
        let kinds = [
            ArgKind::Count,
            ArgKind::Register,
            ArgKind::Motion,
            ArgKind::Range,
            ArgKind::FilePath,
            ArgKind::String,
            ArgKind::Bang,
            ArgKind::Bool,
            ArgKind::BufferId,
            ArgKind::Char,
            ArgKind::Rest,
        ];
        for kind in kinds {
            let spec = ArgSpec::optional("test", kind, "desc");
            assert!(!spec.required);
            assert_eq!(spec.kind, kind);
        }
    }

    #[test]
    fn test_arg_spec_clone() {
        let spec = ArgSpec::required("count", ArgKind::Count, "Number");
        #[allow(clippy::redundant_clone)]
        let cloned = spec.clone();
        assert_eq!(cloned.name, "count");
        assert_eq!(cloned.kind, ArgKind::Count);
        assert!(cloned.required);
        assert_eq!(cloned.description, "Number");
    }

    #[test]
    fn test_arg_spec_debug() {
        let spec = ArgSpec::required("count", ArgKind::Count, "Number");
        let debug_str = format!("{spec:?}");
        assert!(debug_str.contains("ArgSpec"));
        assert!(debug_str.contains("count"));
    }

    // === ArgKind tests ===

    #[test]
    fn test_arg_kind_equality() {
        assert_eq!(ArgKind::Count, ArgKind::Count);
        assert_eq!(ArgKind::Register, ArgKind::Register);
        assert_ne!(ArgKind::Count, ArgKind::Register);
        assert_ne!(ArgKind::Motion, ArgKind::Range);
        assert_ne!(ArgKind::FilePath, ArgKind::String);
        assert_ne!(ArgKind::Bang, ArgKind::Bool);
        assert_ne!(ArgKind::Bang, ArgKind::BufferId);
        assert_ne!(ArgKind::Char, ArgKind::Count);
        assert_ne!(ArgKind::Rest, ArgKind::String);
    }

    #[test]
    fn test_arg_kind_copy_clone() {
        let kind = ArgKind::Motion;
        let copied = kind;
        #[allow(clippy::clone_on_copy)]
        let cloned = kind.clone();
        assert_eq!(kind, copied);
        assert_eq!(kind, cloned);
    }

    #[test]
    fn test_arg_kind_debug() {
        assert_eq!(format!("{:?}", ArgKind::Count), "Count");
        assert_eq!(format!("{:?}", ArgKind::Register), "Register");
        assert_eq!(format!("{:?}", ArgKind::Motion), "Motion");
        assert_eq!(format!("{:?}", ArgKind::Range), "Range");
        assert_eq!(format!("{:?}", ArgKind::FilePath), "FilePath");
        assert_eq!(format!("{:?}", ArgKind::String), "String");
        assert_eq!(format!("{:?}", ArgKind::Bang), "Bang");
        assert_eq!(format!("{:?}", ArgKind::Bool), "Bool");
        assert_eq!(format!("{:?}", ArgKind::BufferId), "BufferId");
        assert_eq!(format!("{:?}", ArgKind::Char), "Char");
        assert_eq!(format!("{:?}", ArgKind::Rest), "Rest");
    }

    // === ArgValue tests ===

    #[test]
    fn test_arg_value_count() {
        let val = ArgValue::Count(42);
        assert_eq!(val, ArgValue::Count(42));
        assert_ne!(val, ArgValue::Count(0));
    }

    #[test]
    fn test_arg_value_register() {
        let val = ArgValue::Register('a');
        assert_eq!(val, ArgValue::Register('a'));
        assert_ne!(val, ArgValue::Register('b'));
    }

    #[test]
    fn test_arg_value_motion() {
        let val = ArgValue::Motion("word".to_string());
        assert_eq!(val, ArgValue::Motion("word".to_string()));
        assert_ne!(val, ArgValue::Motion("line".to_string()));
    }

    #[test]
    fn test_arg_value_range() {
        let val = ArgValue::Range(1, 10);
        assert_eq!(val, ArgValue::Range(1, 10));
        assert_ne!(val, ArgValue::Range(1, 20));
    }

    #[test]
    fn test_arg_value_file_path() {
        let val = ArgValue::FilePath("/tmp/test.txt".to_string());
        assert_eq!(val, ArgValue::FilePath("/tmp/test.txt".to_string()));
    }

    #[test]
    fn test_arg_value_string() {
        let val = ArgValue::String("hello".to_string());
        assert_eq!(val, ArgValue::String("hello".to_string()));
    }

    #[test]
    fn test_arg_value_bang() {
        let val_true = ArgValue::Bang(true);
        let val_false = ArgValue::Bang(false);
        assert_eq!(val_true, ArgValue::Bang(true));
        assert_eq!(val_false, ArgValue::Bang(false));
        assert_ne!(val_true, val_false);
    }

    #[test]
    fn test_arg_value_bool() {
        let val_true = ArgValue::Bool(true);
        let val_false = ArgValue::Bool(false);
        assert_eq!(val_true, ArgValue::Bool(true));
        assert_eq!(val_false, ArgValue::Bool(false));
        assert_ne!(val_true, val_false);
        assert_ne!(val_true, ArgValue::Bang(true));
    }

    #[test]
    fn test_arg_value_buffer_id() {
        let val = ArgValue::BufferId(7);
        assert_eq!(val, ArgValue::BufferId(7));
        assert_ne!(val, ArgValue::BufferId(8));
    }

    #[test]
    fn test_arg_value_char() {
        let val = ArgValue::Char('x');
        assert_eq!(val, ArgValue::Char('x'));
        assert_ne!(val, ArgValue::Char('y'));
    }

    #[test]
    fn test_arg_value_window_id() {
        let val = ArgValue::WindowId(3);
        assert_eq!(val, ArgValue::WindowId(3));
        assert_ne!(val, ArgValue::WindowId(4));
        assert_ne!(val, ArgValue::BufferId(3));
    }

    #[test]
    fn test_arg_value_position() {
        let val = ArgValue::Position(10, 5);
        assert_eq!(val, ArgValue::Position(10, 5));
        assert_ne!(val, ArgValue::Position(10, 6));
        assert_ne!(val, ArgValue::Position(11, 5));
    }

    #[test]
    fn test_arg_value_clone() {
        let val = ArgValue::Motion("test".to_string());
        let cloned = val.clone();
        assert_eq!(val, cloned);
    }

    #[test]
    fn test_arg_value_debug() {
        let val = ArgValue::Count(3);
        let debug_str = format!("{val:?}");
        assert!(debug_str.contains("Count"));
        assert!(debug_str.contains('3'));
    }

    #[test]
    fn test_arg_value_cross_variant_inequality() {
        // Different variants are always unequal
        let count = ArgValue::Count(1);
        let register = ArgValue::Register('a');
        let string = ArgValue::String("1".to_string());
        let bang = ArgValue::Bang(true);
        let bool_val = ArgValue::Bool(true);
        let char_val = ArgValue::Char('a');

        assert_ne!(count, register);
        assert_ne!(count, string);
        assert_ne!(count, bang);
        assert_ne!(bang, bool_val);
        assert_ne!(register, char_val);
    }
}
