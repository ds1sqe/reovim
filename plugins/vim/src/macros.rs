//! Macro key storage and replay state.
//!
//! Control spelling covers ASCII letters and @, [, \, ], ^, _, ?.
//! Tab, Enter and Esc are the canonical i, m and [ aliases; ? is DEL.
//! Arbitrary constructed Unicode Ctrl values are outside this spelling,
//! not additional control characters promised to round-trip.

use crate::{Effect, Fed, Key, Kind, Register, Vim};
use alloc::string::String;
use alloc::vec::Vec;

/// Maximum fed keys in one run; typed input starts a fresh budget.
pub const BUDGET: usize = 100_000;

/// Recorded state survives a stay; execution flags belong to one stay.
#[derive(Debug, Default)]
pub struct Macros {
    pub recording: Option<char>,
    pub record: Vec<Key>,
    pub last: Option<char>,
    pub last_ex: String,
    pub fed: bool,
    pub joined: bool,
}

fn encode(keys: &[Key]) -> String {
    let mut text = String::new();
    for &key in keys {
        let c = match key {
            Key::Char(c) => c,
            Key::Esc => '\u{1b}',
            Key::Enter => '\r',
            Key::Tab => '\t',
            Key::Ctrl('?') => '\u{7f}',
            Key::Ctrl(c) => char::from_u32(u32::from(c) & 31).unwrap(),
            Key::Backspace => '\u{e000}',
            Key::Left => '\u{e001}',
            Key::Right => '\u{e002}',
            Key::Up => '\u{e003}',
            Key::Down => '\u{e004}',
            Key::Home => '\u{e005}',
            Key::End => '\u{e006}',
            Key::Alt(c) => {
                text.push('\u{e007}');
                c
            }
        };
        text.push(c);
    }
    text
}

fn decode(mut chars: impl Iterator<Item = char>) -> Vec<Key> {
    let mut keys = Vec::new();
    while keys.len() < BUDGET + 1 {
        let Some(c) = chars.next() else { break };
        let key = match c {
            '\u{1b}' => Key::Esc,
            '\u{7f}' => Key::Ctrl('?'),
            '\r' => Key::Enter,
            '\t' => Key::Tab,
            '\u{e000}' => Key::Backspace,
            '\u{e001}' => Key::Left,
            '\u{e002}' => Key::Right,
            '\u{e003}' => Key::Up,
            '\u{e004}' => Key::Down,
            '\u{e005}' => Key::Home,
            '\u{e006}' => Key::End,
            '\u{e007}' => chars.next().map_or(Key::Char(c), Key::Alt),
            '\0' | '\u{1c}'..='\u{1f}' => {
                Key::Ctrl(char::from_u32(u32::from(c) + 64).unwrap())
            }
            '\u{1}'..='\u{1a}' => {
                Key::Ctrl(char::from_u32(u32::from(c) + 96).unwrap())
            }
            _ => Key::Char(c),
        };
        keys.push(key);
    }
    keys
}

impl Vim {
    pub(crate) fn record_into(&mut self, name: char) -> Effect {
        self.macros.recording = Some(name);
        self.macros.record.clear();
        self.repeat.record.clear();
        Effect::Moved
    }

    pub(crate) fn recorded(&mut self) -> Effect {
        let Some(name) = self.macros.recording.take() else {
            return Effect::Nothing;
        };
        self.macros.record.pop();
        let text = encode(&core::mem::take(&mut self.macros.record));
        let taken = Register { text, kind: Kind::Char };
        if name.is_ascii_uppercase() {
            self.append(name.to_ascii_lowercase(), &taken);
        } else {
            self.registers.insert(name, taken);
        }
        self.repeat.record.clear();
        Effect::Moved
    }

    pub(crate) fn macro_run(&mut self, name: char, count: usize) -> Effect {
        self.repeat.record.clear();
        let name = if name == '@' { self.macros.last } else { Some(name) };
        let Some(name) = name else {
            return self.error("E748: No previously used register");
        };
        if name == ':' && self.macros.last_ex.is_empty() {
            return self.error("E30: No previous command line");
        }
        self.macros.last = Some(name);
        let (head, text, tail) = if name == ':' {
            (":", self.macros.last_ex.as_str(), "\r")
        } else if let Some(held) =
            self.registers.get(&name.to_ascii_lowercase())
        {
            let tail = match held.kind {
                Kind::Line => "\n",
                _ => "",
            };
            ("", held.text.as_str(), tail)
        } else {
            ("", "", "")
        };
        let keys = decode(head.chars().chain(text.chars()).chain(tail.chars()));
        let len = keys.len().saturating_mul(count).min(BUDGET + 1);
        let keys = keys.iter().copied().cycle().take(len).collect::<Vec<_>>();
        Effect::Fed(Fed { keys: crate::coded(&keys), repeat: false })
    }

    pub(crate) const fn ring(&mut self) {
        self.bell = true;
    }

    /// Takes the bell raised by an unsuccessful command.
    ///
    /// ```
    /// use reovim_client_api::{Key, NONE, keyboard::Press, origin};
    /// use reovim_plugin_vim::{Vim, keyed};
    /// let mut vim = Vim::default();
    /// vim.loaded(b"a\n");
    /// let none = Press { key: 0, when: 0, source: NONE };
    /// vim.key(keyed(Key::Char('h'), origin::TYPED, &none));
    /// assert!(vim.rung());
    /// assert!(!vim.rung());
    /// ```
    pub fn rung(&mut self) -> bool {
        core::mem::take(&mut self.bell)
    }

    pub fn error(&mut self, message: &str) -> Effect {
        self.message = String::from(message);
        self.ring();
        Effect::Moved
    }

    /// Edits of a fed or global run share its first revision.
    pub(crate) fn continued(&mut self, effect: &mut Effect) {
        let Effect::Edits(edits) = effect else { return };
        if !(self.macros.fed || self.global.busy) {
            return;
        }
        for edit in edits {
            edit.join |= self.macros.joined;
            self.macros.joined = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use reovim_client_api::origin::{FED, TYPED};

    use crate::{Effect, Fed, Key, Vim, coded, tapped};

    /// The keys a run answers with, as the record spells them.
    fn replayed(keys: &[Key]) -> Effect {
        Effect::Fed(Fed { keys: coded(keys), repeat: false })
    }

    #[test]
    fn decoding_stops_at_the_key_budget_without_reading_the_tail() {
        let text = alloc::format!("{}\u{e007}éTAIL", "x".repeat(BUDGET));
        let consumed = core::cell::Cell::new(0);
        let mut chars = text.chars().inspect(|_| {
            consumed.set(consumed.get() + 1);
        });
        let keys = decode(chars.by_ref());
        assert_eq!(
            consumed.get(),
            BUDGET + 2,
            "the last key is a complete two-scalar Alt, then reading stops"
        );
        assert_eq!(keys.last(), Some(&Key::Alt('é')));
        assert_eq!(chars.collect::<String>(), "TAIL");
    }

    #[test]
    fn bounded_preparation_preserves_repetition_and_implicit_keys() {
        let mut vim = Vim::default();
        vim.registers.insert(
            'a',
            Register { text: String::from("x"), kind: Kind::Line },
        );
        assert_eq!(
            vim.macro_run('a', 2),
            replayed(&[
                Key::Char('x'),
                Key::Ctrl('j'),
                Key::Char('x'),
                Key::Ctrl('j'),
            ])
        );
        vim.macros.last_ex = String::from("write");
        assert_eq!(
            vim.macro_run(':', 1),
            replayed(&[
                Key::Char(':'),
                Key::Char('w'),
                Key::Char('r'),
                Key::Char('i'),
                Key::Char('t'),
                Key::Char('e'),
                Key::Enter,
            ])
        );
        vim.registers.get_mut(&'a').unwrap().text = "x".repeat(BUDGET + 2);
        assert_eq!(
            vim.macro_run('a', 2),
            replayed(&alloc::vec![Key::Char('x'); BUDGET + 1])
        );
        vim.macros.last_ex = "x".repeat(BUDGET + 2);
        let mut expected = alloc::vec![Key::Char('x'); BUDGET + 1];
        expected[0] = Key::Char(':');
        assert_eq!(vim.macro_run(':', 2), replayed(&expected));
    }

    #[test]
    fn every_key_round_trips_and_controls_have_canonical_spellings() {
        let keys = [
            Key::Char('é'),
            Key::Esc,
            Key::Enter,
            Key::Tab,
            Key::Ctrl('a'),
            Key::Ctrl('j'),
            Key::Ctrl('@'),
            Key::Ctrl('\\'),
            Key::Ctrl(']'),
            Key::Ctrl('^'),
            Key::Ctrl('_'),
            Key::Backspace,
            Key::Left,
            Key::Right,
            Key::Up,
            Key::Down,
            Key::Home,
            Key::End,
            Key::Alt('é'),
        ];
        assert_eq!(decode(encode(&keys).chars()), keys);
        assert_eq!(
            decode(
                encode(&[Key::Ctrl('i'), Key::Ctrl('m'), Key::Ctrl('[')])
                    .chars()
            ),
            [Key::Tab, Key::Enter, Key::Esc]
        );
        assert_eq!(decode("\n".chars()), [Key::Ctrl('j')]);
    }

    #[test]
    fn del_is_control_question_mark_not_control_underscore() {
        assert_eq!(encode(&[Key::Ctrl('?')]), "\u{7f}");
        assert_eq!(decode("\u{7f}".chars()), [Key::Ctrl('?')]);
        assert_ne!(encode(&[Key::Ctrl('?')]), encode(&[Key::Ctrl('_')]));
    }

    #[test]
    fn leave_keeps_recording_and_last_run_but_ends_the_revision() {
        let mut vim = Vim::default();
        vim.loaded(b"abc\n");
        for key in [Key::Char('q'), Key::Char('a')] {
            vim.key(tapped(key, TYPED));
        }
        vim.key(tapped(Key::Char('l'), TYPED));
        vim.key(tapped(Key::Char('x'), FED));
        vim.ring();
        vim.macros.last = Some('b');
        vim.macros.last_ex = String::from("write");
        vim.leave();
        assert_eq!(vim.macros.recording, Some('a'));
        assert_eq!(vim.macros.record, [Key::Char('l')]);
        assert_eq!(vim.macros.last, Some('b'));
        assert_eq!(vim.macros.last_ex, "write");
        assert!(!vim.macros.fed);
        assert!(!vim.macros.joined);
        assert!(!vim.rung());
    }

    #[test]
    fn nested_dot_restores_the_callers_fed_context() {
        let mut vim = Vim::default();
        vim.loaded(b"abcdef\n");
        vim.key(tapped(Key::Char('x'), TYPED));
        let effect = vim.key(tapped(Key::Char('.'), FED));
        assert!(matches!(effect, Effect::Fed(..)), "keys for the queue");
        assert!(vim.macros.fed);
        assert!(!vim.repeat.replaying);
    }
}
