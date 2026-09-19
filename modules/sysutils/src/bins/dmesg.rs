//! `dmesg`, a Driver under `/bin`: its declaration and its run. The
//! slots it is served through are the crate's.
//!
//! One run answers with an output cell holding `/sys/log` as util-linux
//! reads it: one line per entry, stamped from the Kernel's first, the
//! Kernel's own numbers said in words, filtered by mark and level. The
//! fields are dmesg's own letters, which a shell takes literally, so
//! `-T` fills `T`; every form is a function of an entry, the boot time,
//! the entry printed before it and the record.
//!
//! Under `w` or `W` the cell goes on: the run keeps its handle on the
//! log, and a read at the cell's end answers whatever the log gained
//! past the last sequence it saw, in the same form. `w` writes what the
//! log already held first, `W` only what comes after.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_kernel_interface::{Calls, Error, Handle, Op, Rd};
use reovim_sdk::cell::Follow;
use reovim_sdk::date::civil;
use reovim_sdk::log::{self, Entry};
use reovim_sdk::ops;
use reovim_sdk::sgr::{BLUE, BOLD, DIM, GREEN, RED, RED_BOLD, paint};

use crate::names::{KERNEL, Names, decoded};
use crate::{Sysutils, entries};

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
        w: bool,
        W: bool,
        f: utf8(256),
        l: utf8(256),
        color: bool
    }
}

/// What a run takes, beside the api it belongs to.
pub use dmesg::Args;
pub(crate) use dmesg::{decl, dispatch};

impl dmesg::Api for Sysutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        let (tails, held) = (args.w || args.W, !args.W);
        let form = Form::of(args)?;
        let h = entries::open(ctx)?;
        // The handle is the cell's once there is one to keep it: a run
        // that ends before that hands it back here.
        let read = entries::from(ctx, h, 0);
        let entries = read.inspect_err(|_| ops::shut(ctx, &[h]))?;
        let boot = entries.first().map_or(0, |entry| entry.time);
        let from = entries.last().map_or(0, |entry| entry.seq + 1);
        let mut names = Names::default();
        let mut last = None;
        // `W` is the new entries only, and learns nothing of the ones
        // it does not write.
        let text = if held {
            names.learn(ctx, &entries);
            show(&entries, boot, &mut last, &form, &names)
        } else {
            String::new()
        };
        if !tails {
            let _ = ctx.submit(Op::Close { h });
            return self.cell(ctx, rd, text.into_bytes());
        }
        let tailing = Tailing { h, from, boot, last, form, names };
        self.following(ctx, rd, text.into_bytes(), Box::new(tailing))
    }
}

/// `w` and `W` between reads: the handle the run kept on the log, the
/// sequence to read from, and what the forms need to write the next
/// line.
struct Tailing {
    h: Handle,
    from: u64,
    boot: u64,
    last: Option<u64>,
    form: Form,
    names: Names,
}

impl Follow for Tailing {
    fn more(&mut self, ctx: &dyn Calls) -> Result<Option<Vec<u8>>, Error> {
        let got = entries::from(ctx, self.h, self.from)?;
        let Some(newest) = got.last() else { return Ok(Some(Vec::new())) };
        self.from = newest.seq + 1;
        self.names.learn(ctx, &got);
        let text =
            show(&got, self.boot, &mut self.last, &self.form, &self.names);
        Ok(Some(text.into_bytes()))
    }

    fn end(&mut self, ctx: &dyn Calls) {
        let _ = ctx.submit(Op::Close { h: self.h });
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
///
/// `last` is the time of the entry written before the first of these,
/// which a following read carries from the read before it; `None`
/// starts fresh.
fn show(
    entries: &[Entry],
    boot: u64,
    last: &mut Option<u64>,
    form: &Form,
    names: &Names,
) -> String {
    let mut out = String::new();
    for entry in entries.iter().filter(|entry| form.shows(entry)) {
        out.push_str(&line(entry, boot, *last, form, names));
        *last = Some(entry.time);
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
        paint(color, DIM, &head)
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
            paint(color, GREEN, &format!("[{}]", since().trim_start())) + " "
        }
        Stamp::Delta => {
            let (seconds, micros) = split(entry.time.saturating_sub(boot));
            let at = format!("[{seconds:>5}.{micros:06}{}]", since());
            paint(color, GREEN, &at) + " "
        }
        Stamp::Ctime => {
            let at = format!("[{}{}]", ctime(entry.time), since());
            paint(color, GREEN, &at) + " "
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
        return paint(color, BLUE, &at);
    };
    let (seconds, micros) = split(time.saturating_sub(last));
    paint(color, GREEN, &format!("[  +{seconds}.{micros:06}]"))
}

/// The text, the Kernel's own numbers said in words, colored by how
/// urgent the entry is: `err` red, `crit` and above bold red, `warn`
/// bold, the rest as it is.
fn text(entry: &Entry, form: &Form, names: &Names) -> String {
    let text = entry.text();
    let text = if entry.mark == KERNEL { decoded(&text, names) } else { text };
    let color = form.args.color;
    match entry.level {
        0..=2 => paint(color, RED_BOLD, &text),
        3 => paint(color, RED, &text),
        4 => paint(color, BOLD, &text),
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

#[cfg(test)]
mod tests {
    use alloc::string::{String, ToString};

    use reovim_sdk::log::Entry;

    use super::{Args, Form, show};
    use crate::names::Names;

    /// 2026-09-17 12:34:56 UTC, where these entries' boot stands. A run
    /// over the rig is stamped with the Kernel's own clock, so the two
    /// forms below are read from entries made here instead.
    const BOOT: u64 = 1_789_648_496_000_000_000;

    /// A run with nothing asked for.
    fn plain() -> Args {
        Args {
            t: false,
            T: false,
            H: false,
            d: false,
            r: false,
            x: false,
            k: false,
            w: false,
            W: false,
            f: String::new(),
            l: String::new(),
            color: false,
        }
    }

    /// One entry `after` nanoseconds past the boot.
    fn entry(seq: u64, after: u64, mark: &str, level: u8, text: &str) -> Entry {
        Entry {
            seq,
            time: BOOT + after,
            mark: mark.to_string(),
            level,
            bytes: text.as_bytes().to_vec(),
        }
    }

    /// What a run of `args` writes of `entries`, stamped from the first
    /// one's time and knowing no name for any number they hold.
    fn shown(entries: &[Entry], args: Args) -> String {
        let form = Form::of(args).unwrap();
        let boot = entries.first().map_or(0, |entry| entry.time);
        let mut last = None;
        show(entries, boot, &mut last, &form, &Names::default())
    }

    #[test]
    fn the_human_form_says_the_minute_once() {
        let entries = [
            entry(1, 0, "kernel", 6, "first"),
            entry(2, 12_345_000, "kernel", 6, "within"),
            entry(3, 61_000_000_000, "kernel", 6, "later"),
        ];
        let said = shown(&entries, Args { H: true, ..plain() });
        assert_eq!(
            said,
            "[Sep17 12:34] first\n\
             [  +0.012345] within\n\
             [Sep17 12:35] later\n"
        );
    }

    #[test]
    fn a_number_no_one_can_name_stays_the_number() {
        let call = "call rd=120 api=30d1a3b10a4306ee method=2 reply took=8123";
        let entries = [
            entry(1, 0, "kernel", 7, call),
            entry(2, 1000, "kernel", 7, "op=read caller=3 rd=120 took=1200000"),
            entry(
                3,
                2000,
                "kernel",
                7,
                "op=write caller=3 rd=120 took=1500000000",
            ),
            entry(4, 3000, "kernel", 7, "op=close caller=3 rd=120 took=812"),
            entry(5, 4000, "fs", 6, "rd=120 is the Module's own word"),
        ];
        // An rd the tree no longer has is `rd:<n>`, which is what a
        // shell takes as a target for it; a hash with no declaration
        // stays the hash, and a method with none to name it stays the
        // number. The keys stay whatever the value becomes, and `took=`
        // is said in a unit.
        let said = shown(&entries, Args { t: true, ..plain() });
        let lines: [&str; 5] = [
            "call rd=rd:120 api=30d1a3b10a4306ee method=2 reply \
             took=8.123\u{b5}s",
            "op=read caller=3 rd=rd:120 took=1.2ms",
            "op=write caller=3 rd=rd:120 took=1.5s",
            "op=close caller=3 rd=rd:120 took=812ns",
            // Only the Kernel writes in numbers; another mark's words
            // stand.
            "rd=120 is the Module's own word",
        ];
        assert_eq!(said.lines().collect::<alloc::vec::Vec<_>>(), lines);

        // `r` is the entry as the Kernel wrote it, whatever else is
        // asked.
        let raw = shown(&entries, Args { r: true, T: true, ..plain() });
        assert_eq!(raw.lines().next(), Some(call));
    }
}
