//! A typed line into the record a run takes, and that rule run
//! backwards, which is the form `help` prints and a Miss shows.
//!
//! Three kinds of field, told from the declaration and the shell's own
//! environment, which carry names and types. An environment field is
//! one the host of the shell gave a value of that name and type for,
//! beside `cwd: Rd`, which is where the line stands; the shell fills it
//! and no token ever matches it. A flag is a
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
        return Err(missed(shell, name, "", &[]));
    };
    let filled =
        |field: &Field| given(shell, field).unwrap_or_else(|| empty(&field.ty));
    let mut values: Vec<Value> = fields.iter().map(filled).collect();
    let left = flags(shell, fields, name, tokens, &mut values)?;
    positionals(shell, fields, name, &left, &mut values)?;
    let named = fields.iter().map(|field| field.name.clone());
    let record = Value::Record(named.zip(values).collect());
    let typed = dynamic::encode(ty, &record);
    typed.map_err(|_| missed(shell, name, &tokens.join(" "), fields))
}

/// The line `help` prints for `name`: a part per field a token fills,
/// every one bracketed, since a declaration says which fields exist and
/// not which are needed.
pub fn form(shell: &Shell, name: &str, fields: &[Field]) -> String {
    let mut out = name.to_string();
    for field in fields {
        if given(shell, field).is_some() {
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
pub fn dashes(shell: &Shell, fields: &[Field]) -> Vec<String> {
    let mine = |f: &&Field| given(shell, f).is_none() && flag(f);
    fields.iter().filter(mine).map(dash).collect()
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
    shell: &Shell,
    fields: &[Field],
    name: &str,
    tokens: &[&'a str],
    values: &mut [Value],
) -> Result<Vec<&'a str>, Miss> {
    let mut left = Vec::new();
    let mut tokens = tokens.iter().copied();
    while let Some(token) = tokens.next() {
        let missed = |token| missed(shell, name, token, fields);
        let Some(letters) = dashed(token) else {
            left.push(token);
            continue;
        };
        let Some(at) = whole(shell, fields, letters) else {
            let each = each(shell, fields, letters, values);
            each.ok_or_else(|| missed(token))?;
            continue;
        };
        if fields[at].ty == Ty::Bool {
            values[at] = Value::Bool(true);
            continue;
        }
        let next = tokens.next().ok_or_else(|| missed(token))?;
        let value = taken(&fields[at].ty, next);
        values[at] = value.ok_or_else(|| missed(next))?;
    }
    Ok(left)
}

/// Every letter of a token under a dash naming a `bool` flag, each of
/// them set; `None` where one letter names none.
fn each(
    shell: &Shell,
    fields: &[Field],
    letters: &str,
    values: &mut [Value],
) -> Option<()> {
    for letter in letters.chars() {
        values[letter_of(shell, fields, letter)?] = Value::Bool(true);
    }
    Some(())
}

/// The tokens no flag took, by the positional fields in declared order
/// and then the sequence, which takes everything left. A token with no
/// field left to take it is the Miss.
fn positionals(
    shell: &Shell,
    fields: &[Field],
    name: &str,
    left: &[&str],
    values: &mut [Value],
) -> Result<(), Miss> {
    let missed = |token| missed(shell, name, token, fields);
    let (mut scalars, mut rest) = (Vec::new(), None);
    for (at, field) in fields.iter().enumerate() {
        if given(shell, field).is_some() || flag(field) {
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
        values[at] = value.ok_or_else(|| missed(token))?;
    }
    let Some((at, item)) = rest else {
        let over = left.next();
        return over.map_or(Ok(()), |token| Err(missed(token)));
    };
    let mut items = Vec::new();
    for token in left {
        let value = taken(item, token);
        items.push(value.ok_or_else(|| missed(token))?);
    }
    values[at] = Value::Sequence(items);
    Ok(())
}

/// What the shell fills a field with and no token matches: the cwd,
/// which is the shell's own and which `cd` moves, and whatever the host
/// gave a value of this name and this type for. `None` for a field the
/// host named nothing of, which is then a positional or a flag like any
/// other, so a Driver of anyone's may declare `bang` and mean a flag.
fn given(shell: &Shell, field: &Field) -> Option<Value> {
    if field.name == "cwd" && field.ty == Ty::Rd {
        return Some(Value::Rd(shell.cwd));
    }
    let held = shell.env.iter().find(|(name, _)| *name == field.name)?;
    fits(&held.1, &field.ty).then(|| held.1.clone())
}

/// Whether a value the host gave is of the type a field declares.
const fn fits(value: &Value, ty: &Ty) -> bool {
    matches!(
        (value, ty),
        (Value::Bool(_), Ty::Bool)
            | (Value::U8(_), Ty::U8)
            | (Value::U32(_), Ty::U32)
            | (Value::U64(_), Ty::U64)
            | (Value::I64(_), Ty::I64)
            | (Value::F64(_), Ty::F64)
            | (Value::Rd(_), Ty::Rd)
            | (Value::Bytes(_), Ty::Bytes(_))
            | (Value::Utf8(_), Ty::Utf8(_))
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
fn whole(shell: &Shell, fields: &[Field], letters: &str) -> Option<usize> {
    let named = |field: &Field| {
        given(shell, field).is_none() && flag(field) && field.name == letters
    };
    fields.iter().position(named)
}

/// The `bool` flag `letter` names: by its own one-letter name, else by
/// its first letter, so `t` and `T` are two fields and not one.
fn letter_of(shell: &Shell, fields: &[Field], letter: char) -> Option<usize> {
    let begins = |field: &Field| {
        given(shell, field).is_none()
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
fn missed(shell: &Shell, name: &str, token: &str, fields: &[Field]) -> Miss {
    let cause = format!("usage: {}", form(shell, name, fields));
    let (name, target) = (name.to_string(), token.to_string());
    Miss { name, target, cause, unknown: false }
}
