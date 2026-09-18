#![no_std]
#![forbid(unsafe_code)]
//! `stat`, a Driver under `/bin`.
//!
//! One run answers with an output cell holding the long line of one
//! rd, its number, its three times, and every method of every api it
//! offers as the declarations spell them. It asks the Kernel and
//! nothing else: `Info` and `Parent` for the rd and `/sys/decls`
//! for each api it names.

extern crate alloc;

use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write as _;

use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Field, Flags, Handle, Hash,
    Info, Module, Op, Out, Rd, Reply, Rights, Ty,
};
use reovim_sdk::cell;
use reovim_sdk::walk::ROOT;

reovim_sdk::declare! {
    api stat ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { cwd: Rd, target: utf8(256) }
}

/// What a run takes, beside the api it belongs to.
pub use stat::Args;

/// The rd `stat` is served at: a Container, so a run's output cell
/// goes under it, and READ, so a caller may hold the right `run` asks
/// for.
const STAT: Flags = Flags::READ.union(Flags::CONTAINER);
/// Bytes asked of a declaration per `Read`.
const CHUNK: u32 = 64 * 1024;

/// The Driver and its Module in one; pass it twice at `load`.
pub struct Stat {
    decl: Decl,
    cells: cell::Cells,
}

/// The Driver built and ready for `load`, as a Module's `boot` is.
#[must_use]
pub fn boot(_root: &str) -> Booted {
    let stat = Rc::new(Stat::new());
    Booted { module: stat.clone(), drivers: alloc::vec![stat] }
}

impl Default for Stat {
    fn default() -> Self {
        Self::new()
    }
}

impl Stat {
    /// The Driver, before it is loaded.
    #[must_use]
    pub fn new() -> Self {
        Self { decl: stat::decl(), cells: cell::Cells::default() }
    }
}

/// A `Reply` the Driver needs now: `Pending` is `Busy`, it cannot wait.
fn ready(reply: Reply<Out>) -> Result<Out, Error> {
    match reply {
        Reply::Ready(result) => result,
        Reply::Pending(_) => Err(Error::Busy),
    }
}

/// What the Kernel caches about `rd`.
fn info(ctx: &dyn Calls, rd: Rd) -> Result<Info, Error> {
    match ready(ctx.submit(Op::Info { rd }))? {
        Out::Info(info) => Ok(info),
        _ => Err(Error::Invalid),
    }
}

/// Every byte of `h` from 0, one `Read` at a time.
fn read_all(ctx: &dyn Calls, h: Handle) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    loop {
        let at = bytes.len() as u64;
        let out = ready(ctx.submit(Op::Read { h, at, len: CHUNK }))?;
        let Out::Bytes(chunk) = out else { return Err(Error::Invalid) };
        if chunk.is_empty() {
            return Ok(bytes);
        }
        bytes.extend(chunk);
    }
}

/// The declaration `hash` names, as the Kernel publishes it at
/// `/sys/decls/<hash>`.
fn decl(ctx: &dyn Calls, hash: Hash) -> Result<Decl, Error> {
    let rd = cell::resolve(ctx, ROOT, &format!("/sys/decls/{:016x}", hash.0))?;
    let h = ctx.open(rd, Rights::READ)?;
    let bytes = read_all(ctx, h);
    let _ = ctx.submit(Op::Close { h });
    Decl::parse(&bytes?)
}

/// The long line, the three times as dates, then every api with its
/// methods as the declaration shows them.
fn text(ctx: &dyn Calls, rd: Rd) -> Result<String, Error> {
    let info = info(ctx, rd)?;
    let mut out = long_line(ctx, rd, &info)?;
    let times = [
        ("rd", rd.0.to_string()),
        ("ctime", date(info.ctime)),
        ("mtime", date(info.mtime)),
        ("atime", date(info.atime)),
    ];
    for (key, value) in times {
        let _ = writeln!(out, "{key} {value}");
    }
    for hash in &info.apis {
        let decl = decl(ctx, *hash)?;
        let _ = writeln!(out, "{} v{}", decl.api, decl.ver);
        out.push_str(&methods(&decl));
    }
    Ok(out)
}

/// `TYPE RIGHTS  MTIME  APIS  NAME`; an rd the Kernel does not name
/// shows as its number.
fn long_line(ctx: &dyn Calls, rd: Rd, info: &Info) -> Result<String, Error> {
    let flags = info.flags;
    let kind = if flags.contains(Flags::CONTAINER) { 'c' } else { '-' };
    let read = if flags.contains(Flags::READ) { 'r' } else { '-' };
    let write = if flags.contains(Flags::WRITE) { 'w' } else { '-' };
    let mut apis = Vec::new();
    for hash in &info.apis {
        apis.push(decl(ctx, *hash)?.api);
    }
    let apis = if apis.is_empty() { "-".to_string() } else { apis.join(",") };
    let at = date(info.mtime);
    let name = info.name.clone().unwrap_or_else(|| format!("rd:{}", rd.0));
    Ok(format!("{kind} {read}{write}  {at}  {apis}  {name}\n"))
}

/// Every method of `decl` under its api, one per line, as the
/// declaration writes it: `0 replace({at: u64}) -> () WRITE`.
fn methods(decl: &Decl) -> String {
    let mut out = String::new();
    for method in &decl.methods {
        let right =
            if method.right.contains(Rights::WRITE) { "WRITE" } else { "READ" };
        let input = match &method.input {
            Ty::Unit => String::new(),
            ty => shown(ty),
        };
        let (number, name) = (method.number, &method.name);
        let output = shown(&method.output);
        let _ = writeln!(out, "  {number} {name}({input}) -> {output} {right}");
    }
    out
}

/// A type as the declaration's text form writes it; a Record and a
/// Choice by their fields, since the canonical bytes keep no name for
/// the type itself.
fn shown(ty: &Ty) -> String {
    match ty {
        Ty::Unit => "()".to_string(),
        Ty::Bool => "bool".to_string(),
        Ty::U8 => "u8".to_string(),
        Ty::U32 => "u32".to_string(),
        Ty::U64 => "u64".to_string(),
        Ty::I64 => "i64".to_string(),
        Ty::F64 => "f64".to_string(),
        Ty::Rd => "Rd".to_string(),
        Ty::Bytes(max) => format!("bytes({max})"),
        Ty::Utf8(max) => format!("utf8({max})"),
        Ty::Sequence(max, item) | Ty::Array(max, item) => {
            format!("[{}; {max}]", shown(item))
        }
        Ty::Record(fields) | Ty::Choice(fields) => {
            let part = |f: &Field| format!("{}: {}", f.name, shown(&f.ty));
            let parts: Vec<String> = fields.iter().map(part).collect();
            let (open, sep, close) = match ty {
                Ty::Choice(_) => ("<", " | ", ">"),
                _ => ("{", ", ", "}"),
            };
            format!("{open}{}{close}", parts.join(sep))
        }
    }
}

/// `YYYY-MM-DD HH:MM:SS` in UTC of one instant of the Kernel clock, by
/// the civil-from-days arithmetic, which needs no table and no host.
fn date(nanos: u64) -> String {
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

impl stat::Api for Stat {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        let target = cell::resolve(ctx, args.cwd, &args.target)?;
        let text = text(ctx, target)?;
        self.cells.open(ctx, DriverId(0), rd, text.into_bytes())
    }
}

impl Module for Stat {
    fn start(&self, ctx: &dyn Calls, _home: Rd) -> Result<(), Error> {
        let rd = ctx.register(DriverId(0), cell::bin(ctx)?, Some("stat"))?;
        ctx.report(rd, STAT, core::slice::from_ref(&self.decl))
    }
}

impl Driver for Stat {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        _: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        self.cells.read(Rd(h), at, len).into()
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        _: Hash,
        number: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        stat::dispatch(self, ctx, Rd(h), number, input).into()
    }

    fn close(&self, ctx: &dyn Calls, h: u64) -> Reply<()> {
        self.cells.close(ctx, Rd(h)).into()
    }
}
