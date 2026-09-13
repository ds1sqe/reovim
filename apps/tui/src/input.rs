//! Terminal event normalization; editing policy belongs to the Client module.

use crossterm::event::{
    Event as NativeEvent, KeyCode, KeyEventKind, KeyModifiers,
};
use reovim_client_tui::input::{Code, Input, Key, Modifiers};

pub enum Event {
    Input(Input),
    Resize,
}

pub fn event(value: NativeEvent) -> Option<Event> {
    match value {
        NativeEvent::Resize(..) => Some(Event::Resize),
        value => input(value).map(Event::Input),
    }
}

pub fn key(event: &NativeEvent) -> Option<Key> {
    let NativeEvent::Key(key) = event else { return None };
    if key.kind == KeyEventKind::Release {
        return None;
    }
    Some(Key { code: code(key.code)?, modifiers: modifiers(key.modifiers)? })
}

pub fn input(event: NativeEvent) -> Option<Input> {
    match event {
        NativeEvent::Paste(text) => Some(Input::Text(text)),
        event => key(&event).map(Input::Key),
    }
}

const fn code(code: KeyCode) -> Option<Code> {
    Some(match code {
        KeyCode::Char(ch) => Code::Char(ch),
        KeyCode::Enter => Code::Enter,
        KeyCode::Tab => Code::Tab,
        KeyCode::Backspace => Code::Backspace,
        KeyCode::Delete => Code::Delete,
        KeyCode::Left => Code::Left,
        KeyCode::Right => Code::Right,
        KeyCode::F(number) => Code::Function(number),
        KeyCode::PageUp => Code::PageUp,
        KeyCode::PageDown => Code::PageDown,
        _ => return None,
    })
}

fn modifiers(raw: KeyModifiers) -> Option<Modifiers> {
    let known = KeyModifiers::CONTROL
        | KeyModifiers::ALT
        | KeyModifiers::SHIFT
        | KeyModifiers::SUPER
        | KeyModifiers::META;
    if !known.contains(raw) {
        return None;
    }
    let mut result = Modifiers::empty();
    for (native, logical) in [
        (KeyModifiers::CONTROL, Modifiers::CONTROL),
        (KeyModifiers::ALT, Modifiers::ALT),
        (KeyModifiers::SHIFT, Modifiers::SHIFT),
        (KeyModifiers::SUPER | KeyModifiers::META, Modifiers::META),
    ] {
        if raw.intersects(native) {
            result |= logical;
        }
    }
    Some(result)
}
