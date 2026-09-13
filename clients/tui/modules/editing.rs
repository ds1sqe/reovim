//! First-party scalar editing policy over a qualified Text window.

use crate::context::Window;
use crate::input::{Action, Code, Input, Key, Modifiers};
use crate::viewport::step;
use alloc::string::{String, ToString};

#[must_use]
pub fn quit(key: Key) -> bool {
    key.code == Code::Char('q') && key.modifiers == Modifiers::CONTROL
}

#[must_use]
pub fn discard(key: Key) -> bool {
    key.code == Code::Char('d') && key.modifiers == Modifiers::CONTROL
}

#[must_use]
pub fn recover(key: Key) -> bool {
    key.code == Code::Function(6) && key.modifiers.is_empty()
}

#[must_use]
pub fn refresh(key: Key) -> bool {
    key.code == Code::Function(5) && key.modifiers.is_empty()
}

/// Resolves input only within this validated semantic window.
/// Motion changes the local cursor; edits return an intent without moving it.
#[must_use]
pub fn action(
    input: Input,
    window: &Window,
    cursor: &mut usize,
) -> Option<Action> {
    if *cursor > window.units.len() {
        return None;
    }
    let position =
        window.range.start.checked_add(u64::try_from(*cursor).ok()?)?;
    if position > window.range.end {
        return None;
    }
    match input {
        Input::Text(text) => {
            Some(Action::Insert(position, position, text, window.range.start))
        }
        Input::Key(key) => key_action(key, window, cursor, position),
    }
}

fn key_action(
    key: Key,
    window: &Window,
    cursor: &mut usize,
    position: u64,
) -> Option<Action> {
    let insert =
        |end, text| Action::Insert(position, end, text, window.range.start);
    match key.code {
        Code::Left => {
            *cursor = step(&window.units, *cursor, false);
            None
        }
        Code::Right => {
            *cursor = step(&window.units, *cursor, true);
            None
        }
        Code::Function(5) if refresh(key) => {
            Some(Action::Observe(window.range.start))
        }
        Code::PageDown if !window.eof => {
            Some(Action::Observe(window.range.end))
        }
        Code::PageUp => {
            Some(Action::Observe(window.range.start.saturating_sub(1024)))
        }
        Code::Char('z') if key.modifiers == Modifiers::CONTROL => {
            Some(Action::Undo)
        }
        Code::Char('y') if key.modifiers == Modifiers::CONTROL => {
            Some(Action::Redo)
        }
        Code::Char(ch)
            if !key.modifiers.intersects(
                Modifiers::CONTROL | Modifiers::ALT | Modifiers::META,
            ) =>
        {
            Some(insert(position, ch.to_string()))
        }
        Code::Enter => Some(insert(position, "\n".into())),
        Code::Tab if key.modifiers.is_empty() => {
            Some(insert(position, "\t".into()))
        }
        Code::Backspace if *cursor > 0 => Some(Action::Insert(
            window.range.start + step(&window.units, *cursor, false) as u64,
            position,
            String::new(),
            window.range.start,
        )),
        Code::Delete if position < window.range.end => Some(insert(
            window.range.start + step(&window.units, *cursor, true) as u64,
            String::new(),
        )),
        _ => None,
    }
}
