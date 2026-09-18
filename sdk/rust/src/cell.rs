//! The output cell a run answers with.
//!
//! A Driver under `/bin` is a Driver like any other: its method 0
//! registers one unnamed rd of bytes under itself and answers with it,
//! and whoever opened that rd reads it at an offset until the read is
//! empty and closes it, which takes it out of the tree. Beside the
//! cells, the two lookups such a Driver makes: the root's `bin`, where
//! it registers itself, and the rd a path names.

use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_kernel_interface::{
    Calls, DriverId, Error, Flags, Op, Out, Rd, Reply,
};

use crate::walk::{ROOT, Step, walk};

/// The root's `bin`, the Container a Driver serving a command line
/// registers itself under at `start`.
///
/// ```ignore
/// fn start(&self, ctx: &dyn Calls, _: Rd) -> Result<(), Error> {
///     let rd = ctx.register(DriverId(0), cell::bin(ctx)?, Some("stat"))?;
///     ctx.report(rd, READ_CONTAINER, &[self.decl.clone()])
/// }
/// ```
///
/// # Errors
/// As `Op::Lookup`; `Invalid` when the root has no `bin`.
pub fn bin(ctx: &dyn Calls) -> Result<Rd, Error> {
    found(ctx.submit(Op::Lookup { rd: ROOT, name: "bin".to_string() }))
}

/// The rd `path` names, from `from` when it is relative, as [`walk`]
/// walks it over this Module's own ops.
///
/// ```ignore
/// let rd = cell::resolve(ctx, args.cwd, &args.target)?;
/// ```
///
/// # Errors
/// As `Op::Lookup` and `Op::Parent`: `Invalid` for a segment that is
/// not there.
pub fn resolve(ctx: &dyn Calls, from: Rd, path: &str) -> Result<Rd, Error> {
    walk(from, path, |step| match step {
        Step::Child { rd, name } => {
            found(ctx.submit(Op::Lookup { rd, name: name.to_string() }))
        }
        Step::Parent { rd } => found(ctx.submit(Op::Parent { rd })),
    })
}

/// The rd a finished `Lookup` or `Parent` answered; neither answers
/// anything else, and neither may block.
fn found(reply: Reply<Out>) -> Result<Rd, Error> {
    let Reply::Ready(result) = reply else { return Err(Error::Invalid) };
    match result? {
        Out::Rd(rd) => Ok(rd),
        _ => Err(Error::Invalid),
    }
}

/// The cells of the runs a Driver has open.
///
/// One unnamed rd of bytes under the Driver's own rd, the thing a run
/// answers with. The reader reads it at an offset until the read is
/// empty, which is its end, and closing it takes it out of the tree.
#[derive(Default)]
pub struct Cells(RefCell<BTreeMap<Rd, Vec<u8>>>);

impl Cells {
    /// A cell of `bytes` under `parent`, served by `driver`: unnamed,
    /// READ, and the rd the run answers with.
    ///
    /// # Errors
    /// As `Calls::register` and `Calls::report`.
    pub fn open(
        &self,
        ctx: &dyn Calls,
        driver: DriverId,
        parent: Rd,
        bytes: Vec<u8>,
    ) -> Result<Rd, Error> {
        let rd = ctx.register(driver, parent, None)?;
        ctx.report(rd, Flags::READ, &[])?;
        self.0.borrow_mut().insert(rd, bytes);
        Ok(rd)
    }

    /// Up to `len` bytes of the cell `rd` at `at`; none at its end.
    ///
    /// # Errors
    /// `Unsupported` for an rd that is no cell of this Driver.
    pub fn read(&self, rd: Rd, at: u64, len: usize) -> Result<Vec<u8>, Error> {
        let cells = self.0.borrow();
        let bytes = cells.get(&rd).ok_or(Error::Unsupported)?;
        let start = usize::try_from(at).unwrap_or(usize::MAX).min(bytes.len());
        let end = start.saturating_add(len).min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }

    /// Forget the cell `rd` and take it out of the tree. An rd that is
    /// no cell is the Driver's own, whose handle closes with nothing
    /// to do.
    ///
    /// # Errors
    /// As `Calls::unregister`.
    pub fn close(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        // The borrow is gone before the Kernel is asked anything.
        let held = self.0.borrow_mut().remove(&rd).is_some();
        if held { ctx.unregister(rd) } else { Ok(()) }
    }
}
