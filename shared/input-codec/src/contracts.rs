use reovim_subsys_input_contracts::{KeySequence, ToKeyToken};

use crate::{KeyCode, KeyEvent, Modifiers};

impl ToKeyToken for KeyEvent {
    fn to_key_token(&self) -> String {
        key_event_to_contract_token(self)
    }
}

/// Convert a typed key event into the canonical contract token string.
#[must_use]
pub fn key_event_to_contract_token(event: &KeyEvent) -> String {
    let has_ctrl = event.modifiers.contains(Modifiers::CTRL);
    let has_alt = event.modifiers.contains(Modifiers::ALT);
    let has_shift = event.modifiers.contains(Modifiers::SHIFT);
    let has_any_mod = has_ctrl || has_alt || has_shift;

    let mut prefix = String::new();
    if has_ctrl {
        prefix.push_str("C-");
    }
    if has_alt {
        prefix.push_str("A-");
    }
    if has_shift {
        prefix.push_str("S-");
    }

    match event.code {
        KeyCode::Char(c) => {
            if has_any_mod {
                let key = match c {
                    '<' => "lt".to_owned(),
                    '>' => "gt".to_owned(),
                    ' ' => "Space".to_owned(),
                    _ => c.to_string(),
                };
                format!("<{prefix}{key}>")
            } else if c == '<' {
                "<lt>".to_owned()
            } else if c == '>' {
                "<gt>".to_owned()
            } else {
                c.to_string()
            }
        }
        KeyCode::Escape => format!("<{prefix}Esc>"),
        KeyCode::Enter => format!("<{prefix}Enter>"),
        KeyCode::Tab => format!("<{prefix}Tab>"),
        KeyCode::Backspace => format!("<{prefix}BS>"),
        KeyCode::Delete => format!("<{prefix}Del>"),
        KeyCode::Up => format!("<{prefix}Up>"),
        KeyCode::Down => format!("<{prefix}Down>"),
        KeyCode::Left => format!("<{prefix}Left>"),
        KeyCode::Right => format!("<{prefix}Right>"),
        KeyCode::Home => format!("<{prefix}Home>"),
        KeyCode::End => format!("<{prefix}End>"),
        KeyCode::PageUp => format!("<{prefix}PageUp>"),
        KeyCode::PageDown => format!("<{prefix}PageDown>"),
        KeyCode::F(n) => format!("<{prefix}F{n}>"),
        KeyCode::BackTab => "<S-Tab>".to_owned(),
        _ => "<?>".to_owned(),
    }
}

/// Adapt a contract-owned key sequence into typed key events.
#[must_use]
pub fn key_sequence_to_key_events(sequence: &KeySequence) -> Option<Vec<KeyEvent>> {
    sequence
        .as_slice()
        .iter()
        .map(|token| contract_token_to_key_event(token))
        .collect()
}

fn contract_token_to_key_event(token: &str) -> Option<KeyEvent> {
    if let Some(spec) = token
        .strip_prefix('<')
        .and_then(|rest| rest.strip_suffix('>'))
    {
        let mut modifiers = Modifiers::empty();
        let mut remaining = spec;

        loop {
            if let Some(rest) = remaining.strip_prefix("C-") {
                modifiers |= Modifiers::CTRL;
                remaining = rest;
            } else if let Some(rest) = remaining.strip_prefix("A-") {
                modifiers |= Modifiers::ALT;
                remaining = rest;
            } else if let Some(rest) = remaining.strip_prefix("S-") {
                modifiers |= Modifiers::SHIFT;
                remaining = rest;
            } else {
                break;
            }
        }

        let lowered = remaining.to_lowercase();
        let code = match lowered.as_str() {
            "esc" => KeyCode::Escape,
            "enter" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "space" => KeyCode::Char(' '),
            "bs" => KeyCode::Backspace,
            "del" => KeyCode::Delete,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "lt" => KeyCode::Char('<'),
            "gt" => KeyCode::Char('>'),
            _ => {
                if let Some(function) = lowered.strip_prefix('f') {
                    let value = function.parse::<u8>().ok()?;
                    KeyCode::F(value)
                } else if lowered.chars().count() == 1 {
                    KeyCode::Char(lowered.chars().next()?)
                } else {
                    return None;
                }
            }
        };

        return Some(KeyEvent::with_modifiers(code, modifiers));
    }

    let mut chars = token.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(KeyEvent::new(KeyCode::Char(first)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_sequence_adapters_cover_single_and_multi_token_paths() {
        let single = KeySequence::parse("j").unwrap();
        let single_typed = key_sequence_to_key_events(&single).unwrap();
        assert_eq!(single_typed, vec![KeyEvent::new(KeyCode::Char('j'))]);

        let multi = KeySequence::parse("<C-w>h").unwrap();
        let multi_typed = key_sequence_to_key_events(&multi).unwrap();
        assert_eq!(
            multi_typed,
            vec![
                KeyEvent::with_modifiers(KeyCode::Char('w'), Modifiers::CTRL),
                KeyEvent::new(KeyCode::Char('h')),
            ]
        );
    }

    #[test]
    fn contract_sequence_adapter_rejects_unknown_tokens() {
        let sequence = KeySequence::from_keys(&["<InvalidKey>"]);
        assert!(key_sequence_to_key_events(&sequence).is_none());
    }

    #[test]
    fn key_event_contract_tokens_roundtrip_for_common_cases() {
        let cases = [
            KeyEvent::new(KeyCode::Char('j')),
            KeyEvent::new(KeyCode::Escape),
            KeyEvent::with_modifiers(KeyCode::Char('w'), Modifiers::CTRL),
            KeyEvent::with_modifiers(KeyCode::Char(' '), Modifiers::CTRL),
            KeyEvent::with_modifiers(KeyCode::Char('<'), Modifiers::ALT),
        ];

        for event in cases {
            let token = key_event_to_contract_token(&event);
            let reparsed = key_sequence_to_key_events(&KeySequence::from_keys(&[token.as_str()]))
                .unwrap()
                .into_iter()
                .next()
                .unwrap();
            assert_eq!(reparsed.code, event.code);
            assert_eq!(reparsed.modifiers, event.modifiers);
        }
    }
}
