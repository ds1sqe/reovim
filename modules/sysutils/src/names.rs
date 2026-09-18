//! What the numbers of the Kernel's own lines stand for.
//!
//! The Kernel writes an rd, an api hash and a method number, because
//! those are what it has; the words are the tree's and the
//! declarations'. Each is looked up once per run, and an rd id is never
//! reused, so what is learned here stays true.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_kernel_interface::{Calls, Decl, Hash, Rd};
use reovim_sdk::log::Entry;
use reovim_sdk::ops;

/// The mark of the Kernel's own entries, the ones written in numbers.
pub const KERNEL: &str = "kernel";

/// The paths and the declarations one run's entries name.
pub struct Names {
    /// The path of an rd, or `rd:<n>` when it has none.
    paths: BTreeMap<u64, String>,
    /// The declaration of an api hash; `None` when there is none.
    apis: BTreeMap<u64, Option<Decl>>,
}

impl Names {
    /// Every number `entries` name, looked up once each. An rd the
    /// tree no longer has, and a hash it does not publish, keep the
    /// number the Kernel wrote.
    pub fn of(ctx: &dyn Calls, entries: &[Entry]) -> Self {
        let (rds, apis) = numbers(entries);
        let mut names = Self { paths: BTreeMap::new(), apis: BTreeMap::new() };
        for rd in rds {
            let gone = |_| format!("rd:{rd}");
            let path = ops::where_is(ctx, Rd(rd)).unwrap_or_else(gone);
            names.paths.insert(rd, path);
        }
        for hash in apis {
            names.apis.insert(hash, ops::decl(ctx, Hash(hash)).ok());
        }
        names
    }
}

/// The Kernel's own line said in words: `rd=` as its path, `api=` as
/// the declaration's name, `method=` as that declaration's name for
/// the number, `took=` as a time with its unit. The keys stay, so a
/// decoded line is as greppable as the one the Kernel wrote.
pub fn decoded(text: &str, names: &Names) -> String {
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

/// The rds and the api hashes the Kernel's own entries name, each of
/// them once, in the order they say them.
fn numbers(entries: &[Entry]) -> (Vec<u64>, Vec<u64>) {
    let (mut rds, mut apis) = (Vec::new(), Vec::new());
    for entry in entries.iter().filter(|entry| entry.mark == KERNEL) {
        named_in(&entry.text(), &mut rds, &mut apis);
    }
    (once(rds), once(apis))
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

/// `values` without the repeats, in the order they first came.
fn once(values: Vec<u64>) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    for value in values {
        if !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

fn hash(value: &str) -> Option<u64> {
    u64::from_str_radix(value, 16).ok()
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
