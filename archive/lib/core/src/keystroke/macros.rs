//! Macros for creating keystrokes and key sequences.
//!
//! # Examples
//!
//! ```ignore
//! // Single keystroke
//! key!('a')           // character 'a'
//! key!(Space)         // space key (leader)
//! key!(Escape)        // escape
//! key!(Ctrl 'd')      // Ctrl+d
//! key!(Ctrl Shift 'h') // Ctrl+Shift+h
//! key!(Shift Tab)     // Shift+Tab
//!
//! // Key sequence
//! keys!['g', 'g']           // "gg"
//! keys![Space, 'f', 'f']    // leader-f-f
//! keys![Ctrl 'd']           // Ctrl+d (single key sequence)
//! keys!['d', 'd']           // "dd"
//! ```

/// Create a single `Keystroke`.
///
/// # Syntax
///
/// - `key!('a')` - character key
/// - `key!(Space)` - named key (Space, Escape, Enter, Tab, etc.)
/// - `key!(Ctrl 'x')` - Ctrl + character
/// - `key!(Ctrl Shift 'h')` - Ctrl + Shift + character
/// - `key!(Shift Tab)` - Shift + named key
/// - `key!(F 1)` - Function key F1
#[macro_export]
macro_rules! key {
    // Ctrl + Shift + character
    (Ctrl Shift $c:literal) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::Char($c),
            $crate::keystroke::Modifiers::CTRL.with_shift(),
        )
    };

    // Ctrl + Shift + named key
    (Ctrl Shift $name:ident) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::$name,
            $crate::keystroke::Modifiers::CTRL.with_shift(),
        )
    };

    // Ctrl + character
    (Ctrl $c:literal) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::Char($c),
            $crate::keystroke::Modifiers::CTRL,
        )
    };

    // Ctrl + named key
    (Ctrl $name:ident) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::$name,
            $crate::keystroke::Modifiers::CTRL,
        )
    };

    // Shift + character
    (Shift $c:literal) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::Char($c),
            $crate::keystroke::Modifiers::SHIFT,
        )
    };

    // Shift + named key
    (Shift $name:ident) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::$name,
            $crate::keystroke::Modifiers::SHIFT,
        )
    };

    // Alt + character
    (Alt $c:literal) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::Char($c),
            $crate::keystroke::Modifiers::ALT,
        )
    };

    // Alt + named key
    (Alt $name:ident) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::$name,
            $crate::keystroke::Modifiers::ALT,
        )
    };

    // Function key: key!(F 1) for F1
    (F $n:literal) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::F($n),
            $crate::keystroke::Modifiers::NONE,
        )
    };

    // Plain character
    ($c:literal) => {
        $crate::keystroke::Keystroke::char($c)
    };

    // Named keys without modifiers
    (Space) => {
        $crate::keystroke::Keystroke::space()
    };
    (Escape) => {
        $crate::keystroke::Keystroke::escape()
    };
    (Enter) => {
        $crate::keystroke::Keystroke::enter()
    };
    (Tab) => {
        $crate::keystroke::Keystroke::tab()
    };
    (Backspace) => {
        $crate::keystroke::Keystroke::backspace()
    };
    (Delete) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::Delete,
            $crate::keystroke::Modifiers::NONE,
        )
    };
    (Insert) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::Insert,
            $crate::keystroke::Modifiers::NONE,
        )
    };
    (Home) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::Home,
            $crate::keystroke::Modifiers::NONE,
        )
    };
    (End) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::End,
            $crate::keystroke::Modifiers::NONE,
        )
    };
    (PageUp) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::PageUp,
            $crate::keystroke::Modifiers::NONE,
        )
    };
    (PageDown) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::PageDown,
            $crate::keystroke::Modifiers::NONE,
        )
    };
    (Up) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::Up,
            $crate::keystroke::Modifiers::NONE,
        )
    };
    (Down) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::Down,
            $crate::keystroke::Modifiers::NONE,
        )
    };
    (Left) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::Left,
            $crate::keystroke::Modifiers::NONE,
        )
    };
    (Right) => {
        $crate::keystroke::Keystroke::new(
            $crate::keystroke::Key::Right,
            $crate::keystroke::Modifiers::NONE,
        )
    };
}

/// Create a `KeySequence` from multiple keystrokes.
///
/// # Syntax
///
/// Uses parentheses for modifier combinations:
/// ```ignore
/// keys!['g' 'g']              // two character keys: gg
/// keys![Space 'f' 'f']        // leader + f + f
/// keys![(Ctrl 'd')]           // single Ctrl+d
/// keys!['d' 'd']              // delete line: dd
/// keys![Escape]               // single escape
/// keys![Space 'w' 'h']        // leader + w + h
/// keys![(Ctrl Shift 'h')]     // Ctrl+Shift+h
/// ```
#[macro_export]
macro_rules! keys {
    // Empty sequence
    [] => {
        $crate::keystroke::KeySequence::new()
    };

    // Internal: accumulate keystrokes
    (@acc [$($acc:expr),*]) => {
        $crate::keystroke::KeySequence::from_vec(vec![$($acc),*])
    };

    // Ctrl Shift + char in parens
    (@acc [$($acc:expr),*] (Ctrl Shift $c:literal) $($rest:tt)*) => {
        $crate::keys!(@acc [$($acc,)* $crate::key!(Ctrl Shift $c)] $($rest)*)
    };

    // Ctrl Shift + named key in parens
    (@acc [$($acc:expr),*] (Ctrl Shift $name:ident) $($rest:tt)*) => {
        $crate::keys!(@acc [$($acc,)* $crate::key!(Ctrl Shift $name)] $($rest)*)
    };

    // Ctrl + char in parens
    (@acc [$($acc:expr),*] (Ctrl $c:literal) $($rest:tt)*) => {
        $crate::keys!(@acc [$($acc,)* $crate::key!(Ctrl $c)] $($rest)*)
    };

    // Ctrl + named key in parens
    (@acc [$($acc:expr),*] (Ctrl $name:ident) $($rest:tt)*) => {
        $crate::keys!(@acc [$($acc,)* $crate::key!(Ctrl $name)] $($rest)*)
    };

    // Shift + char in parens
    (@acc [$($acc:expr),*] (Shift $c:literal) $($rest:tt)*) => {
        $crate::keys!(@acc [$($acc,)* $crate::key!(Shift $c)] $($rest)*)
    };

    // Shift + named key in parens
    (@acc [$($acc:expr),*] (Shift $name:ident) $($rest:tt)*) => {
        $crate::keys!(@acc [$($acc,)* $crate::key!(Shift $name)] $($rest)*)
    };

    // Alt + char in parens
    (@acc [$($acc:expr),*] (Alt $c:literal) $($rest:tt)*) => {
        $crate::keys!(@acc [$($acc,)* $crate::key!(Alt $c)] $($rest)*)
    };

    // Alt + named key in parens
    (@acc [$($acc:expr),*] (Alt $name:ident) $($rest:tt)*) => {
        $crate::keys!(@acc [$($acc,)* $crate::key!(Alt $name)] $($rest)*)
    };

    // Function key in parens: (F 1)
    (@acc [$($acc:expr),*] (F $n:literal) $($rest:tt)*) => {
        $crate::keys!(@acc [$($acc,)* $crate::key!(F $n)] $($rest)*)
    };

    // Plain character literal
    (@acc [$($acc:expr),*] $c:literal $($rest:tt)*) => {
        $crate::keys!(@acc [$($acc,)* $crate::key!($c)] $($rest)*)
    };

    // Named key (Space, Escape, Enter, etc.)
    (@acc [$($acc:expr),*] $name:ident $($rest:tt)*) => {
        $crate::keys!(@acc [$($acc,)* $crate::key!($name)] $($rest)*)
    };

    // Entry point: start accumulation
    [$($tokens:tt)+] => {
        $crate::keys!(@acc [] $($tokens)+)
    };
}

#[cfg(test)]
mod tests {
    use crate::keystroke::Key;

    #[test]
    fn test_key_char() {
        let k = key!('a');
        assert_eq!(k.key, Key::Char('a'));
        assert!(k.modifiers.is_empty());
    }

    #[test]
    fn test_key_space() {
        let k = key!(Space);
        assert_eq!(k.key, Key::Space);
    }

    #[test]
    fn test_key_escape() {
        let k = key!(Escape);
        assert_eq!(k.key, Key::Escape);
    }

    #[test]
    fn test_key_ctrl_char() {
        let k = key!(Ctrl 'd');
        assert_eq!(k.key, Key::Char('d'));
        assert!(k.modifiers.ctrl);
        assert!(!k.modifiers.shift);
    }

    #[test]
    fn test_key_ctrl_shift_char() {
        let k = key!(Ctrl Shift 'h');
        assert_eq!(k.key, Key::Char('h'));
        assert!(k.modifiers.ctrl);
        assert!(k.modifiers.shift);
    }

    #[test]
    fn test_key_shift_tab() {
        let k = key!(Shift Tab);
        assert_eq!(k.key, Key::Tab);
        assert!(k.modifiers.shift);
    }

    #[test]
    fn test_key_function() {
        let k = key!(F 1);
        assert_eq!(k.key, Key::F(1));
    }

    #[test]
    fn test_keys_empty() {
        let seq = keys![];
        assert!(seq.is_empty());
    }

    #[test]
    fn test_keys_single_char() {
        let seq = keys!['a'];
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.0[0].key, Key::Char('a'));
    }

    #[test]
    fn test_keys_double_char() {
        let seq = keys!['g' 'g'];
        assert_eq!(seq.len(), 2);
        assert_eq!(seq.0[0].key, Key::Char('g'));
        assert_eq!(seq.0[1].key, Key::Char('g'));
    }

    #[test]
    fn test_keys_leader_sequence() {
        let seq = keys![Space 'f' 'f'];
        assert_eq!(seq.len(), 3);
        assert_eq!(seq.0[0].key, Key::Space);
        assert_eq!(seq.0[1].key, Key::Char('f'));
        assert_eq!(seq.0[2].key, Key::Char('f'));
    }

    #[test]
    fn test_keys_ctrl() {
        let seq = keys![(Ctrl 'd')];
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.0[0].key, Key::Char('d'));
        assert!(seq.0[0].modifiers.ctrl);
    }

    #[test]
    fn test_keys_mixed() {
        let seq = keys![Space (Ctrl 'd')];
        assert_eq!(seq.len(), 2);
        assert_eq!(seq.0[0].key, Key::Space);
        assert_eq!(seq.0[1].key, Key::Char('d'));
        assert!(seq.0[1].modifiers.ctrl);
    }
}
