//! Keys as text: `ihello<Esc>:wq<CR>`.
//!
//! It is a spelling of [`crate::Key`] and not of any one Plugin,
//! so it stands beside `Key`: the Vim Plugin's cases are driven with
//! it and so is `keys`, the name that types into another Client.

use alloc::vec::Vec;

use crate::Key;

/// The keys of `text`, as Vim writes them.
///
/// `<Esc>`, `<CR>`, `<BS>`, `<Tab>`, `<Left>`, `<Right>`, `<Up>`,
/// `<Down>`, `<Home>`, `<End>`, `<C-x>`, `<M-x>`, `<Space>` and `<lt>`
/// are named; any other char is itself. `None` for a `<` that names
/// nothing: `<M->` and `<M-xy>` name no char, and `<A-x>` is not a
/// spelling this notation reads.
#[must_use]
pub fn keys(text: &str) -> Option<Vec<Key>> {
    let mut keys = Vec::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c != '<' {
            keys.push(Key::Char(c));
            rest = &rest[c.len_utf8()..];
            continue;
        }
        let end = rest.find('>')?;
        keys.push(named(&rest[1..end])?);
        rest = &rest[end + 1..];
    }
    Some(keys)
}

fn named(name: &str) -> Option<Key> {
    let key = match name {
        "Esc" => Key::Esc,
        "CR" | "Enter" => Key::Enter,
        "BS" => Key::Backspace,
        "Tab" => Key::Tab,
        "Left" => Key::Left,
        "Right" => Key::Right,
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Home" => Key::Home,
        "End" => Key::End,
        "Space" => Key::Char(' '),
        "lt" => Key::Char('<'),
        _ => {
            if let Some(letter) = name.strip_prefix("M-") {
                let mut chars = letter.chars();
                return match (chars.next(), chars.next()) {
                    (Some(c), None) => Some(Key::Alt(c)),
                    _ => None,
                };
            }
            let letter = name.strip_prefix("C-")?;
            let mut chars = letter.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Key::Ctrl(c.to_ascii_lowercase()),
                _ => return None,
            }
        }
    };
    Some(key)
}
