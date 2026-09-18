//! A name under PATH and the run of it.
//!
//! The name resolves by lookup under PATH, by its exact name and never
//! by a table: what is there is what runs. The declaration its rd
//! reports says what the line fills; the run answers with the rd of one
//! output cell, which the shell opens, reads at an offset to the empty
//! read that ends it, and closes, which takes the cell out of the tree.
//! A `Busy` there is a Driver that follows, at the end of what it has
//! now: the run comes back open at its offset for whoever holds it to
//! read again.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_kernel_interface::{
    Calls, Error, Field, Handle, Op, Rd, Rights, Ty,
};
use reovim_sdk::dynamic::{self, Value};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};

use crate::{Miss, Shell, Step, decode, miss, resolved};

/// Where a name is looked up. One Container in this milestone, and a
/// name is taken from it by its exact spelling.
const PATH: &str = "/bin";

/// Bytes asked per read of an output cell.
const CHUNK: u32 = 64 * 1024;

/// One typed line: what it printed, and the run it left open.
pub struct Ran {
    /// What to print; empty for a line that redirects.
    pub out: Vec<u8>,
    /// The run still open, for a Driver that follows.
    pub open: Option<Open>,
}

/// A run still open: a Driver that follows answers `Busy` at the end of
/// what it has now, so its reader comes back to the cell instead of
/// ending it.
#[derive(Debug)]
pub struct Open {
    name: String,
    h: Handle,
    at: u64,
}

/// Where a redirect puts a run's output: the target as typed, and `>>`,
/// which starts at what the target already holds.
pub struct Sink<'a> {
    target: &'a str,
    append: bool,
}

impl Open {
    /// What the cell holds past the last read, and whether the empty
    /// read that ends a cell came with it, which closes the cell.
    /// Nothing and not ended is a follower with nothing new yet.
    ///
    /// # Errors
    /// Whatever the read says.
    pub fn more(&mut self, ctx: &dyn Calls) -> Result<(Vec<u8>, bool), Miss> {
        let (bytes, ended) = reading(ctx, &self.name, self.h, &mut self.at)?;
        if ended {
            self.close(ctx)?;
        }
        Ok((bytes, ended))
    }

    /// The cell closed, which takes it out of the tree and ends what the
    /// run was following.
    ///
    /// # Errors
    /// Whatever the close says.
    pub fn close(&mut self, ctx: &dyn Calls) -> Result<(), Miss> {
        let refused = |e: Error| miss(&self.name, "", Step::Op, e, "");
        closed(ctx, self.h).map_err(refused)
    }
}

/// The tokens before a redirect and where it puts the output; `>` and
/// `>>` are the shell's own and take a target after them.
pub fn split<'a>(
    typed: &'a [&'a str],
) -> Result<(&'a [&'a str], Option<Sink<'a>>), Miss> {
    let Some(at) = typed.iter().position(|t| matches!(*t, ">" | ">>")) else {
        return Ok((typed, None));
    };
    let (tokens, rest) = typed.split_at(at);
    let [arrow, target] = rest else {
        return Err(Miss::line("a redirect takes one target"));
    };
    let sink = Sink { target, append: *arrow == ">>" };
    Ok((tokens, Some(sink)))
}

/// The names under PATH, sorted, whatever order the tree lists them in.
pub fn names(ctx: &dyn Calls) -> Vec<String> {
    let Ok(at) = cell::resolve(ctx, ROOT, PATH) else { return Vec::new() };
    let Ok(children) = ops::children(ctx, at) else { return Vec::new() };
    let under = |rd| ops::info(ctx, rd).ok()?.name;
    let mut names: Vec<String> =
        children.into_iter().filter_map(under).collect();
    names.sort();
    names
}

/// The fields the run of `name` takes; `None` for a name PATH does not
/// hold or a declaration that cannot be read, which Tab offers nothing
/// for and never faults over.
pub fn fields(ctx: &dyn Calls, name: &str) -> Option<Vec<Field>> {
    let rd = lookup(ctx, name)?;
    let (_, method) = ops::method(ctx, rd, "run").ok()?;
    match method.input {
        Ty::Record(fields) => Some(fields),
        _ => None,
    }
}

/// The line `help` prints for `name`.
///
/// # Errors
/// `Miss` for a name PATH does not hold.
pub fn form(ctx: &dyn Calls, name: &str) -> Result<String, Miss> {
    let Some(fields) = fields(ctx, name) else {
        return Err(not_under(name));
    };
    Ok(decode::form(name, &fields))
}

/// The run of `name`: the record filled from the tokens, the call, and
/// the output cell read to its end.
///
/// # Errors
/// `Miss` for a name PATH does not hold, a token no field takes, a
/// redirect whose target is not there, or whatever the run says.
pub fn under_path(
    ctx: &dyn Calls,
    shell: &Shell,
    name: &str,
    args: &[&str],
    sink: Option<Sink<'_>>,
) -> Result<Ran, Miss> {
    let Some(rd) = lookup(ctx, name) else {
        // The line named this one, so the Miss is the one a host tells
        // from the rest: Vim's `:` line says E492 of it.
        return Err(Miss { unknown: true, ..not_under(name) });
    };
    let typed = args.join(" ");
    let refused = |e: Error| miss(name, &typed, Step::Op, e, "");
    let (api, method) = ops::method(ctx, rd, "run").map_err(refused)?;
    let input = decode::record(shell, name, &method.input, args)?;
    // The target resolves first, so a bad one runs nothing.
    let into = match sink {
        Some(sink) => Some(poured(ctx, shell, name, &sink)?),
        None => None,
    };
    let out = ops::call(ctx, rd, api, &method, input).map_err(refused)?;
    let answered = dynamic::decode(&method.output, &out).map_err(refused)?;
    let Value::Rd(cell) = answered else {
        return Err(refused(Error::Invalid));
    };
    let h = ctx.open(cell, Rights::READ).map_err(refused)?;
    let mut open = Open { name: name.to_string(), h, at: 0 };
    let read = reading(ctx, name, h, &mut open.at);
    let (bytes, ended) = match read {
        Ok(got) => got,
        Err(stopped) => {
            let _ = open.close(ctx);
            return Err(stopped);
        }
    };
    let out = match into {
        Some((rd, at)) => {
            if let Err(error) = written(ctx, rd, at, bytes) {
                let _ = open.close(ctx);
                return Err(refused(error));
            }
            Vec::new()
        }
        None => bytes,
    };
    if !ended {
        return Ok(Ran { out, open: Some(open) });
    }
    open.close(ctx)?;
    Ok(Ran { out, open: None })
}

/// The rd of a name PATH does not hold: none.
fn lookup(ctx: &dyn Calls, name: &str) -> Option<Rd> {
    let at = cell::resolve(ctx, ROOT, PATH).ok()?;
    ops::lookup(ctx, at, name).ok()
}

/// A name no directory of PATH holds, as `help NAME` and a line that
/// names one are both refused.
fn not_under(name: &str) -> Miss {
    let cause = format!("not under {PATH}; help lists them");
    let (name, target) = (name.to_string(), String::new());
    Miss { name, target, cause, unknown: false }
}

/// One reading of the cell from `at`: the bytes, and whether the read
/// that stopped it was the empty one that ends a cell. A `Busy` is a
/// follower at the end of what it has now, and the cell goes on.
fn reading(
    ctx: &dyn Calls,
    name: &str,
    h: Handle,
    at: &mut u64,
) -> Result<(Vec<u8>, bool), Miss> {
    let mut out = Vec::new();
    loop {
        match ops::read(ctx, h, *at, CHUNK) {
            Ok(chunk) if chunk.is_empty() => return Ok((out, true)),
            Ok(chunk) => {
                *at += chunk.len() as u64;
                out.extend(chunk);
            }
            Err(Error::Busy) => return Ok((out, false)),
            Err(error) => {
                return Err(miss(name, "", Step::Op, error, ""));
            }
        }
    }
}

/// Where a redirect writes: the target, which must be there already,
/// and the offset, 0 for `>` and what the target holds for `>>`, which
/// a whole read counts since `Info` carries no size.
fn poured(
    ctx: &dyn Calls,
    shell: &Shell,
    name: &str,
    sink: &Sink<'_>,
) -> Result<(Rd, u64), Miss> {
    let rd = resolved(ctx, shell, name, sink.target)?;
    if !sink.append {
        return Ok((rd, 0));
    }
    let refused = |e: Error| miss(name, sink.target, Step::Op, e, "");
    let held = ops::bytes(ctx, rd).map_err(refused)?;
    Ok((rd, held.len() as u64))
}

/// `bytes` into `rd` at `at`, through a handle opened and closed here.
fn written(
    ctx: &dyn Calls,
    rd: Rd,
    at: u64,
    bytes: Vec<u8>,
) -> Result<(), Error> {
    let h = ctx.open(rd, Rights::WRITE)?;
    let written = ops::write(ctx, h, at, bytes);
    closed(ctx, h)?;
    written.map(|_| ())
}

/// One handle closed.
fn closed(ctx: &dyn Calls, h: Handle) -> Result<(), Error> {
    ops::ready(ctx.submit(Op::Close { h })).map(|_| ())
}
