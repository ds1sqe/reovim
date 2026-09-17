//! `dmesg`, as util-linux writes it: one line per `/sys/log` entry,
//! stamped from the Kernel's first entry, filtered by mark and level.
//!
//! Every form here is a function of an entry, the boot time, the entry
//! printed before it and the options, so the exact strings are pinned
//! by tests and the follow in the app prints each new entry the same
//! way as the first read did.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_client::log::{self, Entry};

use crate::paint::Paint;
use crate::{Miss, civil, misuse};

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

/// Every entry `form` shows, in order. `last` is the time of the entry
/// printed before the first of these, which `-H` counts its deltas
/// from; `None` starts a fresh run.
#[must_use]
pub fn show(
    entries: &[Entry],
    boot: u64,
    last: &mut Option<u64>,
    form: &Form,
) -> String {
    let mut out = String::new();
    for entry in entries.iter().filter(|entry| shown(entry, form)) {
        out.push_str(&line(entry, boot, *last, form));
        *last = Some(entry.time);
    }
    out
}

/// One entry: the `-x` decode, the stamp, the text, and a newline.
#[must_use]
pub fn line(
    entry: &Entry,
    boot: u64,
    last: Option<u64>,
    form: &Form,
) -> String {
    let paint = &form.paint;
    let decode = if form.decode {
        let mark = &entry.mark;
        paint.dim(&format!("{mark:<6}:{:<6}: ", entry.level_name()))
    } else {
        String::new()
    };
    format!("{decode}{}{}\n", stamp(entry, boot, last, form), text(entry, form))
}

/// The stamp and the space after it, colored; empty under `-t`.
fn stamp(entry: &Entry, boot: u64, last: Option<u64>, form: &Form) -> String {
    let paint = &form.paint;
    match form.stamp {
        Stamp::None => String::new(),
        Stamp::Delta => {
            let (seconds, micros) = split(entry.time.saturating_sub(boot));
            paint.green(&format!("[{seconds:>5}.{micros:06}]")) + " "
        }
        Stamp::Ctime => paint.green(&format!("[{}]", ctime(entry.time))) + " ",
        Stamp::Human => human(entry.time, last, *paint) + " ",
    }
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

/// The text, colored by how urgent the entry is: `err` red, `crit` and
/// above bold red, `warn` bold, the rest as it is.
fn text(entry: &Entry, form: &Form) -> String {
    let (paint, text) = (&form.paint, entry.text());
    match entry.level {
        0..=2 => paint.red_bold(&text),
        3 => paint.red(&text),
        4 => paint.bold(&text),
        _ => text,
    }
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
