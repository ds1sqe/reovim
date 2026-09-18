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

use reovim_client::value::Value;
use reovim_client::{Error, Fault, Field, Handle, Rd, Rights, Tree, Ty, value};

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
    pub fn more(&mut self, tree: &mut Tree) -> Result<(Vec<u8>, bool), Miss> {
        let (bytes, ended) = reading(tree, &self.name, self.h, &mut self.at)?;
        if ended {
            self.close(tree)?;
        }
        Ok((bytes, ended))
    }

    /// The cell closed, which takes it out of the tree and ends what the
    /// run was following.
    ///
    /// # Errors
    /// Whatever the close says.
    pub fn close(&mut self, tree: &mut Tree) -> Result<(), Miss> {
        let refused = |f: Fault| miss(&self.name, "", Step::Op, &f, "");
        tree.close(self.h).map_err(refused)
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
pub fn names(tree: &mut Tree) -> Vec<String> {
    let Ok(at) = tree.resolve(PATH) else { return Vec::new() };
    let Ok(children) = tree.children(at) else { return Vec::new() };
    let mut names: Vec<String> =
        children.into_iter().filter_map(|rd| tree.name(rd).ok()).collect();
    names.sort();
    names
}

/// The fields the run of `name` takes; `None` for a name PATH does not
/// hold or a declaration that cannot be read, which Tab offers nothing
/// for and never faults over.
pub fn fields(tree: &mut Tree, name: &str) -> Option<Vec<Field>> {
    let rd = lookup(tree, name)?;
    let (_, method) = tree.method(rd, "run").ok()?;
    match method.input {
        Ty::Record(fields) => Some(fields),
        _ => None,
    }
}

/// The line `help` prints for `name`.
///
/// # Errors
/// `Miss` for a name PATH does not hold.
pub fn form(tree: &mut Tree, name: &str) -> Result<String, Miss> {
    let Some(fields) = fields(tree, name) else {
        return Err(unknown(name));
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
    tree: &mut Tree,
    shell: &Shell,
    name: &str,
    args: &[&str],
    sink: Option<Sink<'_>>,
) -> Result<Ran, Miss> {
    let Some(rd) = lookup(tree, name) else {
        return Err(unknown(name));
    };
    let typed = args.join(" ");
    let refused = |f: Fault| miss(name, &typed, Step::Op, &f, "");
    let (api, method) = tree.method(rd, "run").map_err(refused)?;
    let input = decode::record(shell, name, &method.input, args)?;
    // The target resolves first, so a bad one runs nothing.
    let into = match sink {
        Some(sink) => Some(poured(tree, shell, name, &sink)?),
        None => None,
    };
    let out = tree.call_method(rd, api, &method, input).map_err(refused)?;
    let answered = value::decode(&method.output, &out).map_err(refused)?;
    let Value::Rd(cell) = answered else {
        return Err(refused(Fault::Wire));
    };
    let h = tree.open(cell, Rights::READ).map_err(refused)?;
    let mut open = Open { name: name.to_string(), h, at: 0 };
    let read = reading(tree, name, h, &mut open.at);
    let (bytes, ended) = match read {
        Ok(got) => got,
        Err(stopped) => {
            let _ = open.close(tree);
            return Err(stopped);
        }
    };
    let out = match into {
        Some((rd, at)) => {
            written(tree, rd, at, bytes).map_err(refused)?;
            Vec::new()
        }
        None => bytes,
    };
    if !ended {
        return Ok(Ran { out, open: Some(open) });
    }
    open.close(tree)?;
    Ok(Ran { out, open: None })
}

/// The rd of a name PATH does not hold: none.
fn lookup(tree: &mut Tree, name: &str) -> Option<Rd> {
    let at = tree.resolve(PATH).ok()?;
    tree.lookup(at, name).ok()
}

fn unknown(name: &str) -> Miss {
    let cause = format!("not under {PATH}; help lists them");
    Miss { name: name.to_string(), target: String::new(), cause }
}

/// One reading of the cell from `at`: the bytes, and whether the read
/// that stopped it was the empty one that ends a cell. A `Busy` is a
/// follower at the end of what it has now, and the cell goes on.
fn reading(
    tree: &mut Tree,
    name: &str,
    h: Handle,
    at: &mut u64,
) -> Result<(Vec<u8>, bool), Miss> {
    let mut out = Vec::new();
    loop {
        match tree.read(h, *at, CHUNK) {
            Ok(chunk) if chunk.is_empty() => return Ok((out, true)),
            Ok(chunk) => {
                *at += chunk.len() as u64;
                out.extend(chunk);
            }
            Err(Fault::Kernel(Error::Busy)) => return Ok((out, false)),
            Err(fault) => {
                return Err(miss(name, "", Step::Op, &fault, ""));
            }
        }
    }
}

/// Where a redirect writes: the target, which must be there already,
/// and the offset, 0 for `>` and what the target holds for `>>`, which
/// a whole read counts since `Info` carries no size.
fn poured(
    tree: &mut Tree,
    shell: &Shell,
    name: &str,
    sink: &Sink<'_>,
) -> Result<(Rd, u64), Miss> {
    let rd = resolved(tree, shell, name, sink.target)?;
    if !sink.append {
        return Ok((rd, 0));
    }
    let refused = |f: Fault| miss(name, sink.target, Step::Op, &f, "");
    let held = tree.read_all(rd).map_err(refused)?;
    Ok((rd, held.len() as u64))
}

/// `bytes` into `rd` at `at`, through a handle opened and closed here.
fn written(
    tree: &mut Tree,
    rd: Rd,
    at: u64,
    bytes: Vec<u8>,
) -> Result<(), Fault> {
    let h = tree.open(rd, Rights::WRITE)?;
    let written = tree.write(h, at, bytes);
    tree.close(h)?;
    written.map(|_| ())
}
