//! `dmesg`, as util-linux writes it: one line per `/sys/log` entry,
//! stamped from the Kernel's first entry, filtered by mark and level.
//!
//! Every form here is a function of an entry, the boot time, the entry
//! printed before it and the options, so the exact strings are pinned
//! by tests and the follow in the app prints each new entry the same
//! way as the first read did.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_client::Decl;
use reovim_client::log::{self, Entry};

use crate::paint::Paint;
use crate::{Miss, civil, misuse};

/// The mark of the Kernel's own entries, the ones written in numbers.
const KERNEL: &str = "kernel";

/// The stamp in front of each line.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Stamp {
    /// `[   12.345678]`: seconds since the Kernel's first entry.
    #[default]
    Delta,
    /// `-t`: none.
    None,
    /// `-T`: `[Wed Sep 17 02:17:05 2026]` in UTC.
    Ctime,
    /// `-H`: the minute once, then `[  +0.012345]` within it.
    Human,
}

/// Whether the word goes on after what the log already holds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Tail {
    /// No `-w`.
    #[default]
    No,
    /// `-w`: what is there, then every new entry.
    All,
    /// `-W`: the new entries only.
    New,
}

/// What `dmesg` was asked for.
#[derive(Clone, Debug, Default)]
pub struct Form {
    pub stamp: Stamp,
    /// `-d`: the time since the line before, inside the stamp.
    pub delta: bool,
    /// `-r`: the text as the Kernel wrote it, and nothing else.
    pub raw: bool,
    /// `-x`: the mark and the level decoded in front.
    pub decode: bool,
    /// `-k` and `-f`: the marks to show, all of them when empty.
    pub marks: Vec<String>,
    /// `-l`: the levels to show, all of them when empty.
    pub levels: Vec<u8>,
    pub paint: Paint,
}

/// The flags of `dmesg`, and whether it follows. `paint` is what the
/// app allows; `-L` and `--color=always` turn it on for this word.
///
/// # Errors
/// `Miss` for a flag `dmesg` does not take, a flag whose value is
/// missing, or a level that is not one of the eight.
pub fn form(args: &[String], paint: Paint) -> Result<(Form, Tail), Miss> {
    let mut form = Form { paint, ..Form::default() };
    let mut follow = Tail::No;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if let Some(when) = arg.strip_prefix("--color") {
            form.paint = color(when, paint)?;
            continue;
        }
        let letters = crate::flags(arg).ok_or_else(|| misuse("dmesg"))?;
        for letter in letters.chars() {
            let mut value = || args.next().ok_or_else(|| misuse("dmesg"));
            match letter {
                't' => form.stamp = Stamp::None,
                'T' => form.stamp = Stamp::Ctime,
                'H' => form.stamp = Stamp::Human,
                'd' => form.delta = true,
                'r' => form.raw = true,
                'x' => form.decode = true,
                'k' => form.marks.push("kernel".to_string()),
                'w' => follow = Tail::All,
                'W' => follow = Tail::New,
                'L' => form.paint = Paint::on(),
                'f' => form.marks.extend(parts(value()?)),
                'l' => form.levels.extend(levels(value()?)?),
                _ => return Err(misuse("dmesg")),
            }
        }
    }
    Ok((form, follow))
}

/// `--color`, `--color=always`, `=never`, `=auto`; nothing else.
fn color(when: &str, paint: Paint) -> Result<Paint, Miss> {
    match when {
        "" | "=always" => Ok(Paint::on()),
        "=never" => Ok(Paint::off()),
        "=auto" => Ok(paint),
        _ => Err(misuse("dmesg")),
    }
}

fn parts(list: &str) -> Vec<String> {
    list.split(',').filter(|p| !p.is_empty()).map(ToString::to_string).collect()
}

/// The bytes of each level name; a name that is not one of the eight
/// says which they are.
fn levels(list: &str) -> Result<Vec<u8>, Miss> {
    let mut levels = Vec::new();
    for name in parts(list) {
        let Some(level) = log::level(&name) else {
            let names = log::LEVELS.join(", ");
            let cause = format!("no such level {name}; they are: {names}");
            return Err(Miss {
                word: "dmesg".to_string(),
                ..Miss::line(&cause)
            });
        };
        levels.push(level);
    }
    Ok(levels)
}

/// Where the seconds count from: the Kernel's first entry, which is
/// the oldest the ring still holds.
#[must_use]
pub fn boot(entries: &[Entry]) -> u64 {
    entries.first().map_or(0, |entry| entry.time)
}

/// Whether `form` shows this entry at all.
fn shown(entry: &Entry, form: &Form) -> bool {
    let marked = form.marks.is_empty() || form.marks.contains(&entry.mark);
    let leveled = form.levels.is_empty() || form.levels.contains(&entry.level);
    marked && leveled
}

/// Every entry `form` shows, in order.
///
/// `last` is the time of the entry printed before the first of these,
/// which `-H` and `-d` count their deltas from; `None` starts a fresh
/// run. `names` is what the numbers of the Kernel's lines stand for.
#[must_use]
pub fn show(
    entries: &[Entry],
    boot: u64,
    last: &mut Option<u64>,
    form: &Form,
    names: &Names,
) -> String {
    let mut out = String::new();
    for entry in entries.iter().filter(|entry| shown(entry, form)) {
        out.push_str(&line(entry, boot, *last, form, names));
        *last = Some(entry.time);
    }
    out
}

/// One entry: the `-x` decode, the stamp, the text, and a newline.
/// Under `-r` the text alone, as the Kernel wrote it.
#[must_use]
pub fn line(
    entry: &Entry,
    boot: u64,
    last: Option<u64>,
    form: &Form,
    names: &Names,
) -> String {
    if form.raw {
        return format!("{}\n", entry.text());
    }
    let paint = &form.paint;
    let decode = if form.decode {
        let mark = &entry.mark;
        paint.dim(&format!("{mark:<6}:{:<6}: ", entry.level_name()))
    } else {
        String::new()
    };
    let stamp = stamp(entry, boot, last, form);
    format!("{decode}{stamp}{}\n", text(entry, form, names))
}

/// The stamp and the space after it, colored; empty under `-t` unless
/// `-d` asks for the delta, which has nothing else to stand in.
fn stamp(entry: &Entry, boot: u64, last: Option<u64>, form: &Form) -> String {
    let paint = &form.paint;
    let since = || delta(entry.time, last, form);
    match form.stamp {
        Stamp::None if !form.delta => String::new(),
        Stamp::None => {
            paint.green(&format!("[{}]", since().trim_start())) + " "
        }
        Stamp::Delta => {
            let (seconds, micros) = split(entry.time.saturating_sub(boot));
            let at = format!("[{seconds:>5}.{micros:06}{}]", since());
            paint.green(&at) + " "
        }
        Stamp::Ctime => {
            let at = format!("[{}{}]", ctime(entry.time), since());
            paint.green(&at) + " "
        }
        // `-H` already says the time since the line before.
        Stamp::Human => human(entry.time, last, *paint) + " ",
    }
}

/// `-d`: ` <    0.000008>`, the time since the line before, as
/// util-linux writes it; nothing without it.
fn delta(time: u64, last: Option<u64>, form: &Form) -> String {
    if !form.delta {
        return String::new();
    }
    let (seconds, micros) = split(time.saturating_sub(last.unwrap_or(time)));
    format!(" <{seconds:>5}.{micros:06}>")
}

/// `-H`: the minute when it changes, else the delta from `last`.
fn human(time: u64, last: Option<u64>, paint: Paint) -> String {
    let minute = |at: u64| {
        let c = civil(at);
        (c.year, c.month, c.day, c.hour, c.minute)
    };
    let within = last.filter(|last| minute(*last) == minute(time));
    let Some(last) = within else {
        let c = civil(time);
        let (month, day) = (month_name(c.month), c.day);
        let at = format!("[{month}{day:>2} {:02}:{:02}]", c.hour, c.minute);
        return paint.blue(&at);
    };
    let (seconds, micros) = split(time.saturating_sub(last));
    paint.green(&format!("[  +{seconds}.{micros:06}]"))
}

/// The text, the Kernel's own numbers said in words, colored by how
/// urgent the entry is: `err` red, `crit` and above bold red, `warn`
/// bold, the rest as it is.
fn text(entry: &Entry, form: &Form, names: &Names) -> String {
    let text = entry.text();
    let text = if entry.mark == KERNEL { decoded(&text, names) } else { text };
    let paint = &form.paint;
    match entry.level {
        0..=2 => paint.red_bold(&text),
        3 => paint.red(&text),
        4 => paint.bold(&text),
        _ => text,
    }
}

/// What the numbers of the Kernel's own lines stand for.
///
/// The Kernel writes an rd, an api hash and a method number, because
/// those are what it has; the words are the tree's and the
/// declarations'. Each is looked up once per `dmesg` run, and an rd id
/// is never reused, so what is learned here stays true.
#[derive(Clone, Debug, Default)]
pub struct Names {
    /// The path of an rd, or `rd:<n>` when it has none.
    pub paths: BTreeMap<u64, String>,
    /// The declaration of an api hash; `None` when there is none.
    pub apis: BTreeMap<u64, Option<Decl>>,
}

impl Names {
    /// The rds and the api hashes these entries name that are not
    /// known yet, for the caller to look up and put in.
    #[must_use]
    pub fn unknown(&self, entries: &[Entry]) -> (Vec<u64>, Vec<u64>) {
        let (rds, apis) = numbers(entries);
        (fresh(rds, &self.paths), fresh(apis, &self.apis))
    }
}

/// The rds and the api hashes the Kernel's own entries name, in the
/// order they say them.
fn numbers(entries: &[Entry]) -> (Vec<u64>, Vec<u64>) {
    let (mut rds, mut apis) = (Vec::new(), Vec::new());
    for entry in entries.iter().filter(|entry| entry.mark == KERNEL) {
        named_in(&entry.text(), &mut rds, &mut apis);
    }
    (rds, apis)
}

/// What one line names, added to what the lines before it named.
fn named_in(text: &str, rds: &mut Vec<u64>, apis: &mut Vec<u64>) {
    let words = text.split(' ').filter_map(|word| word.split_once('='));
    for (key, value) in words {
        match key {
            "rd" => rds.extend(value.parse::<u64>().ok()),
            "api" => apis.extend(hash(value)),
            _ => {}
        }
    }
}

/// The values `known` has not learned, each of them once.
fn fresh<T>(values: Vec<u64>, known: &BTreeMap<u64, T>) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    for value in values {
        if !known.contains_key(&value) && !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

fn hash(value: &str) -> Option<u64> {
    u64::from_str_radix(value, 16).ok()
}

/// The Kernel's own line said in words: `rd=` as its path, `api=` as
/// the declaration's name, `method=` as that declaration's name for
/// the number, `took=` as a time with its unit. The keys stay, so a
/// decoded line is as greppable as the one the Kernel wrote.
fn decoded(text: &str, names: &Names) -> String {
    let mut api: Option<&Decl> = None;
    let mut out = Vec::new();
    for word in text.split(' ') {
        let Some((key, value)) = word.split_once('=') else {
            out.push(word.to_string());
            continue;
        };
        let said = match key {
            "rd" => value.parse().ok().map(|rd| path(names, rd)),
            "api" => {
                api = hash(value)
                    .and_then(|hash| names.apis.get(&hash))
                    .and_then(Option::as_ref);
                api.map(|decl| decl.api.clone())
            }
            "method" => method(api, value.parse().ok()),
            "took" => value.parse().ok().map(took),
            _ => None,
        };
        let word = said
            .map_or_else(|| word.to_string(), |said| format!("{key}={said}"));
        out.push(word);
    }
    out.join(" ")
}

/// The path of an rd, or `rd:<n>` for one that is gone or unnamed.
fn path(names: &Names, rd: u64) -> String {
    names.paths.get(&rd).cloned().unwrap_or_else(|| format!("rd:{rd}"))
}

/// The declaration's own name for a method number.
fn method(api: Option<&Decl>, number: Option<u16>) -> Option<String> {
    Some(api?.method(number?)?.name.clone())
}

/// A span of nanoseconds in the unit that shows it: `812ns`,
/// `8.123µs`, `1.2ms`, `1.5s`; three decimals, the trailing zeros of
/// them dropped.
fn took(nanos: u64) -> String {
    const UNITS: [(u64, &str); 3] =
        [(1_000_000_000, "s"), (1_000_000, "ms"), (1_000, "µs")];
    let Some((scale, unit)) =
        UNITS.into_iter().find(|(scale, _)| nanos >= *scale)
    else {
        return format!("{nanos}ns");
    };
    let rest = nanos % scale * 1000 / scale;
    let count = format!("{}.{rest:03}", nanos / scale);
    let count = count.trim_end_matches('0').trim_end_matches('.');
    format!("{count}{unit}")
}

/// Whole seconds and microseconds of a span of nanoseconds.
const fn split(nanos: u64) -> (u64, u64) {
    (nanos / 1_000_000_000, nanos % 1_000_000_000 / 1000)
}

/// `Wed Sep 17 02:17:05 2026`, as `ctime` writes it, in UTC.
fn ctime(time: u64) -> String {
    let c = civil(time);
    let (day, month) = (day_name(c.weekday), month_name(c.month));
    let (hour, minute, second) = (c.hour, c.minute, c.second);
    format!(
        "{day} {month} {:>2} {hour:02}:{minute:02}:{second:02} {:04}",
        c.day, c.year
    )
}

fn month_name(month: u64) -> &'static str {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct",
        "Nov", "Dec",
    ];
    let at = usize::try_from(month).unwrap_or(1).clamp(1, 12);
    MONTHS[at - 1]
}

fn day_name(weekday: u64) -> &'static str {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    DAYS[usize::try_from(weekday % 7).unwrap_or(0)]
}
