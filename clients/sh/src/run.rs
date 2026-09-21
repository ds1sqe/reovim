//! A name under PATH and the run of it.
//!
//! The name resolves by lookup under each directory of the Shell's PATH
//! in turn, by its exact name and never by a table: the first directory
//! that holds it wins and what is there is what runs. Nothing about a
//! name says which side of the door it is on. The declaration its rd
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
use reovim_sdk::{cell, ops, sgr};

use crate::{Miss, Shell, Step, decode, miss, resolved};

/// Bytes asked per read of an output cell.
const CHUNK: u32 = 64 * 1024;

/// A stage, or a whole line, that names nothing to run.
pub const NO_NAME: &str = "no name; help lists them";

/// One stage of a line: the name it runs, and the tokens after it.
pub type Stage<'a> = (&'a str, &'a [&'a str]);

/// Where a redirect writes: the target, and the offset the bytes still
/// to come go at.
type Pour = (Rd, u64);

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
    /// The tokens after the name, as a Miss of this run says them.
    typed: String,
    h: Handle,
    at: u64,
    /// Where a redirect puts the bytes, and what it has taken: a run
    /// that follows goes on writing where its first batch went.
    into: Option<Pour>,
    /// Whether the shell asked for color on this stage: off, every read
    /// with nowhere to pour strips a bin's own escapes before the
    /// holder sees them, no bin having to ask what it is drawing for.
    /// A redirect takes the bytes as the bin wrote them either way —
    /// capture should be byte, and a file is not a reader to render
    /// for.
    color: bool,
}

/// Where a redirect puts a run's output: the target as typed, and `>>`,
/// which starts at what the target already holds.
pub struct Sink<'a> {
    target: &'a str,
    append: bool,
}

impl Open {
    /// What the cell holds past the last read, and whether the empty
    /// read that ends a cell came with it. Nothing and not ended is a
    /// follower with nothing new yet; a redirect takes the bytes here
    /// as it took the first, and the holder is handed none.
    ///
    /// The cell closes with the run: at the empty read, and at a
    /// failure, which leaves nobody to come back to it.
    ///
    /// # Errors
    /// Whatever the read, the write or the close says.
    pub fn more(&mut self, ctx: &dyn Calls) -> Result<(Vec<u8>, bool), Miss> {
        match self.read(ctx) {
            Ok((bytes, false)) => Ok((bytes, false)),
            Ok((bytes, true)) => self.close(ctx).map(|()| (bytes, true)),
            Err(stopped) => {
                let _ = self.close(ctx);
                Err(stopped)
            }
        }
    }

    /// One reading, into the redirect's target where the run has one —
    /// whole, the bin's own escapes and all, since a file is not a
    /// reader for `color` to be asked of — or to the holder, stripped
    /// of a bin's own escapes where the shell asked for no color.
    fn read(&mut self, ctx: &dyn Calls) -> Result<(Vec<u8>, bool), Miss> {
        let (bytes, ended) = reading(ctx, &self.name, self.h, &mut self.at)?;
        let Some((rd, at)) = self.into else {
            return Ok((self.plain(bytes), ended));
        };
        let refused = |e: Error| miss(&self.name, &self.typed, Step::Op, e, "");
        let wrote = bytes.len() as u64;
        written(ctx, rd, at, bytes).map_err(refused)?;
        self.into = Some((rd, at + wrote));
        Ok((Vec::new(), ended))
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

    /// `bytes` bound for the holder, a bin's own escapes taken out when
    /// this stage asked for none: the one fact is the shell's `color`,
    /// so a bin that declares it is told it and every other bin's
    /// bytes are stripped here regardless of what it wrote. `cat` of
    /// anything is a bin too, so this never touches a byte that is not
    /// whole of an escape sequence — `sgr::plain` works on the bytes
    /// themselves and asks nothing of them being text. Bytes bound for
    /// a redirect never reach here: `read` writes those whole.
    fn plain(&self, bytes: Vec<u8>) -> Vec<u8> {
        if self.color { bytes } else { sgr::plain(&bytes) }
    }
}

/// The line cut at bare `|` tokens, and where the last one's output
/// goes. A bare `>` or `>>` belongs to the last stage; elsewhere it is
/// refused, a stage's output being the next stage's input.
///
/// # Errors
/// `Miss` for a stage that names nothing, a redirect off the last
/// stage, and one that takes other than one target.
pub fn stages<'a>(
    typed: &'a [&'a str],
    bare: &[bool],
) -> Result<(Vec<Stage<'a>>, Option<Sink<'a>>), Miss> {
    let mut index = 0;
    let cut: Vec<&[&str]> = typed
        .split(|token| {
            let pipe = bare[index] && *token == "|";
            index += 1;
            pipe
        })
        .collect();
    let end = cut.len() - 1;
    let (mut stages, mut sink) = (Vec::new(), None);
    let mut offset = 0;
    let off = "a redirect belongs to the last stage";
    for (at, stage) in cut.into_iter().enumerate() {
        let poured = stage
            .iter()
            .enumerate()
            .position(|(i, token)| {
                bare[offset + i] && matches!(*token, ">" | ">>")
            })
            .unwrap_or(stage.len());
        offset += stage.len() + 1;
        let (tokens, rest) = stage.split_at(poured);
        sink = match rest {
            [] => None,
            [arrow, target] if at == end => {
                Some(Sink { target, append: *arrow == ">>" })
            }
            [_, _] => return Err(Miss::line(off)),
            _ => return Err(Miss::line("a redirect takes one target")),
        };
        let Some((name, args)) = tokens.split_first() else {
            return Err(Miss::line(NO_NAME));
        };
        stages.push((*name, args));
    }
    Ok((stages, sink))
}

/// The names `shell`'s PATH holds, in the order a name resolves:
/// PATH's order, and inside a directory the order the tree lists it,
/// the first spelling of a name winning so each appears once. A
/// listing that reads better sorted sorts this itself.
pub fn names(ctx: &dyn Calls, shell: &Shell) -> Vec<String> {
    let held = |at| ops::children(ctx, at).unwrap_or_default();
    let under = |rd| ops::info(ctx, rd).ok()?.name;
    let mut names: Vec<String> = Vec::new();
    for name in dirs(ctx, shell).flat_map(held).filter_map(under) {
        // The first spelling of a name wins, so each appears once.
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// The directories of `shell`'s PATH that resolve, in PATH's order. It
/// is lazy, so a name the first directory holds costs no walk to the
/// rest.
fn dirs<'a>(
    ctx: &'a dyn Calls,
    shell: &'a Shell,
) -> impl Iterator<Item = Rd> + 'a {
    shell.dirs.iter().filter_map(|path| cell::resolve(ctx, ROOT, path).ok())
}

/// The fields the run of `name` takes; `None` for a name `shell`'s PATH
/// does not hold, which is the one thing the shell says its own
/// sentence for. A name PATH holds whose declaration cannot be read, or
/// whose run takes no record, takes no field: it is there, and it
/// fills from nothing.
pub fn fields(
    ctx: &dyn Calls,
    shell: &Shell,
    name: &str,
) -> Option<Vec<Field>> {
    let rd = lookup(ctx, shell, name)?;
    let read = ops::method(ctx, rd, "run").ok();
    match read.map(|(_, method)| method.input) {
        Some(Ty::Record(fields)) => Some(fields),
        _ => Some(Vec::new()),
    }
}

/// Whether the run of `name` can be filled from the stage before it,
/// which is one declared `input: Rd` field. A name `shell`'s PATH does
/// not hold declares nothing to read and answers true, the run of it
/// being what says why the line cannot stand.
pub fn joins(ctx: &dyn Calls, shell: &Shell, name: &str) -> bool {
    let Some(fields) = fields(ctx, shell, name) else {
        return true;
    };
    let takes = |f: &Field| f.name == "input" && f.ty == Ty::Rd;
    fields.iter().any(takes)
}

/// The line `help` prints for `name`.
///
/// # Errors
/// `Miss` for a name PATH does not hold.
pub fn form(
    ctx: &dyn Calls,
    shell: &Shell,
    name: &str,
) -> Result<String, Miss> {
    let Some(fields) = fields(ctx, shell, name) else {
        return Err(not_under(shell, name));
    };
    Ok(decode::form(shell, name, &fields))
}

/// The run of `name`: the record filled from the tokens, the call, and
/// the rd of the one output cell it answered with, beside where a
/// redirect puts the bytes. The cell is not opened here, a stage that
/// is not the last ending at this and the stage after it being the one
/// reader.
///
/// # Errors
/// `Miss` for a name PATH does not hold, a token no field takes, a
/// redirect whose target is not there, or whatever the run says.
pub fn called(
    ctx: &dyn Calls,
    shell: &Shell,
    name: &str,
    args: &[&str],
    sink: Option<Sink<'_>>,
) -> Result<(Rd, Option<Pour>), Miss> {
    let Some(rd) = lookup(ctx, shell, name) else {
        // The line named this one, so the Miss is the one a host tells
        // from the rest: Vim's `:` line says E492 of it.
        return Err(Miss { unknown: true, ..not_under(shell, name) });
    };
    let typed = args.join(" ");
    let refused = |e: Error| miss(name, &typed, Step::Op, e, "");
    let (api, method) = ops::method(ctx, rd, "run").map_err(refused)?;
    let record = decode::record(shell, name, &method.input, args)?;
    // The target resolves first, so a bad one runs nothing.
    let into = match sink {
        Some(sink) => Some(poured(ctx, shell, name, &sink)?),
        None => None,
    };
    let out = ops::call(ctx, rd, api, &method, record).map_err(refused)?;
    let answered = dynamic::decode(&method.output, &out).map_err(refused)?;
    let Value::Rd(cell) = answered else {
        return Err(refused(Error::Invalid));
    };
    Ok((cell, into))
}

/// The cell a stage was handed and did not take: opened and closed,
/// which ends it. A stage that read its input closed it there and the
/// open fails, the rd being gone; an rd is never spelled twice, so this
/// can reach nothing else.
pub fn dropped(ctx: &dyn Calls, rd: Rd) {
    if rd == ROOT {
        return;
    }
    if let Ok(h) = ctx.open(rd, Rights::READ) {
        let _ = closed(ctx, h);
    }
}

/// The last stage of a line: [`called`], and its output cell read to
/// the empty read that ends it, into the sink where there is one.
///
/// # Errors
/// As [`called`], and whatever the reads and the write say.
pub fn under_path(
    ctx: &dyn Calls,
    shell: &Shell,
    name: &str,
    args: &[&str],
    sink: Option<Sink<'_>>,
) -> Result<Ran, Miss> {
    let color = colored(shell);
    let (cell, into) = called(ctx, shell, name, args, sink)?;
    let typed = args.join(" ");
    let refused = |e: Error| miss(name, &typed, Step::Op, e, "");
    let h = ctx.open(cell, Rights::READ).map_err(refused)?;
    let name = name.to_string();
    let mut open = Open { name, typed, h, at: 0, into, color };
    let (out, ended) = open.more(ctx)?;
    Ok(Ran { out, open: (!ended).then_some(open) })
}

/// Whether `shell` was given color: the one fact a bin that declares
/// its own `color` field is told, and [`Open::read`] strips a last
/// stage's bytes by when it is off.
fn colored(shell: &Shell) -> bool {
    let color = shell.env.iter().find(|(name, _)| name == "color");
    matches!(color, Some((_, Value::Bool(true))))
}

/// The rd of `name` under the first directory of `shell`'s PATH that
/// holds it; none where no directory does.
fn lookup(ctx: &dyn Calls, shell: &Shell, name: &str) -> Option<Rd> {
    dirs(ctx, shell).find_map(|at| ops::lookup(ctx, at, name).ok())
}

/// A name no directory of `shell`'s PATH holds, as `help NAME` and a
/// line that names one are both refused.
fn not_under(shell: &Shell, name: &str) -> Miss {
    let cause = format!("not under {}; help lists them", shell.dirs.join(" "));
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
