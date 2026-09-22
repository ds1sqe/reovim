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
//! and a number taking it parsed.
//!
//! One kind of field changes how a whole line is read: a `rest`,
//! which a declaration puts last and puts at most one of. [`rest_at`]
//! is what tells [`crate::run`] to hand the line over whole, and
//! [`record_tail`] reads the leading flags and the earlier
//! positionals off its front and gives that field everything left, as
//! it was typed.

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

/// The field that takes the rest of the line, if the record declares
/// one: a `Ty::Rest`, which a declaration puts last and puts at most
/// one of ([`reovim_kernel_interface::Decl::check`] refuses any
/// other placing). This is the whole of what tells [`crate::run`]
/// which door a name's declaration wants.
#[must_use]
pub fn rest_at(fields: &[Field]) -> Option<usize> {
    fields.iter().position(|field| matches!(field.ty, Ty::Rest(_)))
}

/// The bytes of the record `ty`, filled from `tail` — everything a
/// line held past its name, as it was typed.
///
/// The declaration is read first and the line is never cut. Leading
/// words under a dash that name a field of `ty` are its flags, a
/// `bool` set and any other type taking the word after it; the first
/// word that is not one of those ends the flags, and from there the
/// earlier positionals take a word each in declared order. What is
/// left of `tail` from the next word's own first byte is the `rest`
/// field's value, whole: blanks, quotes, `|` and `>` kept, since it is
/// read from `tail` itself and never rebuilt from words.
///
/// Flags stand before the positionals and only there, because a
/// `rest` field swallows dashes as readily as anything else: a word
/// under a dash no field is named by is where the value begins
/// (`:search -x` looks for `-x`, `:w -x` writes `-x`, as nvim has
/// both). The one exception is a word under two dashes naming an
/// option this shell's host reserved ([`Shell::option`]): that word
/// is never a positional on any line, so a record with no field for
/// it is an `undeclared` Miss, which the host whose own word it is
/// words its own way.
///
/// # Errors
/// As [`record`], and the `undeclared` Miss above.
pub fn record_tail(
    shell: &Shell,
    name: &str,
    ty: &Ty,
    tail: &str,
) -> Result<Vec<u8>, Miss> {
    let Ty::Record(fields) = ty else {
        return Err(missed(shell, name, "", &[]));
    };
    let Some(at) = rest_at(fields) else {
        return Err(missed(shell, name, "", fields));
    };
    let filled =
        |field: &Field| given(shell, field).unwrap_or_else(|| empty(&field.ty));
    let mut values: Vec<Value> = fields.iter().map(filled).collect();
    let mut cursor = crate::Cursor::new(tail);
    let mut from = leading(shell, fields, name, &mut cursor, &mut values)?;
    // The earlier positionals: one word each, in declared order, and
    // the rest begins where the word after the last of them does.
    let earlier = fields.iter().enumerate().filter(|(at_earlier, field)| {
        *at_earlier != at
            && given(shell, field).is_none()
            && !flag(shell, field)
    });
    for (field_at, field) in earlier {
        let Some(word) = cursor.next()? else { break };
        let value = taken(&field.ty, &word.text);
        values[field_at] =
            value.ok_or_else(|| missed(shell, name, &word.text, fields))?;
        from = cursor.mark();
    }
    values[at] = Value::Utf8(tail[from..].to_string());
    let held = fields.iter().map(|field| field.name.clone());
    let record = Value::Record(held.zip(values).collect());
    dynamic::encode(ty, &record)
        .map_err(|_| missed(shell, name, &tail[from..], fields))
}

/// The flags at the front of a line whose record ends in a `rest`,
/// each set in `values`; where the word that ended them begins, which
/// is the first word under no dash, or under a dash no field of this
/// record is named by.
fn leading(
    shell: &Shell,
    fields: &[Field],
    name: &str,
    cursor: &mut crate::Cursor<'_>,
    values: &mut [Value],
) -> Result<usize, Miss> {
    loop {
        let from = cursor.mark();
        let run = cursor.bare();
        let Some(letters) = spelled(run) else { return Ok(from) };
        let led = led(shell, fields, name, run, letters, values)?;
        if matches!(led, Led::Value) {
            return Ok(from);
        }
        cursor.past(run);
        let Led::Field(at) = led else { continue };
        if fields[at].ty == Ty::Bool {
            values[at] = Value::Bool(true);
            continue;
        }
        let next = cursor.next()?;
        let next = next.ok_or_else(|| missed(shell, name, run, fields))?;
        let value = taken(&fields[at].ty, &next.text);
        values[at] =
            value.ok_or_else(|| missed(shell, name, &next.text, fields))?;
    }
}

/// What a leading token under a dash turned out to be.
enum Led {
    /// The field it names, which takes the word after it unless it is
    /// a `bool`.
    Field(usize),
    /// Letters, each naming a `bool` already set here.
    Set,
    /// No field of this record: the value begins at this token.
    Value,
}

/// Which of the three `run`, a token under a dash whose `letters` are
/// past it, is for this record.
///
/// # Errors
/// The `undeclared` Miss for a word under two dashes this shell's
/// host reserved that this record declares no field for: reserved is
/// shell-wide, so it is no positional and no text on any line.
fn led(
    shell: &Shell,
    fields: &[Field],
    name: &str,
    run: &str,
    letters: &str,
    values: &mut [Value],
) -> Result<Led, Miss> {
    let Some(whole_name) = letters.strip_prefix('-') else {
        let one = whole(shell, fields, letters).map(Led::Field);
        let letters =
            || each(shell, fields, letters, values).map(|()| Led::Set);
        return Ok(one.or_else(letters).unwrap_or(Led::Value));
    };
    match named(shell, fields, whole_name) {
        Some(at) => Ok(Led::Field(at)),
        None if shell.optioned(whole_name) => {
            Err(unflagged(shell, name, run, fields))
        }
        None => Ok(Led::Value),
    }
}

/// The letters of a token under a dash, read from the line as it
/// stands: a flag is spelled with no quote char in it, so one that
/// holds one is text and not a flag, and the value it belongs to
/// begins there.
fn spelled(run: &str) -> Option<&str> {
    dashed(run).filter(|letters| !letters.contains(['\'', '"']))
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

/// Every token under a dash taken by the flag field it names, wherever
/// it stands — a record with no `rest` has no field for a dash to be
/// text in, so `tree /module -L 2` reads its flag where it was typed;
/// [`record_tail`] is the one reading where flags lead and only lead.
/// What is left over is the positionals', in the order it was typed.
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
        let undeclared = |token| unflagged(shell, name, token, fields);
        let at = if let Some(rest) = two {
            named(shell, fields, rest).ok_or_else(|| undeclared(token))?
        } else if let Some(at) = whole(shell, fields, letters) {
            at
        } else {
            let each = each(shell, fields, letters, values);
            each.ok_or_else(|| undeclared(token))?;
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
            | (Value::Utf8(_), Ty::Utf8(_) | Ty::Rest(_))
    )
}

/// A flag: a `bool` field, a field named by one letter, or a field
/// the host of the shell named an option of ([`Shell::option`]) — the
/// fields no positional token ever fills, which [`positionals`] and
/// [`record_tail`] skip and [`form`] prints under a dash. Nothing
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
        Ty::Utf8(_) | Ty::Rest(_) => Value::Utf8(token.to_string()),
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
        Ty::Utf8(_) | Ty::Rest(_) => Value::Utf8(String::new()),
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
    Miss { name, target, cause, unknown: false, undeclared: false }
}

/// A token under a dash this record declares no field for: the same
/// Miss, saying so, which a host whose own word the flag is may word
/// its own way ([`Miss::undeclared`]).
fn unflagged(shell: &Shell, name: &str, token: &str, fields: &[Field]) -> Miss {
    Miss { undeclared: true, ..missed(shell, name, token, fields) }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;
    use alloc::vec;
    use alloc::vec::Vec;

    use reovim_kernel_interface::{Field, Ty};
    use reovim_sdk::dynamic::{self, Value};

    use super::record_tail;
    use crate::{Miss, Shell};

    fn named(name: &str, ty: Ty) -> Field {
        Field { name: name.to_string(), ty }
    }

    /// Vim's own three shapes, as `plugins/vim/src/bins.rs` declares
    /// them: a pattern behind a flag, a spec behind the address the
    /// host named an option of, and a line behind a positional.
    fn sought() -> Vec<Field> {
        vec![named("backward", Ty::Bool), named("pattern", Ty::Rest(1024))]
    }

    fn subst() -> Vec<Field> {
        vec![named("address", Ty::Utf8(256)), named("spec", Ty::Rest(256))]
    }

    fn bound() -> Vec<Field> {
        vec![named("keys", Ty::Utf8(64)), named("line", Ty::Rest(512))]
    }

    /// A shell whose host named `address` an option of it, as Vim's
    /// host does.
    fn shell() -> Shell {
        let mut shell = Shell::default();
        shell.option("address");
        shell
    }

    /// The record `tail` fills, as a value.
    fn read(fields: Vec<Field>, tail: &str) -> Result<Value, Miss> {
        let ty = Ty::Record(fields);
        let bytes = record_tail(&shell(), "n", &ty, tail)?;
        Ok(dynamic::decode(&ty, &bytes).unwrap())
    }

    /// What the field named `name` holds.
    fn held(value: &Value, name: &str) -> Value {
        value.field(name).unwrap().clone()
    }

    #[test]
    fn a_rest_field_takes_the_raw_remainder() {
        let utf8 = |s: &str| Value::Utf8(s.to_string());

        // A quote char is text, not a quote: the line the shell would
        // have refused as an unclosed quote runs whole.
        let one = read(sought(), " /don't/X/").unwrap();
        assert_eq!(held(&one, "pattern"), utf8("/don't/X/"));
        let quoted = read(sought(), " \"a b\"").unwrap();
        assert_eq!(held(&quoted, "pattern"), utf8("\"a b\""));

        // `|` and `>` are the line's own here, not the shell's.
        let piped = read(sought(), " a | b > c").unwrap();
        assert_eq!(held(&piped, "pattern"), utf8("a | b > c"));

        // A dashed word no field of the record names is where the
        // rest begins, as nvim's `/-x` and `:w -x` both have it.
        let dashed = read(sought(), " -x").unwrap();
        assert_eq!(held(&dashed, "pattern"), utf8("-x"));
        assert_eq!(held(&dashed, "backward"), Value::Bool(false));
        let unknown = read(sought(), " --nosuch b").unwrap();
        assert_eq!(held(&unknown, "pattern"), utf8("--nosuch b"));

        // A flag the record does declare stands before it and is read
        // once; what follows is the rest, dashes and all.
        let flagged = read(sought(), " --backward foo -x").unwrap();
        assert_eq!(held(&flagged, "backward"), Value::Bool(true));
        assert_eq!(held(&flagged, "pattern"), utf8("foo -x"));

        // An option's value is a word, quoting and all, since it
        // stands before the rest begins.
        let ranged = read(subst(), " --address \"'<,'>\" /a/b/").unwrap();
        assert_eq!(held(&ranged, "address"), utf8("'<,'>"));
        assert_eq!(held(&ranged, "spec"), utf8("/a/b/"));

        // Nothing past the name is an empty rest, not a Miss.
        assert_eq!(held(&read(sought(), "").unwrap(), "pattern"), utf8(""));
        assert_eq!(held(&read(sought(), "   ").unwrap(), "pattern"), utf8(""));
    }

    #[test]
    fn flags_stand_before_the_positionals_only() {
        let utf8 = |s: &str| Value::Utf8(s.to_string());

        // The documented example: a flag inside the bound line is the
        // line's, the flags having ended at `<C-r>`.
        let bind = read(bound(), " <C-r> search --backward").unwrap();
        assert_eq!(held(&bind, "keys"), utf8("<C-r>"));
        assert_eq!(held(&bind, "line"), utf8("search --backward"));

        // And so is a redirect.
        let pour = read(bound(), " gw point > /self/dev/clipboard").unwrap();
        assert_eq!(held(&pour, "keys"), utf8("gw"));
        assert_eq!(held(&pour, "line"), utf8("point > /self/dev/clipboard"));

        // A positional short of the rest leaves it empty.
        let alone = read(bound(), " gw").unwrap();
        assert_eq!(held(&alone, "keys"), utf8("gw"));
        assert_eq!(held(&alone, "line"), utf8(""));
    }

    #[test]
    fn an_option_no_field_of_the_record_names_is_a_declared_miss() {
        // `set`'s own shape: the options are the rest of the line and
        // there is no address at all. The host reserved the word
        // shell-wide, so it cannot be text here, and the Miss says
        // the declaration lacks it — which is what lets Vim's host
        // word it E481 rather than print a usage line.
        let options = vec![named("options", Ty::Rest(128))];
        let Err(miss) = read(options, " --address \"'<,'>\" ic") else {
            panic!("a reserved option no field names was read as text");
        };
        assert_eq!(miss.target, "--address");
        assert!(miss.undeclared, "{miss}");
    }
}
