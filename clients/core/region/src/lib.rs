#![no_std]
#![forbid(unsafe_code)]
//! What only a Plugin over one region of `/self/dev` needs, beside
//! reovim-client-api's declarations both sides spell.
//!
//! [`Region`] is the cells of `/self/dev` a Plugin reads and writes,
//! [`Surface`] is the `view` cell it serves over one, and [`drafted`]
//! opens the Buffer every Plugin that shows text opens its own way. A
//! Client links neither; only a Plugin does, `ctx` and `home` being
//! all it is handed.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{OnceCell, RefCell};

use reovim_client_api::{buffers, keyboard, open, window};
use reovim_kernel_interface::{
    Calls, DriverId, Error, Flags, Handle, Rd, Reply, Rights, part,
};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, decode, ops};

/// Bytes asked for per read of a cell of `/self/dev`, each of which
/// holds one record or one short line of text.
const CHUNK: u32 = 64 * 1024;

/// The region a Plugin holds: the cells of `/self/dev` it reads and
/// writes, found once when the Client starts it.
///
/// A Plugin is handed `ctx` and its `home` and nothing else, so this
/// is how it reaches the Client's devices: what the region was opened
/// with, where the Client stands, what has been typed, how big the
/// region is, and the two cells that end this Plugin — `open`, which
/// gives the region away, and `exit`, which ends the Client.
#[derive(Debug)]
pub struct Region {
    /// The keyboard, opened at the end of the stream: keys typed at
    /// another Plugin are not replayed.
    keyboard: Handle,
    cwd: Rd,
    ask: Rd,
    size: Rd,
    open: Rd,
    exit: Rd,
}

impl Region {
    /// Every cell of `/self/dev`, resolved, and the keyboard opened.
    ///
    /// # Errors
    /// What the Client says; `Invalid` where there is no `/self/dev`,
    /// which is a Client this Plugin is not for.
    pub fn of(ctx: &dyn Calls) -> Result<Self, Error> {
        let at = |path: &str| cell::resolve(ctx, ROOT, path);
        Ok(Self {
            keyboard: ctx
                .open(at("/self/dev/input/keyboard")?, Rights::READ)?,
            cwd: at("/self/dev/cwd")?,
            ask: at("/self/dev/windows/0/ask")?,
            size: at("/self/dev/windows/0/size")?,
            open: at("/self/dev/open")?,
            exit: at("/self/dev/exit")?,
        })
    }

    /// What the region was opened with.
    ///
    /// # Errors
    /// What the read says; `Invalid` for bytes that are no `Ask`.
    pub fn ask(&self, ctx: &dyn Calls) -> Result<open::Ask, Error> {
        decode(&ops::bytes(ctx, self.ask)?)
    }

    /// Where the Client stands, under the root.
    ///
    /// # Errors
    /// What the read says.
    pub fn cwd(&self, ctx: &dyn Calls) -> Result<String, Error> {
        let held = ops::bytes(ctx, self.cwd)?;
        Ok(String::from_utf8_lossy(&held).into_owned())
    }

    /// How big the region is now.
    ///
    /// # Errors
    /// What the read says; `Invalid` for bytes that are no `Size`.
    pub fn size(&self, ctx: &dyn Calls) -> Result<window::Size, Error> {
        decode(&ops::bytes(ctx, self.size)?)
    }

    /// Every press this Plugin has not taken, in order: what one
    /// frame runs. A read with nothing to take is `Busy`, the stream
    /// not being over, so an empty answer is a frame with nothing
    /// typed.
    ///
    /// A press is handed on whole, the clock and the source the
    /// Client stamped with it: a code no [`Key`](reovim_client_api::Key)
    /// stands for is the reader's to drop, not this one's.
    ///
    /// # Errors
    /// What the read says, `Busy` apart.
    pub fn keys(&self, ctx: &dyn Calls) -> Result<Vec<keyboard::Press>, Error> {
        let mut keys = Vec::new();
        while let Some(press) = self.pressed(ctx)? {
            keys.push(press);
        }
        Ok(keys)
    }

    /// One press, or nothing where the stream has none for this
    /// handle: a read with nothing to take is `Busy`.
    fn pressed(
        &self,
        ctx: &dyn Calls,
    ) -> Result<Option<keyboard::Press>, Error> {
        match ops::read(ctx, self.keyboard, 0, CHUNK) {
            Ok(bytes) => decode(&bytes).map(Some),
            Err(Error::Busy) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// The Client asked to show `target` here, in the Plugin `plugin`
    /// names or, with an empty name, in the one its table gives that
    /// rd's shape. This is the end of this Plugin, once the call it
    /// was asked inside has returned.
    ///
    /// # Errors
    /// What the call says.
    pub fn opens(
        &self,
        ctx: &dyn Calls,
        target: Rd,
        plugin: &str,
    ) -> Result<(), Error> {
        let ask = open::Ask {
            target,
            plugin: String::from(plugin),
            name: String::new(),
            line: 0,
            locator: String::new(),
        };
        self.asks(ctx, ask)
    }

    /// Pass the complete declared open request to the Client.
    ///
    /// Unlike [`Self::opens`], this preserves the intended filename
    /// for an absent source, the requested starting line and the
    /// locator the region is to land on.
    ///
    /// ```no_run
    /// use reovim_client_api::{NONE, open};
    /// use reovim_client_region::Region;
    /// use reovim_kernel_interface::{Calls, Error};
    /// fn open_new(ctx: &dyn Calls) -> Result<(), Error> {
    ///     Region::of(ctx)?.asks(ctx, open::Ask {
    ///         target: NONE, plugin: String::new(),
    ///         name: "notes/new.md".into(), line: 0,
    ///         locator: String::new(),
    ///     })
    /// }
    /// ```
    ///
    /// # Errors
    /// The open, request or handle close failure.
    pub fn asks(
        &self,
        mut ctx: &dyn Calls,
        ask: open::Ask,
    ) -> Result<(), Error> {
        let h = ctx.open(self.open, Rights::WRITE)?;
        let asked = open::Call(&mut ctx, h).open(ask);
        ops::close(ctx, h)?;
        asked
    }

    /// The region back to what it showed before this Plugin.
    ///
    /// # Errors
    /// As [`Self::opens`].
    pub fn backs(&self, mut ctx: &dyn Calls) -> Result<(), Error> {
        let h = ctx.open(self.open, Rights::WRITE)?;
        let asked = open::Call(&mut ctx, h).back();
        ops::close(ctx, h)?;
        asked
    }

    /// The code the Client leaves with.
    ///
    /// # Errors
    /// What the write says.
    pub fn ends(&self, ctx: &dyn Calls, code: u8) -> Result<(), Error> {
        ops::put(ctx, self.exit, format!("{code}").into_bytes()).map(|_| ())
    }
}

/// The `view` cell a Plugin serves and the region it holds: the head
/// of every Plugin over a region of `/self/dev`, whose own state then
/// holds none of the Client's and is free at every op.
#[derive(Debug, Default)]
pub struct Surface {
    region: OnceCell<Region>,
    frames: RefCell<Vec<u8>>,
}

impl Surface {
    /// `view` registered under `home` and reported READ, and the cells
    /// of `/self/dev` found; the answer is that cell's rd.
    ///
    /// # Errors
    /// What the Client says; `Invalid` where there is no `/self/dev`,
    /// which is a Client this Plugin is not for.
    pub fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<Rd, Error> {
        let view = ctx.register(DriverId(0), home, Some("view"))?;
        ctx.report(view, Flags::READ, &[])?;
        let _ = self.region.set(Region::of(ctx)?);
        Ok(view)
    }

    /// The cells of `/self/dev` this Plugin holds.
    ///
    /// # Errors
    /// `Closed` before [`Self::start`] has found them.
    pub fn region(&self) -> Result<&Region, Error> {
        self.region.get().ok_or(Error::Closed)
    }

    /// The `Driver::read` of the `view` cell: one whole read is one
    /// frame. The read at 0 runs `frame` and keeps what it answered;
    /// every read past those bytes serves their tail, so the reader
    /// gets one View and then the empty read that ends it.
    pub fn read<F>(&self, at: u64, len: usize, frame: F) -> Reply<Vec<u8>>
    where
        F: FnOnce() -> Result<Vec<u8>, Error>,
    {
        self.framed(at, len, frame).into()
    }

    fn framed<F>(&self, at: u64, len: usize, frame: F) -> Result<Vec<u8>, Error>
    where
        F: FnOnce() -> Result<Vec<u8>, Error>,
    {
        if at == 0 {
            // The borrow is taken only once the frame has answered: a
            // frame reads cells of its own and may not find this one
            // held.
            let shown = frame()?;
            *self.frames.borrow_mut() = shown;
        }
        Ok(part(&self.frames.borrow(), at, len))
    }
}

/// The draft over `source`, or over nothing: the Client's own copy of
/// a Buffer, which every Plugin that shows text opens this one way.
///
/// The caller resolved `source`, as [`buffers::Api::open`] takes an
/// rd; a source a draft is already over answers that draft.
///
/// # Errors
/// What the Client says; `Invalid` where there is no
/// `/self/dev/buffers`, which is a Client this Plugin is not for.
pub fn drafted(mut ctx: &dyn Calls, source: Option<Rd>) -> Result<Rd, Error> {
    let list = cell::resolve(ctx, ROOT, "/self/dev/buffers")?;
    let h = ctx.open(list, Rights::WRITE)?;
    let mut call = buffers::Call(&mut ctx, h);
    let draft = match source {
        Some(source) => call.open(source),
        None => call.scratch(),
    };
    ops::close(ctx, h)?;
    draft
}

/// A count the Client gave, as far as this machine counts.
#[must_use]
pub fn count(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}
