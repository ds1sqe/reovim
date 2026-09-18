//! `stat`, a Driver under `/bin`: its declaration and its
//! run. The slots it is served through are the crate's.
//!
//! One run answers with an output cell holding the long line of one
//! rd, its number, its three times, and every method of every api it
//! offers as the declarations spell them. It asks the Kernel and
//! nothing else: `Info` and `Parent` for the rd and `/sys/decls` for
//! each api it names.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write as _;

use reovim_kernel_interface::{Calls, Decl, Error, Field, Rd, Rights, Ty};
use reovim_sdk::{cell, ops};

use crate::Coreutils;
use crate::format::{self, Entry};

reovim_sdk::declare! {
    api stat ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { cwd: Rd, target: utf8(256) }
}

/// What a run takes, beside the api it belongs to.
pub use stat::Args;
pub(crate) use stat::{decl, dispatch};

impl stat::Api for Coreutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        let target = cell::resolve(ctx, args.cwd, &args.target)?;
        let text = text(ctx, target)?;
        self.cell(ctx, rd, text.into_bytes())
    }
}

/// The long line, the three times as dates, then every api with its
/// methods as the declaration shows them.
fn text(ctx: &dyn Calls, rd: Rd) -> Result<String, Error> {
    let info = ops::info(ctx, rd)?;
    let entry = Entry { name: format::named(rd, &info), rd, info };
    let mut out = format::long_line(ctx, &entry, false, false)?;
    let times = [
        ("rd", entry.rd.0.to_string()),
        ("ctime", format::date(entry.info.ctime)),
        ("mtime", format::date(entry.info.mtime)),
        ("atime", format::date(entry.info.atime)),
    ];
    for (key, value) in times {
        let _ = writeln!(out, "{key} {value}");
    }
    for hash in &entry.info.apis {
        let decl = ops::decl(ctx, *hash)?;
        let _ = writeln!(out, "{} v{}", decl.api, decl.ver);
        out.push_str(&methods(&decl));
    }
    Ok(out)
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
