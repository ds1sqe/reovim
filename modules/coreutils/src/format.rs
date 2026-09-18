//! The text more than one Driver under `/bin` prints the same way: a
//! listed rd and the name it wears, the long line, a date, and the
//! columns a listing is laid out down.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write as _;

use reovim_kernel_interface::{Calls, Error, Flags, Info, Rd};

use crate::ops;

/// One listed rd: its number, the name it shows under, and what the
/// Kernel caches about it.
pub struct Entry {
    /// The rd itself, which `-a` puts in front of the name.
    pub rd: Rd,
    /// What it shows under.
    pub name: String,
    /// What the Kernel caches about it.
    pub info: Info,
}

/// Every child of `rd`, named and sorted by name.
///
/// # Errors
/// As `Op::Children` and `Op::Info`.
pub fn entries(ctx: &dyn Calls, rd: Rd) -> Result<Vec<Entry>, Error> {
    let mut entries = Vec::new();
    for child in ops::children(ctx, rd)? {
        let info = ops::info(ctx, child)?;
        entries.push(Entry { rd: child, name: named(child, &info), info });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// The name an rd shows under; one the Kernel does not name is `rd:N`.
pub fn named(rd: Rd, info: &Info) -> String {
    info.name.clone().unwrap_or_else(|| format!("rd:{}", rd.0))
}

/// A name as a listing shows it: a Container bold blue, a cell that
/// offers an api green, anything else as it is. The one place a Driver
/// writes an escape, and only where the run asked for `color`.
pub fn painted(name: &str, info: &Info, color: bool) -> String {
    if !color {
        return name.to_string();
    }
    if info.flags.contains(Flags::CONTAINER) {
        format!("\x1b[1;34m{name}\x1b[0m")
    } else if info.apis.is_empty() {
        name.to_string()
    } else {
        format!("\x1b[32m{name}\x1b[0m")
    }
}

/// `TYPE RIGHTS  MTIME  APIS  NAME`, the rd number first under `rds`.
///
/// # Errors
/// As [`ops::decl`], which names each api the rd offers.
pub fn long_line(
    ctx: &dyn Calls,
    entry: &Entry,
    rds: bool,
    color: bool,
) -> Result<String, Error> {
    let flags = entry.info.flags;
    let kind = if flags.contains(Flags::CONTAINER) { 'c' } else { '-' };
    let read = if flags.contains(Flags::READ) { 'r' } else { '-' };
    let write = if flags.contains(Flags::WRITE) { 'w' } else { '-' };
    let mut apis = Vec::new();
    for hash in &entry.info.apis {
        apis.push(ops::decl(ctx, *hash)?.api);
    }
    let apis = if apis.is_empty() { "-".to_string() } else { apis.join(",") };
    let number = if rds { format!("{} ", entry.rd.0) } else { String::new() };
    let at = date(entry.info.mtime);
    let name = painted(&entry.name, &entry.info, color);
    Ok(format!("{number}{kind} {read}{write}  {at}  {apis}  {name}\n"))
}

/// `YYYY-MM-DD HH:MM:SS` in UTC of one instant of the Kernel clock, by
/// the civil-from-days arithmetic, which needs no table and no host.
pub fn date(nanos: u64) -> String {
    let seconds = nanos / 1_000_000_000;
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Counting the year from March, so the leap day is the last day.
    let shifted = days + 719_468;
    let (era, doe) = (shifted / 146_097, shifted % 146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = era * 400 + yoe + u64::from(month <= 2);
    let (hour, minute, second) = (rest / 3600, rest % 3600 / 60, rest % 60);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

/// `names` as a listing shows them: down the columns that fit in
/// `width`, one per line when the run gave no width at all.
pub fn laid_out(names: &[String], width: u64) -> String {
    match usize::try_from(width).unwrap_or(usize::MAX) {
        0 => lines(names),
        width => columns(names, width),
    }
}

/// One name per line, as a pipe wants them.
fn lines(names: &[String]) -> String {
    let mut out = String::new();
    for name in names {
        let _ = writeln!(out, "{name}");
    }
    out
}

/// `names` down columns as a listing lays them out: the most columns
/// whose widths and two-space gaps fit in `width`, read down and then
/// across. Nothing to lay out is nothing printed, as an empty listing
/// is.
fn columns(names: &[String], width: usize) -> String {
    if names.is_empty() {
        return String::new();
    }
    let rows = |columns: usize| names.len().div_ceil(columns);
    let widths = |columns: usize| -> Vec<usize> {
        let widest =
            |column: &[String]| column.iter().map(|name| cells(name)).max();
        names.chunks(rows(columns)).map(|c| widest(c).unwrap_or(0)).collect()
    };
    let fits = |columns: &usize| {
        let widths = widths(*columns);
        widths.len() == *columns
            && widths.iter().sum::<usize>() + 2 * (columns - 1) <= width
    };
    let count = (1..=names.len()).rev().find(fits).unwrap_or(1);
    let (rows, widths) = (rows(count), widths(count));
    let mut out = String::new();
    for row in 0..rows {
        let at = |column: usize| names.get(column * rows + row);
        let cell = |column: usize| Some((at(column)?, widths[column]));
        let pad = |(name, width): (&String, usize)| {
            format!("{name}{}", " ".repeat(width - cells(name) + 2))
        };
        let line: String = (0..count).filter_map(cell).map(pad).collect();
        let _ = writeln!(out, "{}", line.trim_end());
    }
    out
}

/// The cells `text` takes on a terminal row; an SGR sequence [`painted`]
/// wrote takes none, so a colored name lays out as its bare one.
fn cells(text: &str) -> usize {
    let mut cells = 0;
    let mut rest = text;
    while let Some(at) = rest.find('\x1b') {
        cells += rest[..at].chars().map(cell).sum::<usize>();
        let end = rest[at..].find('m').map_or(rest.len(), |m| at + m + 1);
        rest = &rest[end..];
    }
    cells + rest.chars().map(cell).sum::<usize>()
}

/// The cells `c` takes on the screen: two for the East Asian wide and
/// fullwidth chars a terminal draws double, one for the rest. The same
/// table a Client has, which a Driver cannot link.
const fn cell(c: char) -> usize {
    let wide = matches!(
        c as u32,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1F64F
            | 0x1F900..=0x1F9FF
            | 0x20000..=0x3FFFD
    );
    if wide { 2 } else { 1 }
}
