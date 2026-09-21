//! Global command selection and line deletion.

use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::String;

use reovim_pattern::{Pattern, Spent};

use crate::operator::{Extent, Operator};
use crate::{Effect, Vim};

/// A run's still-live lines, in order, and its own command.
#[derive(Debug, Default)]
pub struct Global {
    pub(crate) lines: VecDeque<usize>,
    pub(crate) command: String,
    pub(crate) busy: bool,
}

impl Vim {
    /// Select everything before any command changes the text.
    pub(crate) fn picked(
        &mut self,
        range: (usize, usize),
        spec: &str,
        invert: bool,
    ) -> bool {
        if self.global.busy {
            self.error("E147: Cannot do :global recursive");
            return false;
        }
        let Some(delimiter) = spec.chars().next() else {
            self.error("E148: Regular expression missing from global");
            return false;
        };
        let rest = &spec[delimiter.len_utf8()..];
        let (pattern, command) = split(rest, delimiter);
        let source = if pattern.is_empty() {
            self.search.pattern.clone()
        } else {
            String::from(pattern)
        };
        self.search.pattern.clone_from(&source);
        let options = self.search.options;
        let Some(pattern) =
            Pattern::parse(&source, options.ignorecase, options.smartcase)
        else {
            self.not_found();
            return false;
        };
        let found: Result<VecDeque<_>, Spent> = self
            .text
            .split('\n')
            .take(self.line_count())
            .enumerate()
            .skip(range.0)
            .take(
                range.1.min(self.line_count() - 1).saturating_sub(range.0) + 1,
            )
            .filter_map(|(line, text)| {
                pattern
                    .find(text, 0)
                    .map(|found| (found.is_some() != invert).then_some(line))
                    .transpose()
            })
            .collect();
        let Ok(lines) = found else {
            self.too_costly();
            return false;
        };
        let word = if invert { "found in every line" } else { "not found" };
        if lines.is_empty() {
            self.message = format!("Pattern {word}: {source}");
            return false;
        }
        self.global = Global { lines, command: command.into(), busy: true };
        self.set_previous();
        true
    }

    /// The command runs after this borrow ends, through the usual path.
    pub(crate) fn next_marked(&mut self) -> Option<Effect> {
        let line = self.global.lines.pop_front()?;
        self.cursor = self.line_at(line)?;
        self.want = self.wanted();
        let command = self.global.command.clone();
        Some(self.ex(&command))
    }

    /// Delete whole lines, using the normal operator's register policy.
    pub(crate) fn delete(
        &mut self,
        range: (usize, usize),
        register: &str,
    ) -> Effect {
        let mut chars = register.chars();
        let name = chars.next();
        let valid = name.is_none_or(|c| {
            c.is_ascii_alphabetic() || matches!(c, '_' | '+' | '*' | '-')
        });
        if !valid || chars.next().is_some() {
            self.error("E488: Trailing characters");
            return Effect::Nothing;
        }
        let Some(from) = self.line_at(range.0) else { return Effect::Nothing };
        let to = self.line_at(range.1).unwrap_or_else(|| self.bound());
        self.pending.register = name;
        let effect =
            self.apply(Operator::Delete, (from, to, Extent::Linewise), false);
        self.cursor = self.first_non_blank(self.line_start(self.cursor));
        self.keep_want = false;
        self.want = self.wanted();
        effect
    }

    /// Pass two peeks; the frame remains responsible for taking the bell.
    pub(crate) const fn ringing(&self) -> bool {
        self.bell
    }
}

/// Keep the command whole, including any further delimiters of its own.
fn split(spec: &str, delimiter: char) -> (&str, &str) {
    let mut escaped = false;
    for (at, c) in spec.char_indices() {
        if c == delimiter && !escaped {
            return (&spec[..at], &spec[at + c.len_utf8()..]);
        }
        escaped = c == '\\' && !escaped;
    }
    (spec, "")
}

#[cfg(test)]
mod tests {
    use alloc::collections::VecDeque;
    use alloc::string::String;

    use crate::{Effect, Kind, Vim};

    fn on(text: &str) -> Vim {
        let mut vim = Vim::default();
        vim.loaded(text.as_bytes());
        vim
    }

    #[test]
    fn a_spent_pattern_marks_nothing_and_says_e363() {
        let text = alloc::format!("{}\nc\n", "a".repeat(30));
        let mut vim = on(&text);
        assert!(!vim.picked((0, 1), r"/\(a*\)*b/d", false));
        assert!(vim.message.starts_with("E363:"));
        assert!(vim.ringing());
        assert_eq!(vim.text(), text);
        assert_eq!(vim.global.lines, VecDeque::new());
        assert!(!vim.global.busy);
    }

    #[test]
    fn the_marked_lines_are_the_matches_or_the_rest() {
        let cases = [
            ((0, 2), "/x/d", false, &[0, 2][..]),
            ((0, 2), "/x/d", true, &[1][..]),
            ((1, 2), "/x/d", false, &[2][..]),
            ((0, 2), "#x#d", false, &[0, 2][..]),
            ((0, 2), r"/\/x/d", false, &[0][..]),
            ((0, 2), "//d", false, &[0, 2][..]),
        ];
        for (range, spec, invert, expected) in cases {
            let mut vim = on("a/x\nb\nc x\n");
            vim.search.pattern = String::from("x");
            assert!(vim.picked(range, spec, invert), "{spec}");
            assert_eq!(vim.global.lines, expected.to_vec(), "{spec}");
            assert_eq!(vim.global.command, "d");
            assert!(vim.global.busy);
        }
        let mut vim = on("a\n");
        assert!(!vim.picked((0, 0), "", false));
        assert!(vim.message.starts_with("E148:"));
        assert!(!vim.picked((0, 0), r"/\(/d", false));
        assert!(vim.message.starts_with("E486:"));
    }

    #[test]
    fn no_line_marked_says_so_and_rings_no_bell() {
        let mut vim = on("a\nb\n");
        assert!(!vim.picked((0, 1), "/zzz/d", false));
        assert_eq!(vim.message, "Pattern not found: zzz");
        assert!(!vim.ringing());
        assert!(!vim.picked((0, 1), "/./d", true));
        assert_eq!(vim.message, "Pattern found in every line: .");
        assert!(!vim.ringing());
        assert!(!vim.global.busy);
    }

    #[test]
    fn a_global_while_one_runs_is_e147() {
        let mut vim = on("a\na\n");
        assert!(vim.picked((0, 1), "/a/d", false));
        let before = vim.global.lines.clone();
        assert!(!vim.picked((0, 1), "/a/d", false));
        assert_eq!(vim.message, "E147: Cannot do :global recursive");
        assert_eq!(vim.global.lines, before);
        assert!(vim.global.busy);
    }

    #[test]
    fn a_run_over_five_thousand_lines_ends() {
        let mut vim = on(&"a\n".repeat(5000));
        assert!(vim.picked((0, 4999), "/^/d", false));
        while vim.next_marked().is_some() {
            let before = vim.position();
            let mut effect = vim.delete((before.0, before.0), "");
            vim.changing(before, &mut effect, false);
        }
        assert!(vim.text().is_empty() || vim.text() == "\n");
        assert_eq!(vim.position(), (0, 0));
        assert_eq!(vim.register().unwrap().text, "a");
    }

    #[test]
    fn delete_fills_the_register_and_lands_on_the_first_non_blank() {
        let mut vim = on("  a\n  b\n  c\n");
        assert_ne!(vim.delete((0, 0), "q"), Effect::Nothing);
        assert_eq!(vim.text(), "  b\n  c\n");
        assert_eq!(vim.position(), (0, 2));
        assert_eq!(vim.want, vim.wanted());
        assert!(!vim.keep_want);
        assert_eq!(vim.registers.get(&'q').unwrap().text, "  a");
        assert_eq!(vim.registers.get(&'q').unwrap().kind, Kind::Line);
        assert_eq!(vim.delete((0, 0), "2"), Effect::Nothing);
        assert_eq!(vim.message, "E488: Trailing characters");
        assert_eq!(vim.text(), "  b\n  c\n");
    }
}
