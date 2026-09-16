#![no_std]
#![forbid(unsafe_code)]
//! The cli: a command line in, text out, through a `Client`. Generic
//! over the tree; it knows no Module. The process boundary (stdin,
//! stdout, the link, the terminal) is the app's.

extern crate alloc;

mod complete;
mod line;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write as _;

use reovim_client::{Client, Fault, Rd, log, value};

pub use complete::{Completion, complete};
pub use line::{Act, Line};

/// The commands, one per line. `quit` is the app's and never gets here.
pub const USAGE: &str = "\
lookup <target>                 rd
info <target>                   name, flags, times, apis
children <target>               rd and name per line
read <target>                   bytes to stdout
write <target> [bytes]          bytes, or stdin without them, at 0
call <target> <method> [arg]    output decoded by the declaration
log [FROM]                      entries from seq FROM, default 1
quit                            end an interactive session
<target> is a path from / or rd:<n>";

/// The command names, for completion.
const COMMANDS: [&str; 8] =
    ["lookup", "info", "children", "read", "write", "call", "log", "quit"];

/// One command already split into words; what goes to stdout, text or
/// raw bytes. `write` without bytes needs stdin, so the app fills the
/// argument before calling.
///
/// # Errors
/// `Usage` for an unknown command or the wrong number of words;
/// otherwise what the Client says.
pub fn input(client: &mut Client, words: &[String]) -> Result<Vec<u8>, Fault> {
    let word = |i: usize| words.get(i).map(String::as_str).ok_or_else(usage);
    if word(0)? == "log" {
        return log_from(client, words.get(1));
    }
    let target = client.resolve(word(1)?)?;
    match (word(0)?, words.len()) {
        ("lookup", 2) => Ok(format!("{}\n", target.0).into_bytes()),
        ("info", 2) => info(client, target),
        ("children", 2) => children(client, target),
        ("read", 2) => client.read_all(target),
        ("write", 3) => {
            let len = client.write_all(target, word(2)?.as_bytes().to_vec())?;
            Ok(format!("{len}\n").into_bytes())
        }
        ("call", 3 | 4) => call(client, target, word(2)?, words.get(3)),
        _ => Err(usage()),
    }
}

fn usage() -> Fault {
    Fault::Usage(USAGE.to_string())
}

fn info(client: &mut Client, rd: Rd) -> Result<Vec<u8>, Fault> {
    let info = client.info(rd)?;
    let flags = ["READ", "WRITE", "CONTAINER"]
        .iter()
        .zip([1, 2, 4])
        .filter(|(_, bit)| info.flags.bits() & bit != 0)
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join("|");
    let apis: Vec<String> =
        info.apis.iter().map(|h| format!("{:016x}", h.0)).collect();
    let line = format!(
        "name={} flags={flags} ctime={} mtime={} atime={} apis={}\n",
        info.name.unwrap_or_default(),
        info.ctime,
        info.mtime,
        info.atime,
        apis.join(",")
    );
    Ok(line.into_bytes())
}

fn children(client: &mut Client, rd: Rd) -> Result<Vec<u8>, Fault> {
    let mut out = String::new();
    for child in client.children(rd)? {
        let name = client.name(child)?;
        let _ = writeln!(out, "{} {name}", child.0);
    }
    Ok(out.into_bytes())
}

fn call(
    client: &mut Client,
    rd: Rd,
    name: &str,
    arg: Option<&String>,
) -> Result<Vec<u8>, Fault> {
    let (api, method) = client.method(rd, name)?;
    let input = value::encode_arg(&method.input, arg.map(String::as_str))?;
    let output = client.call_method(rd, api, &method, input)?;
    value::show(&method.output, &output)
}

fn log_from(
    client: &mut Client,
    from: Option<&String>,
) -> Result<Vec<u8>, Fault> {
    let from =
        from.map_or(Ok(1), |text| text.parse::<u64>().map_err(|_| usage()))?;
    let mut out = String::new();
    for entry in log::tail(client, from)? {
        out.push_str(&entry.line());
    }
    Ok(out.into_bytes())
}
