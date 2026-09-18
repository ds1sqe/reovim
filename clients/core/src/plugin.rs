//! The Plugin of 02-client.md: it lives at a Client, makes its own
//! calls on the tree, and reaches what its Client serves through the
//! capabilities that Client provides.
//!
//! Nothing is handed back. What was drawn is a `grid::show`, what was
//! said is that View's status, and what ends the program is
//! `process::exit`. So a Plugin the Client cannot serve at all still
//! runs, and a Client that serves more than a grid needs no new
//! contract.

use alloc::boxed::Box;
use alloc::string::String;

use reovim_kernel_interface::{Calls, Error, Event, Hash, Rd};

use crate::caps::Caps;

/// What a Plugin is given for the length of one call: the one door its
/// ops go through, and the table of what its Client serves.
pub struct Ctx<'a> {
    pub ctx: &'a dyn Calls,
    pub caps: &'a mut Caps,
}

/// What the region the Plugin was started in is to show.
///
/// Its own client rd, the rd the region was opened on, the name under
/// the root a relative one starts from, and the line to begin at. What
/// to open over `over` is the Plugin's: a Container has no Buffer, and
/// one that shows text makes its own.
pub struct Start {
    pub client: Rd,
    /// The rd the region was opened on: a cell to show, a Container to
    /// list. `None` is a region over nothing, which with a `name` is a
    /// file that is not on disk.
    pub over: Option<Rd>,
    /// The name under the root a region over nothing was opened with,
    /// there being no such file: the one its first write makes. `None`
    /// for a region opened on an rd and for one opened with no name.
    pub name: Option<String>,
    pub dir: String,
    /// The line to begin at, counted from 1, clamped to the last line
    /// by whoever has the text. `None` begins at the first.
    pub line: Option<u64>,
}

/// Inputs in, calls out.
///
/// Dyn-compatible on purpose: one Client may hold several, each over
/// an rd of its own with handles of its own.
pub trait Plugin {
    /// The Client is ready.
    ///
    /// # Errors
    /// What a call it made says.
    fn start(&mut self, ctx: Ctx<'_>, at: Start) -> Result<(), Error>;

    /// One input of the capability `from`: `bytes` is one of its
    /// records. An input of an api the Plugin does not know is not an
    /// error; it is ignored.
    ///
    /// # Errors
    /// As [`Self::start`].
    fn input(
        &mut self,
        ctx: Ctx<'_>,
        from: Hash,
        bytes: &[u8],
    ) -> Result<(), Error>;

    /// What happened on the watched rds; what the Plugin shows may
    /// have been written by somebody else.
    ///
    /// # Errors
    /// As [`Self::start`].
    fn changed(&mut self, ctx: Ctx<'_>, events: &[Event]) -> Result<(), Error>;
}

/// A Plugin in a box is a Plugin, so a Client that holds one region's
/// Plugin behind `Box<dyn Plugin>` needs no Plugin of its own.
impl<P: Plugin + ?Sized> Plugin for Box<P> {
    fn start(&mut self, ctx: Ctx<'_>, at: Start) -> Result<(), Error> {
        (**self).start(ctx, at)
    }

    fn input(
        &mut self,
        ctx: Ctx<'_>,
        from: Hash,
        bytes: &[u8],
    ) -> Result<(), Error> {
        (**self).input(ctx, from, bytes)
    }

    fn changed(&mut self, ctx: Ctx<'_>, events: &[Event]) -> Result<(), Error> {
        (**self).changed(ctx, events)
    }
}
