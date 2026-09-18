//! The output cell a run answers with.
//!
//! A Driver under `/bin` is a Driver like any other: its method 0
//! registers one unnamed rd of bytes under itself and answers with it,
//! and whoever opened that rd reads it at an offset until the read is
//! empty and closes it, which takes it out of the tree. A run that
//! follows gives its cell a [`Follow`], and then the end of the bytes
//! is not the end: the read there does the following's work. Beside the
//! cells, the two lookups such a Driver makes: the nearest `bin` above
//! its home, where it registers itself, and the rd a path names.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_kernel_interface::{Calls, DriverId, Error, Flags, Rd};

use crate::ops;
use crate::walk::{Step, walk};

/// The nearest `bin` above `home`, the Container a Driver that a typed
/// line runs registers itself under at `start`.
///
/// The walk goes up and takes the first ancestor that holds a `bin`:
/// `/bin` from `/module/coreutils` at a Kernel, `/self/bin` from
/// `/self/coreutils` at a Client, one rule and nothing in the Module
/// saying which host it is under. It never looks under `home` itself,
/// so a Module serving a child called `bin` is not taken for one
/// standing beside it.
///
/// ```ignore
/// fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
///     let bin = cell::bin(ctx, home)?;
///     let rd = ctx.register(DriverId(0), bin, Some("stat"))?;
///     ctx.report(rd, READ_CONTAINER, &[self.decl.clone()])
/// }
/// ```
///
/// # Errors
/// As `Op::Parent`: `Invalid` when no ancestor of `home` holds a
/// `bin`, the walk ending above the root.
pub fn bin(ctx: &dyn Calls, home: Rd) -> Result<Rd, Error> {
    let mut at = home;
    loop {
        // Above the root `Parent` is `Invalid`, which ends the walk.
        at = ops::parent(ctx, at)?;
        if let Ok(bin) = ops::lookup(ctx, at, "bin") {
            return Ok(bin);
        }
    }
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
        Step::Child { rd, name } => ops::lookup(ctx, rd, name),
        Step::Parent { rd } => ops::parent(ctx, rd),
    })
}

/// What a following run does at the end of what its cell holds.
///
/// Nothing delivers an event into a Driver, so a follower works inside
/// the read that reaches the end: it is asked there, and what it
/// answers is appended and served. Between two reads nothing of it
/// runs.
pub trait Follow {
    /// What the target gained since the ask before: the bytes to
    /// append, none of them while nothing has changed yet, and `None`
    /// once the follow is over and the cell ends.
    ///
    /// # Errors
    /// Whatever the ops it submits for itself failed with.
    fn more(&mut self, ctx: &dyn Calls) -> Result<Option<Vec<u8>>, Error>;

    /// The follow is over, by its own count or because the cell is
    /// closing: what the run opened closes here.
    fn end(&mut self, ctx: &dyn Calls);
}

/// One run's output: the bytes it has written, and what a read past
/// them asks, for a run that follows.
struct Cell {
    bytes: Vec<u8>,
    follow: Option<Box<dyn Follow>>,
}

/// The cells of the runs a Driver has open.
///
/// One unnamed rd of bytes under the Driver's own rd, the thing a run
/// answers with. The reader reads it at an offset until the read is
/// empty, which is its end, and closing it takes it out of the tree. A
/// cell that follows ends only when its [`Follow`] is over.
#[derive(Default)]
pub struct Cells(RefCell<BTreeMap<Rd, Cell>>);

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
        self.cell(ctx, driver, parent, Cell { bytes, follow: None })
    }

    /// A cell that goes on past the `bytes` the run found now, `follow`
    /// being what a read at its end asks. As [`open`](Self::open)
    /// otherwise.
    ///
    /// # Errors
    /// As `Calls::register` and `Calls::report`.
    pub fn follow(
        &self,
        ctx: &dyn Calls,
        driver: DriverId,
        parent: Rd,
        bytes: Vec<u8>,
        follow: Box<dyn Follow>,
    ) -> Result<Rd, Error> {
        self.cell(ctx, driver, parent, Cell { bytes, follow: Some(follow) })
    }

    /// Up to `len` bytes of the cell `rd` at `at`.
    ///
    /// At the end of what the cell holds a read is empty, which ends
    /// it. A cell that follows is asked there instead, and what it
    /// found is appended and answered, so every offset stays readable
    /// and the cell grows while it is followed.
    ///
    /// # Errors
    /// `Unsupported` for an rd that is no cell of this Driver; `Busy`
    /// at the end of a follow that has nothing new yet, since an empty
    /// read would end the cell; whatever the follow failed with.
    pub fn read(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        at: u64,
        len: usize,
    ) -> Result<Vec<u8>, Error> {
        let Some(mut follow) = self.past_end(rd, at)? else {
            return self.held(rd, at, len);
        };
        match follow.more(ctx) {
            Ok(None) => {
                follow.end(ctx);
                Ok(Vec::new())
            }
            Ok(Some(bytes)) if !bytes.is_empty() => {
                self.went_on(rd, follow, bytes);
                self.held(rd, at, len)
            }
            // Nothing new yet, or the follow failed: an empty read here
            // would end the cell, so the reader is sent back instead.
            found => {
                self.went_on(rd, follow, Vec::new());
                Err(found.err().unwrap_or(Error::Busy))
            }
        }
    }

    /// Forget the cell `rd` and take it out of the tree, its follow
    /// ended first. An rd that is no cell is the Driver's own, whose
    /// handle closes with nothing to do.
    ///
    /// # Errors
    /// As `Calls::unregister`.
    pub fn close(&self, ctx: &dyn Calls, rd: Rd) -> Result<(), Error> {
        // The borrow is gone before the Kernel is asked anything.
        let Some(mut cell) = self.0.borrow_mut().remove(&rd) else {
            return Ok(());
        };
        if let Some(follow) = &mut cell.follow {
            follow.end(ctx);
        }
        ctx.unregister(rd)
    }

    /// A cell registered under `parent` and kept: the rd the run
    /// answers with.
    fn cell(
        &self,
        ctx: &dyn Calls,
        driver: DriverId,
        parent: Rd,
        cell: Cell,
    ) -> Result<Rd, Error> {
        let rd = ctx.register(driver, parent, None)?;
        ctx.report(rd, Flags::READ, &[])?;
        self.0.borrow_mut().insert(rd, cell);
        Ok(rd)
    }

    /// The follow of the cell `rd`, when a read at `at` is past what
    /// the cell holds. It leaves the map to be asked, since it submits
    /// ops of its own and a Driver drops its borrow before it does.
    fn past_end(
        &self,
        rd: Rd,
        at: u64,
    ) -> Result<Option<Box<dyn Follow>>, Error> {
        let mut cells = self.0.borrow_mut();
        let cell = cells.get_mut(&rd).ok_or(Error::Unsupported)?;
        if usize::try_from(at).unwrap_or(usize::MAX) < cell.bytes.len() {
            return Ok(None);
        }
        Ok(cell.follow.take())
    }

    /// The follow back in the cell `rd`, with what it gave appended.
    fn went_on(&self, rd: Rd, follow: Box<dyn Follow>, bytes: Vec<u8>) {
        let mut cells = self.0.borrow_mut();
        let Some(cell) = cells.get_mut(&rd) else { return };
        cell.bytes.extend(bytes);
        cell.follow = Some(follow);
    }

    /// Up to `len` bytes the cell `rd` already holds at `at`.
    fn held(&self, rd: Rd, at: u64, len: usize) -> Result<Vec<u8>, Error> {
        let cells = self.0.borrow();
        let bytes = &cells.get(&rd).ok_or(Error::Unsupported)?.bytes;
        let start = usize::try_from(at).unwrap_or(usize::MAX).min(bytes.len());
        let end = start.saturating_add(len).min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }
}
