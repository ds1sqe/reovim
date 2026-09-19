//! A Plugin a test gives a region to, and what every test Plugin
//! opens: a Buffer of its own and a View naming it.
//!
//! The stubs of the Plugin tests differ in one thing only — the word
//! they put in the status row, and whether a key hands the region on —
//! so they are this one Module. What a region becomes is then judged
//! by what the stub was started over.

use std::cell::RefCell;
use std::rc::Rc;

use reovim_client_api::{BLOCK, NONE, window};
use reovim_client_region::{Region, Surface};
use reovim_kernel_interface::{
    Booted, Calls, Driver, Error, Flags, Module, Rd, Reply, Rights,
};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, encode, ops};
use reovim_text_api::open as text_open;

/// What each start of a stub was over, in order.
pub type Seen = Rc<RefCell<Vec<Option<Rd>>>>;

/// What a stub does at the first frame that took a key: it is handed
/// the region and the rd it was started over, which is how a test
/// scripts where the region goes next.
pub type Typed =
    Box<dyn Fn(&dyn Calls, &Region, Option<Rd>) -> Result<(), Error>>;

/// What a stub keeps: the rd it was started over, its region, and the
/// one View it shows.
#[derive(Default)]
struct Held {
    over: Option<Rd>,
    shown: Vec<u8>,
}

/// A Plugin a test gives a region to: it records what it was started
/// over, shows a Buffer over that rd — or a scratch one over nothing —
/// and says `word` in the status row.
pub struct Stub {
    seen: Seen,
    word: String,
    typed: Option<Typed>,
    state: RefCell<Held>,
    surface: Surface,
}

impl Stub {
    /// One under `word`, recording into `seen`, for a table to give.
    #[must_use]
    pub fn booted(seen: &Seen, word: &str) -> Booted {
        Self::made(seen, word, None)
    }

    /// One that also runs `typed` at the first frame that took a key.
    #[must_use]
    pub fn typing(seen: &Seen, word: &str, typed: Typed) -> Booted {
        Self::made(seen, word, Some(typed))
    }

    fn made(seen: &Seen, word: &str, typed: Option<Typed>) -> Booted {
        let stub = Rc::new(Self {
            seen: seen.clone(),
            word: String::from(word),
            typed,
            state: RefCell::default(),
            surface: Surface::default(),
        });
        Booted { module: stub.clone(), drivers: vec![stub] }
    }

    /// One frame: the keys it has not taken, what they were scripted
    /// to do, and the one View it shows.
    fn framed(&self, ctx: &dyn Calls) -> Result<Vec<u8>, Error> {
        let over = self.state.borrow().over;
        let typed = !self.surface.region()?.keys(ctx)?.is_empty();
        let shown = self.state.borrow().shown.clone();
        let Some(act) = self.typed.as_ref().filter(|_| typed) else {
            return Ok(shown);
        };
        act(ctx, self.surface.region()?, over)?;
        Ok(shown)
    }
}

impl Module for Stub {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        self.surface.start(ctx, home)?;
        let ask = self.surface.region()?.ask(ctx)?;
        let over = (ask.target != NONE).then_some(ask.target);
        self.seen.borrow_mut().push(over);
        // A Container holds no text of its own, so the stub shows a
        // scratch Buffer there, as a listing would.
        let source = over.filter(|rd| !container(ctx, *rd));
        let buffer = buffer_over(ctx, source)?;
        let mut held = self.state.borrow_mut();
        held.over = over;
        held.shown = encode(&shown(buffer, &self.word));
        Ok(())
    }
}

impl Driver for Stub {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        ctx: &dyn Calls,
        _: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        self.surface.read(at, len, || self.framed(ctx))
    }
}

/// Whether the tree says `rd` is a Container.
fn container(ctx: &dyn Calls, rd: Rd) -> bool {
    ops::info(ctx, rd).is_ok_and(|it| it.flags.contains(Flags::CONTAINER))
}

/// A Buffer over `source`, or over nothing, as every Plugin that shows
/// text opens its own.
///
/// # Errors
/// What the call to `/module/text/open` says.
pub fn buffer_over(
    mut ctx: &dyn Calls,
    source: Option<Rd>,
) -> Result<Rd, Error> {
    let opener = cell::resolve(ctx, ROOT, "/module/text/open")?;
    let opening = ctx.open(opener, Rights::WRITE)?;
    let mut call = text_open::Call(&mut ctx, opening);
    let buffer = match source {
        Some(source) => call.open(source),
        None => call.new(),
    };
    ops::close(ctx, opening)?;
    buffer
}

/// A View over the whole of `over`, a block cursor on its first byte
/// and `word` in the status row.
#[must_use]
pub fn shown(over: Rd, word: &str) -> window::View {
    window::View {
        over,
        top: 0,
        cursor: 0,
        shape: BLOCK,
        spans: Vec::new(),
        status: String::from(word),
    }
}
