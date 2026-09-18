//! `dmesg`, a Driver under `/bin`: its declaration and its run. The
//! slots it is served through are the crate's.
//!
//! One run answers with an output cell holding `/sys/log` as util-linux
//! reads it: one line per entry, stamped from the Kernel's first, the
//! Kernel's own numbers said in words, filtered by mark and level. The
//! fields are dmesg's own letters, which a shell takes literally, so
//! `-T` fills `T`; every form is a function of an entry, the boot time,
//! the entry printed before it and the record, and nothing of the run
//! is remembered past it.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_kernel_interface::{Calls, Error, Rd};
use reovim_sdk::date::civil;
use reovim_sdk::log::{self, Entry};

use crate::Sysutils;
use crate::entries;
use crate::names::{KERNEL, Names, decoded};

reovim_sdk::declare! {
    api dmesg ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args {
        t: bool,
        T: bool,
        H: bool,
        d: bool,
        r: bool,
        x: bool,
        k: bool,
        f: utf8(256),
        l: utf8(256),
        color: bool
    }
}

/// What a run takes, beside the api it belongs to.
pub use dmesg::Args;
pub(crate) use dmesg::{decl, dispatch};

/// The stamp's color.
const GREEN: &str = "32";
/// The `-H` minute break.
const BLUE: &str = "1;34";
/// The decode `x` puts in front of an entry.
const DIM: &str = "2";
/// An `err` entry.
const RED: &str = "31";
/// `crit` and above.
const RED_BOLD: &str = "1;31";
/// A `warn` entry.
const BOLD: &str = "1";

impl dmesg::Api for Sysutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        let form = Form::of(args)?;
        let entries = entries::from(ctx, 0)?;
        let names = Names::of(ctx, &entries);
        let boot = entries.first().map_or(0, |entry| entry.time);
        let text = show(&entries, boot, &form, &names);
        self.cell(ctx, rd, text.into_bytes())
    }
}

/// The stamp in front of each line.
enum Stamp {
    /// `[   12.345678]`: seconds since the Kernel's first entry.
    Delta,
    /// `t`: none.
    None,
    /// `T`: `[Wed Sep 17 02:17:05 2026]` in UTC.
    Ctime,
    /// `H`: the minute once, then `[  +0.012345]` within it.
    Human,
}

/// What a run asked for: its record, and the two comma lists read out
/// of it.
struct Form {
    args: Args,
    marks: Vec<String>,
    levels: Vec<u8>,
}

impl Form {
    /// The record as the run was given it. A level that is not one of
    /// the eight is `Invalid`; naming them is the shell's.
    fn of(args: Args) -> Result<Self, Error> {
        let mut marks = parts(&args.f);
        if args.k {
            marks.push(KERNEL.to_string());
        }
        let mut levels = Vec::new();
        for name in parts(&args.l) {
            levels.push(log::level(&name).ok_or(Error::Invalid)?);
        }
        Ok(Self { args, marks, levels })
    }

    /// Which stamp the run asked for; `t` is the one that takes a stamp
    /// away, so it wins over the two that shape one.
    const fn stamp(&self) -> Stamp {
        if self.args.t {
            Stamp::None
        } else if self.args.T {
            Stamp::Ctime
        } else if self.args.H {
            Stamp::Human
        } else {
            Stamp::Delta
        }
    }

    /// Whether this entry is shown at all.
    fn shows(&self, entry: &Entry) -> bool {
        let marked = self.marks.is_empty() || self.marks.contains(&entry.mark);
        let leveled =
            self.levels.is_empty() || self.levels.contains(&entry.level);
        marked && leveled
    }
}

/// A comma list as a field spells it; an empty part is no part.
fn parts(list: &str) -> Vec<String> {
    list.split(',').filter(|p| !p.is_empty()).map(ToString::to_string).collect()
}

/// Every entry `form` shows, in order, each counting its deltas from
/// the one printed before it.
fn show(entries: &[Entry], boot: u64, form: &Form, names: &Names) -> String {
    let mut out = String::new();
    let mut last = None;
    for entry in entries.iter().filter(|entry| form.shows(entry)) {
        out.push_str(&line(entry, boot, last, form, names));
        last = Some(entry.time);
    }
    out
}

/// One entry: the `x` decode, the stamp, the text, and a newline. Under
/// `r` the text alone, as the Kernel wrote it.
fn line(
    entry: &Entry,
    boot: u64,
    last: Option<u64>,
    form: &Form,
    names: &Names,
) -> String {
    if form.args.r {
        return format!("{}\n", entry.text());
    }
    let color = form.args.color;
    let decode = if form.args.x {
        let mark = &entry.mark;
        let head = format!("{mark:<6}:{:<6}: ", entry.level_name());
        sgr(color, DIM, &head)
    } else {
        String::new()
    };
    let stamp = stamp(entry, boot, last, form);
    format!("{decode}{stamp}{}\n", text(entry, form, names))
}

/// The stamp and the space after it, colored; empty under `t` unless
/// `d` asks for the delta, which has nothing else to stand in.
fn stamp(entry: &Entry, boot: u64, last: Option<u64>, form: &Form) -> String {
    let color = form.args.color;
    let since = || delta(entry.time, last, form);
    match form.stamp() {
        Stamp::None if !form.args.d => String::new(),
        Stamp::None => {
            sgr(color, GREEN, &format!("[{}]", since().trim_start())) + " "
        }
        Stamp::Delta => {
            let (seconds, micros) = split(entry.time.saturating_sub(boot));
            let at = format!("[{seconds:>5}.{micros:06}{}]", since());
            sgr(color, GREEN, &at) + " "
        }
        Stamp::Ctime => {
            let at = format!("[{}{}]", ctime(entry.time), since());
            sgr(color, GREEN, &at) + " "
        }
        // `H` already says the time since the line before.
        Stamp::Human => human(entry.time, last, color) + " ",
    }
}

/// `d`: ` <    0.000008>`, the time since the line before, as
/// util-linux writes it; nothing without it.
fn delta(time: u64, last: Option<u64>, form: &Form) -> String {
    if !form.args.d {
        return String::new();
    }
    let (seconds, micros) = split(time.saturating_sub(last.unwrap_or(time)));
    format!(" <{seconds:>5}.{micros:06}>")
}

/// `H`: the minute when it changes, else the delta from `last`.
fn human(time: u64, last: Option<u64>, color: bool) -> String {
    let minute = |at: u64| {
        let c = civil(at);
        (c.year, c.month, c.day, c.hour, c.minute)
    };
    let within = last.filter(|last| minute(*last) == minute(time));
    let Some(last) = within else {
        let c = civil(time);
        let (month, day) = (month_name(c.month), c.day);
        let at = format!("[{month}{day:>2} {:02}:{:02}]", c.hour, c.minute);
        return sgr(color, BLUE, &at);
    };
    let (seconds, micros) = split(time.saturating_sub(last));
    sgr(color, GREEN, &format!("[  +{seconds}.{micros:06}]"))
}

/// The text, the Kernel's own numbers said in words, colored by how
/// urgent the entry is: `err` red, `crit` and above bold red, `warn`
/// bold, the rest as it is.
fn text(entry: &Entry, form: &Form, names: &Names) -> String {
    let text = entry.text();
    let text = if entry.mark == KERNEL { decoded(&text, names) } else { text };
    let color = form.args.color;
    match entry.level {
        0..=2 => sgr(color, RED_BOLD, &text),
        3 => sgr(color, RED, &text),
        4 => sgr(color, BOLD, &text),
        _ => text,
    }
}

/// `text` in one SGR sequence when the run asked for color, reset after
/// it, so a line never leaks a color into the next.
fn sgr(color: bool, code: &str, text: &str) -> String {
    if color { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
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
