//! The Plugin of 02-client.md: it lives at a Client, makes its own
//! calls on the tree, and reaches what its Client serves through the
//! capabilities that Client provides.
//!
//! Nothing is handed back. What was drawn is a `grid::show`, what was
//! said is that View's status, and what ends the program is
//! `process::exit`. So a Plugin the Client cannot serve at all still
//! runs, and a Client that serves more than a grid needs no new
//! contract.

use alloc::string::String;

use reovim_kernel_interface::{Event, Hash, Rd};

use crate::Fault;
use crate::caps::Caps;
use crate::client::Tree;

/// What a Plugin is given for the length of one call: the tree its
/// calls go to, and the table of what its Client serves.
pub struct Ctx<'a> {
    pub tree: &'a mut Tree,
    pub caps: &'a mut Caps,
}

/// What the Client had opened before the Plugin's first call.
///
/// Its own client rd, the Buffer to work on and what that Buffer is
/// over, the name under the root a relative one starts from, and the
/// byte to begin at.
pub struct Start {
    pub client: Rd,
    pub buffer: Rd,
    pub source: Option<Rd>,
    pub dir: String,
    pub at: u64,
}

/// Inputs in, calls out.
///
/// Dyn-compatible on purpose: one Client may hold several, each
/// started on the same Buffer with handles of its own.
pub trait Plugin {
    /// The Client is ready.
    ///
    /// # Errors
    /// What a call it made says.
    fn start(&mut self, ctx: Ctx<'_>, at: Start) -> Result<(), Fault>;

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
    ) -> Result<(), Fault>;

    /// What happened on the watched rds; the Buffer may have been
    /// written by somebody else.
    ///
    /// # Errors
    /// As [`Self::start`].
    fn changed(&mut self, ctx: Ctx<'_>, events: &[Event]) -> Result<(), Fault>;
}
