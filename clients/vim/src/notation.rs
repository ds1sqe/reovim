//! Keys as Vim writes them: `ihello<Esc>:wq<CR>`.

use alloc::vec::Vec;

use crate::Key;

/// The keys of `text`; `<Esc>`, `<CR>`, `<BS>`, `<Tab>`, `<Left>`,
/// `<Right>`, `<Up>`, `<Down>`, `<Home>`, `<End>`, `<C-x>` and `<lt>`
/// as Vim writes them, any other char as itself. `None` for a `<`
/// that names nothing.
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
        "lt" => Key::Char('<'),
        _ => {
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
