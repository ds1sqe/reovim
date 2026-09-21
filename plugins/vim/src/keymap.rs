//! The keymap: keys to a `:` line, the leader they may be spelled
//! with, and the keys a binding may still turn out to be.
//!
//! A binding is data: KEYS as a person types them, in the shared key
//! notation, and the `:` line they run. Nothing here reads a file or
//! a format, and nothing here runs anything; lib.rs asks what the map
//! makes of the keys held so far and does what the answer says.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_client_api::notation;

use crate::Key;

/// One key as it was taken: the key itself, and whether a macro fed
/// it rather than a person typing it.
///
/// The origin travels with the key through the keymap and back out to
/// the host's queue, so a bell or the replay budget can drop what a
/// macro fed and keep what was typed, and so a replayed key that a
/// binding let go still joins its macro's one revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stroke {
    /// The key itself.
    pub key: Key,
    /// Fed by a macro or a repeat rather than typed.
    pub fed: bool,
}

/// What the map makes of the keys held so far.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Fact {
    /// They are a binding, and no longer binding begins with them.
    Exact(String),
    /// They are a binding, and a longer one begins with them too: the
    /// line waits here until the next key settles which it is.
    Longer(String),
    /// They are no binding, but a longer one begins with them.
    Prefix,
    /// No binding begins with them.
    None,
}

/// The keys a binding may still be, and the keys a failed prefix let
/// go again.
///
/// One stay's, as the bell is: a Buffer left with a prefix half typed
/// does not carry it to the next.
#[derive(Debug, Default)]
pub struct Held {
    /// The keys held while the map may still answer for them.
    pub(crate) keys: Vec<Stroke>,
    /// The keys the map answered for none of, for the host's queue.
    pub(crate) released: Vec<Stroke>,
}

/// The bindings and the leader they may be spelled with.
///
/// `leader` lives here rather than with the search options because
/// those are `Copy` and a `String` cannot be a field of them; it is
/// still one option with one home and one writer, `set`.
#[derive(Debug)]
pub struct Keymap {
    /// KEYS as typed, to the `:` line they run.
    map: BTreeMap<String, String>,
    /// What `<Leader>` stands for, as typed.
    pub(crate) leader: String,
}

impl Default for Keymap {
    /// `<Leader>y` hands the point to the clipboard and `<M-x>`
    /// opens the cli buffer; the leader is `\`, as Vim's `mapleader`
    /// has it. `bind KEYS` may take either binding away.
    fn default() -> Self {
        let mut map = BTreeMap::new();
        map.insert(
            String::from("<Leader>y"),
            String::from("point > /self/dev/clipboard"),
        );
        map.insert(String::from("<M-x>"), String::from("cli"));
        Self { map, leader: String::from("\\") }
    }
}

impl Keymap {
    /// The keys KEYS spells, every `<Leader>` standing for the leader
    /// as it is now. `None` for a spelling the notation does not read
    /// and for one that spells no key at all.
    pub(crate) fn read(&self, keys: &str) -> Option<Vec<Key>> {
        let spelled = keys.replace("<Leader>", &self.leader);
        notation::keys(&spelled).filter(|keys| !keys.is_empty())
    }

    /// What the map makes of `keys`, every KEYS read at this call:
    /// that is what "the leader is read at the key" costs, and a
    /// keymap is small.
    ///
    /// Two KEYS may read the same once the leader has moved, so the
    /// first in the map's order answers.
    pub(crate) fn fact(&self, keys: &[Key]) -> Fact {
        let mut exact: Option<&String> = None;
        let mut longer = false;
        for (bound, line) in &self.map {
            let read = self.read(bound);
            let read = read.as_deref().unwrap_or_default();
            exact = exact.or_else(|| (read == keys).then_some(line));
            longer |= read.len() > keys.len() && read.starts_with(keys);
        }
        match (exact, longer) {
            (Some(line), false) => Fact::Exact(line.clone()),
            (Some(line), true) => Fact::Longer(line.clone()),
            (None, true) => Fact::Prefix,
            (None, false) => Fact::None,
        }
    }

    /// Every KEYS that reads the same as `keys`, taken away; true
    /// where there was one.
    pub(crate) fn unbind(&mut self, keys: &[Key]) -> bool {
        let gone: Vec<String> = self
            .map
            .keys()
            .filter(|bound| self.read(bound).as_deref() == Some(keys))
            .cloned()
            .collect();
        for bound in &gone {
            self.map.remove(bound);
        }
        !gone.is_empty()
    }

    /// KEYS put to `line`, whatever else read the same taken away.
    pub(crate) fn put(&mut self, keys: &[Key], spelled: &str, line: &str) {
        self.unbind(keys);
        self.map.insert(String::from(spelled), String::from(line));
    }

    /// What `/self/vim/keymap` reads: a line `bind KEYS LINE` a
    /// binding, in the map's order. It is a script that makes the
    /// bindings it lists and no more, so an unbound default is no
    /// line of it.
    pub(crate) fn text(&self) -> String {
        let mut out = String::new();
        for (keys, line) in &self.map {
            out.push_str("bind ");
            out.push_str(keys);
            out.push(' ');
            out.push_str(line);
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use reovim_client_api::notation::keys;

    use crate::{Effect, Key, Vim};

    /// A map of KEYS to a line each, under the default leader.
    fn map(bound: &[(&str, &str)]) -> Keymap {
        let mut keymap = Keymap::default();
        keymap.map.clear();
        for (keys, line) in bound {
            keymap.map.insert(String::from(*keys), String::from(*line));
        }
        keymap
    }

    #[test]
    fn the_four_facts() {
        let keymap =
            map(&[("\\w", "write"), ("\\wq", "wqall"), ("\\x", "xit")]);
        let fact = |text: &str| keymap.fact(&keys(text).unwrap());
        assert_eq!(fact("\\"), Fact::Prefix, "no binding, one begins here");
        assert_eq!(fact("\\w"), Fact::Longer(String::from("write")));
        assert_eq!(fact("\\x"), Fact::Exact(String::from("xit")));
        assert_eq!(fact("z"), Fact::None);
    }

    #[test]
    fn leader_is_read_at_the_call() {
        let mut keymap = map(&[("<Leader>w", "write")]);
        let write = Fact::Exact(String::from("write"));
        assert_eq!(keymap.fact(&keys("\\w").unwrap()), write);
        keymap.leader = String::from("<Space>");
        assert_eq!(keymap.fact(&keys("<Space>w").unwrap()), write);
        assert_eq!(keymap.fact(&keys("\\w").unwrap()), Fact::None);
    }

    #[test]
    fn leave_lets_go_of_a_half_held_prefix_and_keeps_the_keymap() {
        let mut vim = Vim::default();
        vim.loaded(b"abc\n");
        vim.bind("dx", "nohlsearch");
        assert_eq!(vim.key(Key::Char('d')), Effect::Nothing, "a prefix held");

        // The held keys are one stay's and go with it; the keymap is
        // kept, so the next `d` begins the prefix afresh rather than
        // the `x` finishing the one the stay before left half typed.
        vim.leave();
        assert_eq!(vim.keymap.fact(&keys("dx").unwrap()), xit());
        vim.key(Key::Char('x'));
        assert_eq!(vim.text(), "bc\n", "`x` was itself");
    }

    /// `dx`'s line as `fact` answers for it.
    fn xit() -> Fact {
        Fact::Exact(String::from("nohlsearch"))
    }

    #[test]
    fn a_binding_of_the_same_keys_replaces() {
        let mut vim = Vim::default();
        vim.bind("<CR>", "write");
        vim.bind("<Enter>", "quit");
        let quit = Fact::Exact(String::from("quit"));
        assert_eq!(vim.keymap.fact(&keys("<CR>").unwrap()), quit);
        assert_eq!(
            vim.keymap.text(),
            "bind <Enter> quit\n\
             bind <Leader>y point > /self/dev/clipboard\n\
             bind <M-x> cli\n",
            "one binding of those keys, beside the default"
        );
    }
}
