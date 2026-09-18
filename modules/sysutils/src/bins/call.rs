//! `call`, a Driver under `/bin`: its declaration and its run. The
//! slots it is served through are the crate's.
//!
//! One run answers with an output cell holding what the target's method
//! answered, decoded by the declaration that names it. The method is
//! found among the apis the target offers, so a name none of them has
//! fails the run `Unsupported`; an empty `arg` is no argument, and one
//! that does not fit the declared input is `Invalid`. This is the one
//! name under `/bin` that no other spells: every other tree op is some
//! Driver's own shaping of it.

use alloc::vec::Vec;

use reovim_kernel_interface::{Calls, Error, Hash, Method, Op, Out, Rd};
use reovim_sdk::{cell, dynamic, ops};

use crate::Sysutils;

reovim_sdk::declare! {
    api call ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args {
        cwd: Rd,
        target: utf8(256),
        method: utf8(64),
        arg: utf8(1024)
    }
}

/// What a run takes, beside the api it belongs to.
pub use call::Args;
pub(crate) use call::{decl, dispatch};

impl call::Api for Sysutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        let target = cell::resolve(ctx, args.cwd, &args.target)?;
        let (api, method) = named(ctx, target, &args.method)?;
        let arg = (!args.arg.is_empty()).then_some(args.arg.as_str());
        let input = dynamic::encode_arg(&method.input, arg)?;
        let output = called(ctx, target, api, &method, input)?;
        self.cell(ctx, rd, dynamic::show(&method.output, &output)?)
    }
}

/// The method of that name among the apis `rd` offers, with the api it
/// belongs to; `Unsupported` when none of them has one.
fn named(ctx: &dyn Calls, rd: Rd, name: &str) -> Result<(Hash, Method), Error> {
    for hash in ops::info(ctx, rd)?.apis {
        let decl = ops::decl(ctx, hash)?;
        let asked = decl.methods.iter().find(|method| method.name == name);
        if let Some(method) = asked {
            return Ok((hash, method.clone()));
        }
    }
    Err(Error::Unsupported)
}

/// The call itself, on a handle opened with the right the method asks
/// for and closed after it.
fn called(
    ctx: &dyn Calls,
    rd: Rd,
    api: Hash,
    method: &Method,
    input: Vec<u8>,
) -> Result<Vec<u8>, Error> {
    let h = ctx.open(rd, method.right)?;
    let op = Op::Call { h, api, method: method.number, input };
    let answer = ops::ready(ctx.submit(op));
    let _ = ctx.submit(Op::Close { h });
    match answer? {
        Out::Bytes(bytes) => Ok(bytes),
        _ => Err(Error::Invalid),
    }
}
