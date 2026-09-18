//! A typed line into the record a run takes, and that rule run
//! backwards, which is the form `help` prints and a Miss shows.
//!
//! Three kinds of field, told from the declaration alone, which carries
//! names and types. The environment, `cwd: Rd`, `width: u64` and
//! `color: bool`, the shell fills from where the line stands and what
//! the host's screen is, and no token ever matches it. A flag is a
//! `bool` field or any field named by one letter, which a token under a
//! dash names: by the field's whole name, a `bool` set and any other
//! type taking the next token, or letter by letter, each letter naming
//! a `bool` by its own one-letter name or by its first letter, case for
//! case. Every other field is a positional, filled in declared order
//! and then the sequence, a `utf8` taking the token and a number taking
//! it parsed.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write as _;

use reovim_kernel_interface::{Field, Ty};
use reovim_sdk::dynamic::{self, Value};
use reovim_sdk::walk::ROOT;

use crate::{Miss, Shell};

/// The bytes of the record `ty`, filled from `tokens` by the three
/// kinds of field and from `shell` for the environment.
///
/// # Errors
/// The token no field could take, beside the form `help` prints.
pub fn record(
    shell: &Shell,
    name: &str,
    ty: &Ty,
    tokens: &[&str],
) -> Result<Vec<u8>, Miss> {
    let Ty::Record(fields) = ty else {
        return Err(missed(name, "", &[]));
    };
    let filled = |field: &Field| {
        if environment(field) {
            return env(shell, &field.name);
        }
        empty(&field.ty)
    };
    let mut values: Vec<Value> = fields.iter().map(filled).collect();
    let left = flags(fields, name, tokens, &mut values)?;
    positionals(fields, name, &left, &mut values)?;
    let named = fields.iter().map(|field| field.name.clone());
    let record = Value::Record(named.zip(values).collect());
    let typed = dynamic::encode(ty, &record);
    typed.map_err(|_| missed(name, &tokens.join(" "), fields))
}

/// The line `help` prints for `name`: a part per field a token fills,
/// every one bracketed, since a declaration says which fields exist and
/// not which are needed.
pub fn form(name: &str, fields: &[Field]) -> String {
    let mut out = name.to_string();
    for field in fields {
        if environment(field) {
            continue;
        }
        let at = &field.name;
        let part = if !flag(field) {
            match field.ty {
                Ty::Sequence(..) => format!("[{at}..]"),
                _ => format!("[{at}]"),
            }
        } else if field.ty == Ty::Bool {
            format!("[{}]", dash(field))
        } else {
            format!("[{} {}]", dash(field), at.to_uppercase())
        };
        let _ = write!(out, " {part}");
    }
    out
}

/// Every flag field as a token under a dash spells it. What Tab offers
/// under a dash.
pub fn dashes(fields: &[Field]) -> Vec<String> {
    let flags = fields.iter().filter(|f| !environment(f) && flag(f));
    flags.map(dash).collect()
}

/// One flag field under a dash: its own one letter, which is the first
/// letter of any longer name.
fn dash(field: &Field) -> String {
    let letter: String = field.name.chars().take(1).collect();
    format!("-{letter}")
}

/// Every token under a dash taken by the flag field it names; what is
/// left over is the positionals', in the order it was typed.
fn flags<'a>(
    fields: &[Field],
    name: &str,
    tokens: &[&'a str],
    values: &mut [Value],
) -> Result<Vec<&'a str>, Miss> {
    let mut left = Vec::new();
    let mut tokens = tokens.iter().copied();
    while let Some(token) = tokens.next() {
        let Some(letters) = dashed(token) else {
            left.push(token);
            continue;
        };
        let Some(at) = whole(fields, letters) else {
            let each = each(fields, letters, values);
            each.ok_or_else(|| missed(name, token, fields))?;
            continue;
        };
        if fields[at].ty == Ty::Bool {
            values[at] = Value::Bool(true);
            continue;
        }
        let next = tokens.next().ok_or_else(|| missed(name, token, fields))?;
        let value = taken(&fields[at].ty, next);
        values[at] = value.ok_or_else(|| missed(name, next, fields))?;
    }
    Ok(left)
}

/// Every letter of a token under a dash naming a `bool` flag, each of
/// them set; `None` where one letter names none.
fn each(fields: &[Field], letters: &str, values: &mut [Value]) -> Option<()> {
    for letter in letters.chars() {
        values[letter_of(fields, letter)?] = Value::Bool(true);
    }
    Some(())
}

/// The tokens no flag took, by the positional fields in declared order
/// and then the sequence, which takes everything left. A token with no
/// field left to take it is the Miss.
fn positionals(
    fields: &[Field],
    name: &str,
    left: &[&str],
    values: &mut [Value],
) -> Result<(), Miss> {
    let (mut scalars, mut rest) = (Vec::new(), None);
    for (at, field) in fields.iter().enumerate() {
        if environment(field) || flag(field) {
            continue;
        }
        match &field.ty {
            Ty::Sequence(_, item) => rest = Some((at, item.as_ref())),
            _ => scalars.push((at, field)),
        }
    }
    let mut left = left.iter().copied();
    for (at, field) in scalars {
        let Some(token) = left.next() else { return Ok(()) };
        let value = taken(&field.ty, token);
        values[at] = value.ok_or_else(|| missed(name, token, fields))?;
    }
    let Some((at, item)) = rest else {
        let over = left.next();
        return over.map_or(Ok(()), |token| Err(missed(name, token, fields)));
    };
    let mut items = Vec::new();
    for token in left {
        let value = taken(item, token);
        items.push(value.ok_or_else(|| missed(name, token, fields))?);
    }
    values[at] = Value::Sequence(items);
    Ok(())
}

/// The three fields the shell fills and no token matches, told by their
/// names and their types together.
fn environment(field: &Field) -> bool {
    matches!(
        (field.name.as_str(), &field.ty),
        ("cwd", Ty::Rd) | ("width", Ty::U64) | ("color", Ty::Bool)
    )
}

/// A flag: a `bool` field, or any field named by one letter.
fn flag(field: &Field) -> bool {
    field.ty == Ty::Bool || one(&field.name)
}

/// The letters of a token under a dash, or `None` for a positional; a
/// dash by itself is a target and names no field.
fn dashed(token: &str) -> Option<&str> {
    token.strip_prefix('-').filter(|letters| !letters.is_empty())
}

/// The flag field `letters` is the whole name of.
fn whole(fields: &[Field], letters: &str) -> Option<usize> {
    let named = |field: &Field| {
        !environment(field) && flag(field) && field.name == letters
    };
    fields.iter().position(named)
}

/// The `bool` flag `letter` names: by its own one-letter name, else by
/// its first letter, so `t` and `T` are two fields and not one.
fn letter_of(fields: &[Field], letter: char) -> Option<usize> {
    let begins = |field: &Field| {
        !environment(field)
            && field.ty == Ty::Bool
            && field.name.starts_with(letter)
    };
    let itself = |field: &Field| begins(field) && one(&field.name);
    fields.iter().position(itself).or_else(|| fields.iter().position(begins))
}

/// A name of one letter.
fn one(name: &str) -> bool {
    let mut letters = name.chars();
    letters.next().is_some() && letters.next().is_none()
}

/// The environment field of that name, from where the line stands and
/// what the host's screen is.
fn env(shell: &Shell, name: &str) -> Value {
    match name {
        "cwd" => Value::Rd(shell.cwd),
        "width" => Value::U64(shell.width),
        _ => Value::Bool(shell.color),
    }
}

/// `token` as one value of `ty`: a `utf8` takes it, a number takes it
/// parsed, and one that does not parse is nobody's.
fn taken(ty: &Ty, token: &str) -> Option<Value> {
    let value = match ty {
        Ty::Utf8(_) => Value::Utf8(token.to_string()),
        Ty::Bytes(_) => Value::Bytes(token.as_bytes().to_vec()),
        Ty::U8 => Value::U8(token.parse().ok()?),
        Ty::U32 => Value::U32(token.parse().ok()?),
        Ty::U64 => Value::U64(token.parse().ok()?),
        Ty::I64 => Value::I64(token.parse().ok()?),
        Ty::F64 => Value::F64(token.parse().ok()?),
        _ => return None,
    };
    Some(value)
}

/// A field no token filled: the empty value of its type.
fn empty(ty: &Ty) -> Value {
    match ty {
        Ty::Unit => Value::Unit,
        Ty::Bool => Value::Bool(false),
        Ty::U8 => Value::U8(0),
        Ty::U32 => Value::U32(0),
        Ty::U64 => Value::U64(0),
        Ty::I64 => Value::I64(0),
        Ty::F64 => Value::F64(0.0),
        Ty::Rd => Value::Rd(ROOT),
        Ty::Bytes(_) => Value::Bytes(Vec::new()),
        Ty::Utf8(_) => Value::Utf8(String::new()),
        Ty::Sequence(..) => Value::Sequence(Vec::new()),
        Ty::Array(n, item) => {
            Value::Sequence((0..*n).map(|_| empty(item)).collect())
        }
        Ty::Record(fields) => {
            let named = |field: &Field| (field.name.clone(), empty(&field.ty));
            Value::Record(fields.iter().map(named).collect())
        }
        Ty::Choice(fields) => fields.first().map_or(Value::Unit, |field| {
            Value::Choice(field.name.clone(), Box::new(empty(&field.ty)))
        }),
    }
}

/// A token no field could take: the form `help` prints for this name.
fn missed(name: &str, token: &str, fields: &[Field]) -> Miss {
    let cause = format!("usage: {}", form(name, fields));
    let (name, target) = (name.to_string(), token.to_string());
    Miss { name, target, cause, unknown: false }
}
