//! Key notation rendering for different display formats.

use {
    super::{Key, KeySequence, Keystroke},
    std::fmt::Write,
};

/// Format for rendering key notation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyNotationFormat {
    /// Vim-style notation: `<C-d>`, `<S-Tab>`, `<Esc>`
    #[default]
    Vim,
    /// Human-readable notation: `Ctrl+D`, `Shift+Tab`, `Escape`
    Human,
    /// Status line display: `C-d`, `SPC`, `ESC` (compact)
    StatusLine,
}

impl Keystroke {
    /// Render the keystroke in the specified format.
    #[must_use]
    pub fn render(&self, format: KeyNotationFormat) -> String {
        match format {
            KeyNotationFormat::Vim => self.render_vim(),
            KeyNotationFormat::Human => self.render_human(),
            KeyNotationFormat::StatusLine => self.render_status_line(),
        }
    }

    /// Render in Vim notation: `<C-d>`, `<Esc>`, `a`
    #[must_use]
    pub fn render_vim(&self) -> String {
        let mut result = String::new();
        let needs_brackets = !self.modifiers.is_empty() || !matches!(self.key, Key::Char(_));

        if needs_brackets {
            result.push('<');
        }

        // Modifiers in vim order: C-, S-, A-
        if self.modifiers.ctrl {
            result.push_str("C-");
        }
        if self.modifiers.shift {
            result.push_str("S-");
        }
        if self.modifiers.alt {
            result.push_str("A-");
        }

        // Key
        match &self.key {
            Key::Char(c) => result.push(*c),
            Key::Space => result.push_str("Space"),
            Key::Escape => result.push_str("Esc"),
            Key::Enter => result.push_str("CR"),
            Key::Tab => result.push_str("Tab"),
            Key::Backspace => result.push_str("BS"),
            Key::Delete => result.push_str("Del"),
            Key::Insert => result.push_str("Ins"),
            Key::Home => result.push_str("Home"),
            Key::End => result.push_str("End"),
            Key::PageUp => result.push_str("PageUp"),
            Key::PageDown => result.push_str("PageDown"),
            Key::Up => result.push_str("Up"),
            Key::Down => result.push_str("Down"),
            Key::Left => result.push_str("Left"),
            Key::Right => result.push_str("Right"),
            Key::F(n) => {
                let _ = write!(&mut result, "F{n}");
            }
        }

        if needs_brackets {
            result.push('>');
        }

        result
    }

    /// Render in human-readable notation: `Ctrl+D`, `Escape`
    #[must_use]
    pub fn render_human(&self) -> String {
        let mut parts = Vec::new();

        if self.modifiers.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers.shift {
            parts.push("Shift".to_string());
        }
        if self.modifiers.alt {
            parts.push("Alt".to_string());
        }

        let key_str = match &self.key {
            Key::Char(c) => c.to_uppercase().to_string(),
            Key::Space => "Space".to_string(),
            Key::Escape => "Escape".to_string(),
            Key::Enter => "Enter".to_string(),
            Key::Tab => "Tab".to_string(),
            Key::Backspace => "Backspace".to_string(),
            Key::Delete => "Delete".to_string(),
            Key::Insert => "Insert".to_string(),
            Key::Home => "Home".to_string(),
            Key::End => "End".to_string(),
            Key::PageUp => "PageUp".to_string(),
            Key::PageDown => "PageDown".to_string(),
            Key::Up => "Up".to_string(),
            Key::Down => "Down".to_string(),
            Key::Left => "Left".to_string(),
            Key::Right => "Right".to_string(),
            Key::F(n) => format!("F{n}"),
        };

        parts.push(key_str);
        parts.join("+")
    }

    /// Render for status line display: `C-d`, `SPC`, `ESC` (compact)
    #[must_use]
    pub fn render_status_line(&self) -> String {
        let mut result = String::new();

        // Compact modifiers
        if self.modifiers.ctrl {
            result.push_str("C-");
        }
        if self.modifiers.shift {
            result.push_str("S-");
        }
        if self.modifiers.alt {
            result.push_str("A-");
        }

        match &self.key {
            Key::Char(c) => result.push(*c),
            Key::Space => result.push_str("SPC"),
            Key::Escape => result.push_str("ESC"),
            Key::Enter => result.push_str("RET"),
            Key::Tab => result.push_str("TAB"),
            Key::Backspace => result.push_str("BS"),
            Key::Delete => result.push_str("DEL"),
            Key::Insert => result.push_str("INS"),
            Key::Home => result.push_str("HOME"),
            Key::End => result.push_str("END"),
            Key::PageUp => result.push_str("PGUP"),
            Key::PageDown => result.push_str("PGDN"),
            Key::Up => result.push_str("UP"),
            Key::Down => result.push_str("DN"),
            Key::Left => result.push_str("LT"),
            Key::Right => result.push_str("RT"),
            Key::F(n) => {
                let _ = write!(&mut result, "F{n}");
            }
        }

        result
    }
}

impl KeySequence {
    /// Render the sequence in the specified format.
    #[must_use]
    pub fn render(&self, format: KeyNotationFormat) -> String {
        let separator = match format {
            KeyNotationFormat::StatusLine => " ",
            _ => "",
        };

        self.0
            .iter()
            .map(|k| k.render(format))
            .collect::<Vec<_>>()
            .join(separator)
    }

    /// Render in Vim notation.
    #[must_use]
    pub fn render_vim(&self) -> String {
        self.render(KeyNotationFormat::Vim)
    }

    /// Render in human-readable notation.
    #[must_use]
    pub fn render_human(&self) -> String {
        self.render(KeyNotationFormat::Human)
    }

    /// Render for status line display.
    #[must_use]
    pub fn render_status_line(&self) -> String {
        self.render(KeyNotationFormat::StatusLine)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::keystroke::Modifiers};

    #[test]
    fn test_vim_notation_simple_char() {
        let k = Keystroke::char('a');
        assert_eq!(k.render_vim(), "a");
    }

    #[test]
    fn test_vim_notation_ctrl() {
        let k = Keystroke::ctrl(Key::Char('d'));
        assert_eq!(k.render_vim(), "<C-d>");
    }

    #[test]
    fn test_vim_notation_ctrl_shift() {
        let k = Keystroke::new(Key::Char('h'), Modifiers::CTRL.with_shift());
        assert_eq!(k.render_vim(), "<C-S-h>");
    }

    #[test]
    fn test_vim_notation_escape() {
        let k = Keystroke::escape();
        assert_eq!(k.render_vim(), "<Esc>");
    }

    #[test]
    fn test_vim_notation_space() {
        let k = Keystroke::space();
        assert_eq!(k.render_vim(), "<Space>");
    }

    #[test]
    fn test_human_notation_simple_char() {
        let k = Keystroke::char('a');
        assert_eq!(k.render_human(), "A");
    }

    #[test]
    fn test_human_notation_ctrl() {
        let k = Keystroke::ctrl(Key::Char('d'));
        assert_eq!(k.render_human(), "Ctrl+D");
    }

    #[test]
    fn test_human_notation_escape() {
        let k = Keystroke::escape();
        assert_eq!(k.render_human(), "Escape");
    }

    #[test]
    fn test_status_line_notation() {
        let k = Keystroke::space();
        assert_eq!(k.render_status_line(), "SPC");

        let k = Keystroke::ctrl(Key::Char('d'));
        assert_eq!(k.render_status_line(), "C-d");
    }

    #[test]
    fn test_sequence_vim() {
        let seq = KeySequence::from_vec(vec![Keystroke::char('g'), Keystroke::char('g')]);
        assert_eq!(seq.render_vim(), "gg");
    }

    #[test]
    fn test_sequence_leader() {
        let seq = KeySequence::from_vec(vec![
            Keystroke::space(),
            Keystroke::char('f'),
            Keystroke::char('f'),
        ]);
        assert_eq!(seq.render_vim(), "<Space>ff");
    }

    #[test]
    fn test_sequence_status_line() {
        let seq = KeySequence::from_vec(vec![
            Keystroke::space(),
            Keystroke::char('f'),
            Keystroke::char('f'),
        ]);
        assert_eq!(seq.render_status_line(), "SPC f f");
    }
}
