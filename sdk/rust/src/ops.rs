//! What a caller asks of a tree, over its own `Calls`: a Driver under
//! `/bin`, a Plugin, a shell, a Client.
//!
//! What is cached about an rd, what is under it, where it stands, the
//! bytes it holds, the method a name stands for and the declaration a
//! hash names. None of it may wait, so a reply that is not ready is
//! `Busy`.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_kernel_interface::{
    Calls, Decl, Error, Event, Flags, Handle, Hash, Info, Method, Op, Out, Rd,
    Reply, Rights,
};

use crate::cell;
use crate::walk::ROOT;

/// Bytes asked for per `Read`.
const CHUNK: u32 = 64 * 1024;

/// Names of a path past which [`where_is`] says the rd instead, as the
/// depth of a walk is nobody's to trust.
const DEEP: usize = 64;

/// The child of `rd` named `name`.
///
/// # Errors
/// As `Op::Lookup`; `Invalid` for a name that is not there.
pub fn lookup(ctx: &dyn Calls, rd: Rd, name: &str) -> Result<Rd, Error> {
    let name = name.to_string();
    rd_of(&ready(ctx.submit(Op::Lookup { rd, name }))?)
}

/// What `rd` stands under.
///
/// # Errors
/// As `Op::Parent`; `Invalid` for the root, which stands under
/// nothing.
pub fn parent(ctx: &dyn Calls, rd: Rd) -> Result<Rd, Error> {
    rd_of(&ready(ctx.submit(Op::Parent { rd }))?)
}

/// What the Kernel caches about `rd`.
///
/// # Errors
/// As `Op::Info`; `Invalid` for an rd that is not there.
pub fn info(ctx: &dyn Calls, rd: Rd) -> Result<Info, Error> {
    match ready(ctx.submit(Op::Info { rd }))? {
        Out::Info(info) => Ok(info),
        _ => Err(Error::Invalid),
    }
}

/// The rds under `rd`, in whatever order its Driver lists them.
///
/// # Errors
/// As `Op::Children`; `Unsupported` for an rd that lists nothing.
pub fn children(ctx: &dyn Calls, rd: Rd) -> Result<Vec<Rd>, Error> {
    match ready(ctx.submit(Op::Children { rd }))? {
        Out::Rds(rds) => Ok(rds),
        _ => Err(Error::Invalid),
    }
}

/// Every byte of `rd`, through a handle opened and closed here.
///
/// # Errors
/// As `Op::Open` and `Op::Read`; `Unsupported` for a Container, which
/// holds no bytes.
pub fn bytes(ctx: &dyn Calls, rd: Rd) -> Result<Vec<u8>, Error> {
    let h = ctx.open(rd, Rights::READ)?;
    let held = whole(ctx, h);
    let _ = close(ctx, h);
    held
}

/// Up to `len` bytes of `h` at `at`.
///
/// # Errors
/// As `Op::Read`; `Busy` where a run that follows has nothing new yet.
pub fn read(
    ctx: &dyn Calls,
    h: Handle,
    at: u64,
    len: u32,
) -> Result<Vec<u8>, Error> {
    match ready(ctx.submit(Op::Read { h, at, len }))? {
        Out::Bytes(bytes) => Ok(bytes),
        _ => Err(Error::Invalid),
    }
}

/// `bytes` through `h` at `at`; how many of them it took.
///
/// # Errors
/// As `Op::Write`.
pub fn write(
    ctx: &dyn Calls,
    h: Handle,
    at: u64,
    bytes: Vec<u8>,
) -> Result<u32, Error> {
    match ready(ctx.submit(Op::Write { h, at, bytes }))? {
        Out::Len(len) => Ok(len),
        _ => Err(Error::Invalid),
    }
}

/// Every byte through `h` from 0, one `Read` at a time until an empty
/// one: what a caller that holds its own handle open reads with.
///
/// # Errors
/// As [`read`].
pub fn whole(ctx: &dyn Calls, h: Handle) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    loop {
        let chunk = read(ctx, h, bytes.len() as u64, CHUNK)?;
        if chunk.is_empty() {
            return Ok(bytes);
        }
        bytes.extend(chunk);
    }
}

/// `bytes` at 0 of `rd`, through a handle opened and closed here.
///
/// # Errors
/// As `Op::Open` and [`write()`].
pub fn put(ctx: &dyn Calls, rd: Rd, bytes: Vec<u8>) -> Result<u32, Error> {
    let h = ctx.open(rd, Rights::WRITE)?;
    let written = write(ctx, h, 0, bytes);
    let _ = close(ctx, h);
    written
}

/// The first method named `name` among the apis `rd` reports, with the
/// hash of the declaration holding it.
///
/// # Errors
/// `Unsupported` when no api of `rd` declares that name; otherwise as
/// [`info`] and [`decl`].
pub fn method(
    ctx: &dyn Calls,
    rd: Rd,
    name: &str,
) -> Result<(Hash, Method), Error> {
    for hash in info(ctx, rd)?.apis {
        let methods = decl(ctx, hash)?.methods;
        if let Some(found) = methods.into_iter().find(|m| m.name == name) {
            return Ok((hash, found));
        }
    }
    Err(Error::Unsupported)
}

/// `method` of `api` run on `rd` with `input` already encoded, through
/// a handle opened with the method's right and closed here.
///
/// # Errors
/// As `Op::Open` and `Op::Call`.
pub fn call(
    ctx: &dyn Calls,
    rd: Rd,
    api: Hash,
    method: &Method,
    input: Vec<u8>,
) -> Result<Vec<u8>, Error> {
    let h = ctx.open(rd, method.right)?;
    let out = called(ctx, h, api, method.number, input);
    let _ = close(ctx, h);
    out
}

/// [`method`] then [`call`]: the method of that name, run.
///
/// # Errors
/// As both.
pub fn call_by_name(
    ctx: &dyn Calls,
    rd: Rd,
    name: &str,
    input: Vec<u8>,
) -> Result<Vec<u8>, Error> {
    let (api, found) = method(ctx, rd, name)?;
    call(ctx, rd, api, &found, input)
}

/// The names that may replace `word`, in name order.
///
/// They are the children of the Container `word`'s directory part
/// names whose own names begin as its last part does, a Container
/// with a `/` after it so the next Tab goes on under it.
///
/// `a/b` looks under `a`, a word with no `/` under `under` itself, and
/// a word beginning with `/` from the tree's root, as a target is
/// read; `.` stays where it is and `..` goes to the parent. A word
/// whose directory part resolves nowhere offers nothing, which is not
/// a fault: a name that is not there yet is the ordinary case.
#[must_use]
pub fn paths(ctx: &dyn Calls, under: Rd, word: &str) -> Vec<String> {
    let (dir, prefix) = word
        .rfind('/')
        .map_or(("", word), |cut| (&word[..=cut], &word[cut + 1..]));
    let Ok(rd) = cell::resolve(ctx, under, dir) else { return Vec::new() };
    let Ok(below) = children(ctx, rd) else { return Vec::new() };
    let mut names: Vec<String> = below
        .into_iter()
        .filter_map(|child| named(ctx, child, prefix))
        .map(|(name, container)| {
            let end = if container { "/" } else { "" };
            format!("{dir}{name}{end}")
        })
        .collect();
    names.sort();
    names
}

/// The child's name when it begins as `prefix` does, and whether it is
/// a Container.
fn named(ctx: &dyn Calls, child: Rd, prefix: &str) -> Option<(String, bool)> {
    let info = info(ctx, child).ok()?;
    let name = info.name?;
    name.starts_with(prefix)
        .then(|| (name, info.flags.contains(Flags::CONTAINER)))
}

/// A handle on `rd`, open and watching it, which is what a run that
/// follows keeps: closing it stops the watch.
///
/// # Errors
/// As `Op::Open` and `Op::Watch`.
pub fn watch(ctx: &dyn Calls, rd: Rd) -> Result<Handle, Error> {
    let h = ctx.open(rd, Rights::READ)?;
    ready(ctx.submit(Op::Watch { h, on: true }))?;
    Ok(h)
}

/// A handle on each of `rds`, open and watching, or none at all: what
/// a run that follows several targets keeps.
///
/// # Errors
/// As [`watch`]; a failure closes the handles opened before it, so a
/// run that fails leaves nothing open.
pub fn watch_all(ctx: &dyn Calls, rds: &[Rd]) -> Result<Vec<Handle>, Error> {
    let mut handles = Vec::new();
    for rd in rds {
        let opened = watch(ctx, *rd).inspect_err(|_| shut(ctx, &handles));
        handles.push(opened?);
    }
    Ok(handles)
}

/// `h` closed, which is the end of whatever it was open on.
///
/// # Errors
/// As `Op::Close`; `Invalid` for a handle that is not open.
pub fn close(ctx: &dyn Calls, h: Handle) -> Result<(), Error> {
    ready(ctx.submit(Op::Close { h })).map(|_| ())
}

/// Every one of `handles` closed, whatever each close answers.
pub fn shut(ctx: &dyn Calls, handles: &[Handle]) {
    for h in handles {
        let _ = close(ctx, *h);
    }
}

/// The events after `cursor` on every rd this Module watches, and the
/// cursor for the next ask.
///
/// The Kernel keeps no cursor of its own, so each follower holds one
/// and takes its own rds out of what it gets.
///
/// # Errors
/// As `Op::Since`.
pub fn since(ctx: &dyn Calls, cursor: u64) -> Result<(Vec<Event>, u64), Error> {
    match ready(ctx.submit(Op::Since { cursor }))? {
        Out::Events { events, cursor, .. } => Ok((events, cursor)),
        _ => Err(Error::Invalid),
    }
}

/// The declaration `hash` names, as the Kernel publishes it at
/// `/sys/decls/<hash>`.
///
/// # Errors
/// As [`bytes`], and `Invalid` for bytes that are no declaration.
pub fn decl(ctx: &dyn Calls, hash: Hash) -> Result<Decl, Error> {
    let rd = cell::resolve(ctx, ROOT, &format!("/sys/decls/{:016x}", hash.0))?;
    Decl::parse(&bytes(ctx, rd)?)
}

/// The path of `rd`, read back from the tree: its name under each
/// parent up to the root. One that is not named the whole way up, or
/// stands deeper than `DEEP`, has no path to show and is `rd:<n>`.
///
/// # Errors
/// As [`info`] and `Op::Parent`.
pub fn where_is(ctx: &dyn Calls, rd: Rd) -> Result<String, Error> {
    let mut names: Vec<String> = Vec::new();
    let mut at = rd;
    while at != ROOT {
        let Some(name) = info(ctx, at)?.name else {
            return Ok(format!("rd:{}", rd.0));
        };
        if names.len() == DEEP {
            return Ok(format!("rd:{}", rd.0));
        }
        names.push(name);
        // `..` is the walk's own move to the parent.
        at = cell::resolve(ctx, at, "..")?;
    }
    names.reverse();
    Ok(format!("/{}", names.join("/")))
}

/// Method `number` of `api` through the open handle `h`; its output
/// bytes. What [`call`] does once the handle is there, and what a
/// `declare!`d api's caller side runs on.
pub(crate) fn called(
    ctx: &dyn Calls,
    h: Handle,
    api: Hash,
    number: u16,
    input: Vec<u8>,
) -> Result<Vec<u8>, Error> {
    match ready(ctx.submit(Op::Call { h, api, method: number, input }))? {
        Out::Bytes(bytes) => Ok(bytes),
        _ => Err(Error::Invalid),
    }
}

/// The rd a finished `Lookup` or `Parent` answered; neither answers
/// anything else.
const fn rd_of(out: &Out) -> Result<Rd, Error> {
    match out {
        Out::Rd(rd) => Ok(*rd),
        _ => Err(Error::Invalid),
    }
}

/// A `Reply` the Driver needs now: `Pending` is `Busy`, it cannot wait.
///
/// The rule of every read here, and of the ops a Driver submits for
/// itself: a `run` answers with a cell or with an `Error`, and there
/// is nowhere for it to wait.
///
/// # Errors
/// Whatever the op failed with; `Busy` for a reply that is not ready.
pub fn ready(reply: Reply<Out>) -> Result<Out, Error> {
    match reply {
        Reply::Ready(result) => result,
        Reply::Pending(_) => Err(Error::Busy),
    }
}
