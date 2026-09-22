//! A typed line into the record a run takes, and that rule run
//! backwards, which is the form `help` prints and a Miss shows.
//!
//! Three kinds of field, told from the declaration, from the shell's
//! own environment and from the options its host named. An
//! environment field is one the host gave a value of that name and
//! type for, beside `cwd: Rd`, which is where the line stands; the
//! shell fills it and no token ever matches it. A flag is a `bool`
//! field, a field named by one letter, or a field the host named an
//! option of ([`Shell::option`]): no positional token reaches one. A
//! token under one dash names a `bool` or a one-letter field — by the
//! field's whole name, a `bool` set and any other type taking the
//! next token, or letter by letter, each letter naming a `bool` by
//! its own one-letter name or by its first letter, case for case. Two
//! dashes read wider and never letter by letter: the whole name of
//! any declared field at all, flag or not, so `--col` is `col` and
//! `--target` reaches a positional as readily, and a name past the
//! record's own is a Miss. Every other field is a positional, filled
//! in declared order and then the sequence, a `utf8` taking the token
//! and a number taking it parsed — unless it is the last positional
//! and a `utf8`, which [`record_verbatim`] fills instead, from
//! wherever its flags and its earlier positionals end to the end of
//! the line, blanks, `|` and `>` kept: [`verbatim`] is what tells
//! [`crate::run`] which door a name's declaration wants.

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

/// The last positional field of `fields`, if it is a `utf8` — the one
/// [`record_verbatim`] fills. `None` for a name whose last positional
/// is a sequence, or that declares none at all, which [`positionals`]
/// splits and fills as it always has.
fn verbatim_positional(shell: &Shell, fields: &[Field]) -> Option<usize> {
    let (at, field) = fields.iter().enumerate().rfind(|(_, field)| {
        given(shell, field).is_none() && !flag(shell, field)
    })?;
    matches!(field.ty, Ty::Utf8(_)).then_some(at)
}

/// How many positional fields `fields` declares — the environment and
/// the flags aside — [`verbatim`]'s own among them.
fn positional_count(shell: &Shell, fields: &[Field]) -> usize {
    let positional =
        |field: &&Field| given(shell, field).is_none() && !flag(shell, field);
    fields.iter().filter(positional).count()
}

/// Whether `fields`' last positional is a `utf8` and it is the one
/// positional the declaration has: `capture`'s `target` and `write`'s
/// `path`, a single field a trailing bare `>` after it may still be a
/// real redirect for, never `bind`'s `line` behind `keys`, where `>`
/// is as much its own text as any other blank-joined word is.
#[must_use]
pub fn verbatim_alone(shell: &Shell, fields: &[Field]) -> bool {
    verbatim_positional(shell, fields).is_some()
        && positional_count(shell, fields) == 1
}

/// The rule, in one sentence: when the last positional is a `utf8`
/// and more tokens stand than the positionals can take once the flags
/// are read, the rest of the line from there is one value, blanks,
/// `|` and `>` kept.
///
/// So a line the tokenized fill already answers with nothing left
/// over takes that path instead, unquestioned, and a declaration
/// whose flags are not always first (`tree`, `stat`) keeps exactly
/// the reading it always had, its own flags read from `rest` wherever
/// they stand, as [`flags`] always has. One exemption stands before
/// this is asked at all: for a declaration of one positional alone
/// (`capture`'s `target`, `write`'s `path`) a trailing bare `>` and
/// the word after it are set aside as the redirect they are
/// ([`verbatim_alone`], which [`crate::run`] reads first), never for
/// `bind`'s `line` behind its `keys`, where `>` is as much its own
/// text as any other blank-joined word is.
#[must_use]
pub fn verbatim(
    shell: &Shell,
    fields: &[Field],
    rest: &[&str],
) -> Option<usize> {
    let at = verbatim_positional(shell, fields)?;
    let mut values: Vec<Value> = fields
        .iter()
        .map(|field| given(shell, field).unwrap_or_else(|| empty(&field.ty)))
        .collect();
    let left = flags(shell, fields, "", rest, &mut values).ok()?;
    (left.len() > positional_count(shell, fields)).then_some(at)
}

/// The bytes of `fields`' record: every field but [`verbatim`]'s
/// own filled as [`record`] fills them — the environment, the flags,
/// read as leading tokens of `words` alone (a declaration only ever
/// puts them before the positionals, and this rule reads only that
/// far), then the earlier positionals in order. What is left of `line`
/// from there is the last field's value, whole: blanks, `|` and `>`
/// kept, since it is read from `line` itself and not rebuilt from
/// `words`, which quoting has already taken apart.
///
/// # Errors
/// As [`record`]: the token no field could take.
pub fn record_verbatim(
    shell: &Shell,
    name: &str,
    fields: &[Field],
    at: usize,
    line: &str,
    words: &[crate::Word],
) -> Result<Vec<u8>, Miss> {
    let bad = |token| missed(shell, name, token, fields);
    let filled =
        |field: &Field| given(shell, field).unwrap_or_else(|| empty(&field.ty));
    let mut values: Vec<Value> = fields.iter().map(filled).collect();
    let rest = &words[1..];
    let texts: Vec<&str> = rest.iter().map(|word| word.text.as_str()).collect();
    let left = flags(shell, fields, name, &texts, &mut values)?;
    let taken_by_flags = texts.len() - left.len();
    let earlier: Vec<usize> = fields
        .iter()
        .enumerate()
        .filter(|(i, field)| {
            *i != at && given(shell, field).is_none() && !flag(shell, field)
        })
        .map(|(i, _)| i)
        .collect();
    let mut given_count = 0;
    for &field_at in &earlier {
        let Some(token) = left.get(given_count).copied() else { break };
        let value = taken(&fields[field_at].ty, token);
        values[field_at] = value.ok_or_else(|| bad(token))?;
        given_count += 1;
    }
    let consumed = taken_by_flags + given_count;
    let tail = rest.get(consumed).map_or("", |word| &line[word.start..]);
    values[at] = Value::Utf8(tail.to_string());
    let named = fields.iter().map(|field| field.name.clone());
    let record = Value::Record(named.zip(values).collect());
    let ty = Ty::Record(fields.to_vec());
    dynamic::encode(&ty, &record).map_err(|_| bad(tail))
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
        let part = if !flag(shell, field) {
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
    let mine = |f: &&Field| given(shell, f).is_none() && flag(shell, f);
    fields.iter().filter(mine).map(dash).collect()
}

/// One flag field as a token spells it: its own one letter where one
/// dash reaches it (the first letter of any longer `bool` name), else
/// its whole name under two dashes, an option's only door.
fn dash(field: &Field) -> String {
    if lettered(field) {
        let letter: String = field.name.chars().take(1).collect();
        format!("-{letter}")
    } else {
        format!("--{}", field.name)
    }
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
        // Two dashes: `letters` still carries the one `dashed` left
        // unstripped, so a leading dash there means the token had two,
        // and what follows is read as the whole name, never letter by
        // letter, and never narrowed to the fields one dash reaches. A
        // lone `--` has nothing after the second dash, which is a name
        // no field has, so it Misses like any other.
        let two = letters.strip_prefix('-');
        let at = if let Some(rest) = two {
            named(shell, fields, rest).ok_or_else(|| missed(token))?
        } else if let Some(at) = whole(shell, fields, letters) {
            at
        } else {
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
        if given(shell, field).is_some() || flag(shell, field) {
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

/// A flag: a `bool` field, a field named by one letter, or a field
/// the host of the shell named an option of ([`Shell::option`]) — the
/// fields no positional token ever fills, which [`positionals`] and
/// [`record_verbatim`] skip and [`form`] prints under a dash. Nothing
/// here is a name of this crate's own: the one word a host adds is
/// the host's, as `cwd` is [`given`]'s.
fn flag(shell: &Shell, field: &Field) -> bool {
    field.ty == Ty::Bool || one(&field.name) || shell.optioned(&field.name)
}

/// A field a token under one dash may name whole, and the only kind
/// its letters may name: a `bool`, or a name of one letter. An option
/// is reached by its whole name under two dashes alone ([`named`]).
fn lettered(field: &Field) -> bool {
    field.ty == Ty::Bool || one(&field.name)
}

/// The letters of a token under a dash, or `None` for a positional; a
/// dash by itself is a target and names no field.
fn dashed(token: &str) -> Option<&str> {
    token.strip_prefix('-').filter(|letters| !letters.is_empty())
}

/// The field `letters` is the whole name of, under one dash: only
/// [`lettered`]'s own answer.
fn whole(shell: &Shell, fields: &[Field], letters: &str) -> Option<usize> {
    let mine = |field: &Field| {
        given(shell, field).is_none()
            && lettered(field)
            && field.name == letters
    };
    fields.iter().position(mine)
}

/// The field `letters` is the whole name of, under two dashes: any
/// field at all, not only [`lettered`]'s — a `bool` set and any other
/// type taking the next token, exactly as [`whole`] already reads
/// past its own narrower door.
fn named(shell: &Shell, fields: &[Field], letters: &str) -> Option<usize> {
    let mine =
        |field: &Field| given(shell, field).is_none() && field.name == letters;
    fields.iter().position(mine)
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
