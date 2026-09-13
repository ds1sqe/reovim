use super::{Decoder, invalid, number, optional, string};
pub use reovim_client_tui::custody::RetainedInput;
use reovim_client_tui::custody::Submitted;
use reovim_client_tui::input::{Action, Code, Input, Key, Modifiers};
use reovim_kernel_interface::buffer::BufferState;
use std::{collections::VecDeque, io};

pub fn encode(out: &mut Vec<u8>, view: &RetainedInput) -> io::Result<()> {
    out.push(u8::from(view.basis.is_some()));
    if let Some(basis) = view.basis {
        number(out, basis.buffer.0);
        number(out, basis.revision);
    }
    number(out, view.cursor);
    optional(out, view.planned);
    out.extend([u8::from(view.pending), u8::from(view.halted)]);
    if view.queued.len() > 32 {
        return Err(invalid());
    }
    number(out, view.queued.len() as u64);
    for input in &view.queued {
        encode_input(out, input)?;
    }
    submitted(out, view.submitted.as_ref())?;
    Ok(())
}

pub fn decode(
    input: &mut Decoder<'_>,
    with_submitted: bool,
) -> io::Result<RetainedInput> {
    let basis = if input.flag()? {
        Some(BufferState {
            buffer: reovim_kernel_interface::ResourceId(input.number()?),
            revision: input.number()?,
        })
    } else {
        None
    };
    let cursor = input.number()?;
    let planned = input.optional()?;
    let pending = input.flag()?;
    let halted = input.flag()?;
    let count = input.number()?;
    if count > 32 || (basis.is_none() && cursor != 0) {
        return Err(invalid());
    }
    let mut queued = VecDeque::new();
    for _ in 0..count {
        queued.push_back(decode_input(input)?);
    }
    let submitted =
        if with_submitted { decode_submitted(input)? } else { None };
    Ok(RetainedInput {
        basis,
        cursor,
        planned,
        pending,
        halted,
        queued,
        submitted,
    })
}

fn submitted(out: &mut Vec<u8>, value: Option<&Submitted>) -> io::Result<()> {
    let Some(value) = value else {
        out.push(0);
        return Ok(());
    };
    out.push(1);
    number(out, value.basis.buffer.0);
    number(out, value.basis.revision);
    match &value.action {
        Action::Insert(start, end, text, origin) => {
            out.push(0);
            number(out, *start);
            number(out, *end);
            number(out, *origin);
            string(out, text)?;
        }
        Action::Undo => out.push(1),
        Action::Redo => out.push(2),
        _ => return Err(invalid()),
    }
    Ok(())
}

fn decode_submitted(input: &mut Decoder<'_>) -> io::Result<Option<Submitted>> {
    if !input.flag()? {
        return Ok(None);
    }
    let basis = BufferState {
        buffer: reovim_kernel_interface::ResourceId(input.number()?),
        revision: input.number()?,
    };
    let action = match input.byte()? {
        0 => {
            let start = input.number()?;
            let end = input.number()?;
            let origin = input.number()?;
            Action::Insert(start, end, input.string()?, origin)
        }
        1 => Action::Undo,
        2 => Action::Redo,
        _ => return Err(invalid()),
    };
    if action.cursor_after().is_none() {
        return Err(invalid());
    }
    Ok(Some(Submitted { basis, action }))
}

fn encode_input(out: &mut Vec<u8>, input: &Input) -> io::Result<()> {
    let Input::Key(key) = input else {
        let Input::Text(text) = input else {
            return Err(invalid());
        };
        out.push(0);
        return string(out, text);
    };
    let (tag, value) = match key.code {
        Code::Char(ch) => (1, u64::from(u32::from(ch))),
        Code::Enter => (2, 0),
        Code::Tab => (3, 0),
        Code::Backspace => (4, 0),
        Code::Delete => (5, 0),
        Code::Left => (6, 0),
        Code::Right => (7, 0),
        Code::Function(value) => (8, u64::from(value)),
        Code::PageUp => (9, 0),
        Code::PageDown => (10, 0),
    };
    out.extend([tag, key.modifiers.bits()]);
    number(out, value);
    Ok(())
}

fn decode_input(input: &mut Decoder<'_>) -> io::Result<Input> {
    let tag = input.byte()?;
    if tag == 0 {
        return input.string().map(Input::Text);
    }
    let modifiers = Modifiers::from_bits(input.byte()?).ok_or_else(invalid)?;
    let value = input.number()?;
    let code = match tag {
        1 => Code::Char(
            char::from_u32(u32::try_from(value).map_err(|_| invalid())?)
                .ok_or_else(invalid)?,
        ),
        2 if value == 0 => Code::Enter,
        3 if value == 0 => Code::Tab,
        4 if value == 0 => Code::Backspace,
        5 if value == 0 => Code::Delete,
        6 if value == 0 => Code::Left,
        7 if value == 0 => Code::Right,
        8 => Code::Function(u8::try_from(value).map_err(|_| invalid())?),
        9 if value == 0 => Code::PageUp,
        10 if value == 0 => Code::PageDown,
        _ => return Err(invalid()),
    };
    Ok(Input::Key(Key { code, modifiers }))
}
