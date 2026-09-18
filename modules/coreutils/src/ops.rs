//! What a Driver under `/bin` asks the Kernel, over its own `Calls`:
//! what is cached about an rd, what is under it, the bytes it holds,
//! and the declaration a hash names. None of it may wait, so a reply
//! that is not ready is `Busy`.

use alloc::format;
use alloc::vec::Vec;

use reovim_kernel_interface::{
    Calls, Decl, Error, Handle, Hash, Info, Op, Out, Rd, Reply, Rights,
};
use reovim_sdk::cell;
use reovim_sdk::walk::ROOT;

/// Bytes asked for per `Read`.
const CHUNK: u32 = 64 * 1024;

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
    let read = read_all(ctx, h);
    let _ = ctx.submit(Op::Close { h });
    read
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

/// A `Reply` the Driver needs now: `Pending` is `Busy`, it cannot wait.
fn ready(reply: Reply<Out>) -> Result<Out, Error> {
    match reply {
        Reply::Ready(result) => result,
        Reply::Pending(_) => Err(Error::Busy),
    }
}
